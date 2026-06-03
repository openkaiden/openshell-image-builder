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

pub(crate) const DEFAULT_BASE_URL: &str = "http://localhost:11434/v1";

pub struct OllamaInference;

impl Inference for OllamaInference {
    fn policy_yaml(&self, agent_binary: &str, base_url: Option<&str>) -> String {
        let (host, port) = base_url
            .and_then(super::parse_host_port)
            .unwrap_or_else(|| ("host.openshell.internal".to_string(), 11434));
        format!(
            r#"version: 1
network_policies:
  ollama:
    name: ollama
    endpoints:
      - {{ host: {host}, port: {port}, protocol: rest, enforcement: enforce, access: full }}
    binaries:
      - {{ path: {agent_binary} }}
"#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_yaml_contains_ollama_endpoint() {
        assert!(
            OllamaInference
                .policy_yaml("/sandbox/.local/bin/claude", None)
                .contains("host.openshell.internal")
        );
    }

    #[test]
    fn policy_yaml_contains_ollama_port() {
        assert!(
            OllamaInference
                .policy_yaml("/sandbox/.local/bin/claude", None)
                .contains("11434")
        );
    }

    #[test]
    fn policy_yaml_embeds_agent_binary() {
        let yaml = OllamaInference.policy_yaml("/sandbox/.local/bin/opencode", None);
        assert!(yaml.contains("/sandbox/.local/bin/opencode"));
    }

    #[test]
    fn policy_yaml_has_ollama_name() {
        assert!(
            OllamaInference
                .policy_yaml("/sandbox/.local/bin/claude", None)
                .contains("name: ollama")
        );
    }

    #[test]
    fn policy_yaml_with_custom_endpoint_uses_custom_host_and_port() {
        let yaml =
            OllamaInference.policy_yaml("/binary", Some("http://host.openshell.internal:9999/v1"));
        assert!(yaml.contains("host.openshell.internal"));
        assert!(yaml.contains("9999"));
        assert!(!yaml.contains("11434"));
    }

    #[test]
    fn policy_yaml_with_custom_endpoint_remote_host() {
        let yaml = OllamaInference
            .policy_yaml("/binary", Some("http://remote-ollama.example.com:11434/v1"));
        assert!(yaml.contains("remote-ollama.example.com"));
        assert!(!yaml.contains("host.openshell.internal"));
    }
}
