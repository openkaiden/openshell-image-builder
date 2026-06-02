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

use super::Agent;

pub struct ClaudeAgent;

impl Agent for ClaudeAgent {
    fn id(&self) -> &str {
        "claude"
    }

    fn install(&self) -> String {
        "RUN curl -fsSL https://claude.ai/install.sh | bash\nENV PATH=/sandbox/.local/bin:$PATH"
            .to_string()
    }

    fn binary_path(&self) -> &str {
        "/sandbox/.local/bin/claude"
    }

    fn policy_yaml(&self) -> &str {
        r#"version: 1
network_policies:
  claude_code:
    name: claude-code
    endpoints:
      - { host: raw.githubusercontent.com, port: 443 }
      - { host: platform.claude.com, port: 443 }
      - { host: api.github.com, port: 443, protocol: rest, tls: terminate, enforcement: enforce, access: read-only }
    binaries:
      - { path: /sandbox/.local/bin/claude }
"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_id_is_claude() {
        assert_eq!(ClaudeAgent.id(), "claude");
    }

    #[test]
    fn install_is_nonempty() {
        assert!(!ClaudeAgent.install().is_empty());
    }

    #[test]
    fn install_contains_claude_installer() {
        assert!(
            ClaudeAgent
                .install()
                .contains("https://claude.ai/install.sh")
        );
    }

    #[test]
    fn install_adds_local_bin_to_path() {
        assert!(
            ClaudeAgent
                .install()
                .contains("ENV PATH=/sandbox/.local/bin:$PATH")
        );
    }

    #[test]
    fn binary_path_is_local_bin_claude() {
        assert_eq!(ClaudeAgent.binary_path(), "/sandbox/.local/bin/claude");
    }

    #[test]
    fn policy_yaml_has_claude_code_name() {
        assert!(ClaudeAgent.policy_yaml().contains("name: claude-code"));
    }

    #[test]
    fn policy_yaml_has_platform_claude_endpoint() {
        assert!(ClaudeAgent.policy_yaml().contains("platform.claude.com"));
    }
}
