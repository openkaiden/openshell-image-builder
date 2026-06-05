// Copyright (C) 2026 Red Hat, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0

use super::Inference;

pub struct OpenAiInference;

impl Inference for OpenAiInference {
    fn policy_yaml(&self, agent_binary: &str, base_url: Option<&str>) -> String {
        if let Some((host, port)) = base_url.and_then(super::parse_host_port) {
            format!(
                r#"version: 1
network_policies:
  openai:
    name: openai
    endpoints:
      - {{ host: {host}, port: {port}, protocol: rest, enforcement: enforce, access: full, tls: terminate }}
    binaries:
      - {{ path: {agent_binary} }}
"#
            )
        } else {
            format!(
                r#"version: 1
network_policies:
  openai:
    name: openai
    endpoints:
      - {{ host: api.openai.com, port: 443, protocol: rest, enforcement: enforce, access: full, tls: terminate }}
    binaries:
      - {{ path: {agent_binary} }}
"#
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_yaml_contains_openai_endpoint() {
        assert!(
            OpenAiInference
                .policy_yaml("/sandbox/.local/bin/opencode", None)
                .contains("api.openai.com")
        );
    }

    #[test]
    fn policy_yaml_embeds_agent_binary() {
        let yaml = OpenAiInference.policy_yaml("/sandbox/.local/bin/opencode", None);
        assert!(yaml.contains("/sandbox/.local/bin/opencode"));
    }

    #[test]
    fn policy_yaml_has_openai_name() {
        assert!(
            OpenAiInference
                .policy_yaml("/sandbox/.local/bin/opencode", None)
                .contains("name: openai")
        );
    }

    #[test]
    fn policy_yaml_with_custom_endpoint_uses_proxy_host() {
        let yaml =
            OpenAiInference.policy_yaml("/binary", Some("https://my-openai-proxy.example.com"));
        assert!(yaml.contains("my-openai-proxy.example.com"));
        assert!(yaml.contains("443"));
    }

    #[test]
    fn policy_yaml_with_custom_endpoint_omits_default_openai_host() {
        let yaml =
            OpenAiInference.policy_yaml("/binary", Some("https://my-openai-proxy.example.com"));
        assert!(!yaml.contains("api.openai.com"));
    }

    #[test]
    fn policy_yaml_with_custom_endpoint_custom_port() {
        let yaml = OpenAiInference
            .policy_yaml("/binary", Some("https://my-openai-proxy.example.com:8443"));
        assert!(yaml.contains("my-openai-proxy.example.com"));
        assert!(yaml.contains("8443"));
    }
}
