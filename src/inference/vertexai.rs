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

pub struct VertexAiInference;

impl Inference for VertexAiInference {
    fn policy_yaml(&self, agent_binary: &str, _base_url: Option<&str>) -> String {
        format!(
            r#"version: 1
network_policies:
  vertexai:
    name: vertexai
    endpoints:
      - {{ host: oauth2.googleapis.com, port: 443 }}
      - {{ host: aiplatform.googleapis.com, port: 443 }}
      - {{ host: "*-aiplatform.googleapis.com", port: 443 }}
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
    fn policy_yaml_contains_oauth2_endpoint() {
        assert!(
            VertexAiInference
                .policy_yaml("/sandbox/.local/bin/claude", None)
                .contains("oauth2.googleapis.com")
        );
    }

    #[test]
    fn policy_yaml_contains_aiplatform_endpoint() {
        assert!(
            VertexAiInference
                .policy_yaml("/sandbox/.local/bin/claude", None)
                .contains("aiplatform.googleapis.com")
        );
    }

    #[test]
    fn policy_yaml_contains_wildcard_aiplatform_endpoint() {
        assert!(
            VertexAiInference
                .policy_yaml("/sandbox/.local/bin/claude", None)
                .contains("*-aiplatform.googleapis.com")
        );
    }

    #[test]
    fn policy_yaml_embeds_agent_binary() {
        let yaml = VertexAiInference.policy_yaml("/sandbox/.local/bin/opencode", None);
        assert!(yaml.contains("/sandbox/.local/bin/opencode"));
    }

    #[test]
    fn policy_yaml_has_vertexai_name() {
        assert!(
            VertexAiInference
                .policy_yaml("/sandbox/.local/bin/claude", None)
                .contains("name: vertexai")
        );
    }
}
