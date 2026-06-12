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

use std::collections::HashMap;

use kdn_workspace_configuration::McpConfiguration;

use super::Agent;
use crate::inference;

const CLAUDE_CONFIG_FILE: &str = ".claude.json";
const CLAUDE_SETTINGS_FILE: &str = ".claude/settings.json";
const SANDBOX_HOME: &str = "/sandbox";

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

    fn skip_onboarding(&self, mut files: HashMap<String, String>) -> HashMap<String, String> {
        let content = files
            .get(CLAUDE_CONFIG_FILE)
            .cloned()
            .unwrap_or_else(|| "{}".to_string());
        let mut config: serde_json::Value =
            serde_json::from_str(&content).unwrap_or(serde_json::json!({}));

        config["hasCompletedOnboarding"] = serde_json::json!(true);
        if !config["projects"].is_object() {
            config["projects"] = serde_json::json!({});
        }
        config["projects"][SANDBOX_HOME]["hasTrustDialogAccepted"] = serde_json::json!(true);

        files.insert(
            CLAUDE_CONFIG_FILE.to_string(),
            serde_json::to_string_pretty(&config).expect("valid json value"),
        );
        files
    }

    fn supported_inference(&self) -> Vec<inference::InferenceKind> {
        vec![
            inference::InferenceKind::Anthropic,
            inference::InferenceKind::VertexAi,
        ]
    }

    fn set_inference(
        &self,
        mut files: HashMap<String, String>,
        _inference: Option<&inference::InferenceKind>,
        _base_url: Option<&str>,
        model: Option<&str>,
    ) -> HashMap<String, String> {
        if let Some(m) = model {
            let content = files
                .get(CLAUDE_SETTINGS_FILE)
                .cloned()
                .unwrap_or_else(|| "{}".to_string());
            let mut config: serde_json::Value =
                serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
            config["model"] = serde_json::json!(m);
            files.insert(
                CLAUDE_SETTINGS_FILE.to_string(),
                serde_json::to_string_pretty(&config).expect("valid json value"),
            );
        }
        files
    }

    fn set_mcp_servers(
        &self,
        mut files: HashMap<String, String>,
        mcp: Option<&McpConfiguration>,
    ) -> HashMap<String, String> {
        let Some(mcp) = mcp else { return files };
        let content = files
            .get(CLAUDE_CONFIG_FILE)
            .cloned()
            .unwrap_or_else(|| "{}".to_string());
        let mut config: serde_json::Value =
            serde_json::from_str(&content).unwrap_or(serde_json::json!({}));

        let mut mcp_servers = config
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        for cmd in &mcp.commands {
            mcp_servers.insert(
                cmd.name.clone(),
                serde_json::json!({
                    "type": "stdio",
                    "command": cmd.command,
                    "args": cmd.args,
                    "env": cmd.env,
                }),
            );
        }
        for srv in &mcp.servers {
            let mut entry = serde_json::json!({
                "type": "sse",
                "url": srv.url,
            });
            if !srv.headers.is_empty() {
                entry["headers"] = serde_json::json!(srv.headers);
            }
            mcp_servers.insert(srv.name.clone(), entry);
        }

        config["mcpServers"] = serde_json::Value::Object(mcp_servers);
        files.insert(
            CLAUDE_CONFIG_FILE.to_string(),
            serde_json::to_string_pretty(&config).expect("valid json value"),
        );
        files
    }

    fn env_vars(
        &self,
        inference: Option<&inference::InferenceKind>,
        endpoint: Option<&str>,
        _model: Option<&str>,
    ) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        if let (Some(inference::InferenceKind::Anthropic), Some(url)) = (inference, endpoint) {
            vars.insert("ANTHROPIC_BASE_URL".to_string(), url.to_string());
        }
        vars
    }

    fn skills_dir(&self) -> &str {
        "/sandbox/.claude/skills"
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
    use std::collections::HashMap;

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

    #[test]
    fn skip_onboarding_creates_claude_json_when_absent() {
        let result = ClaudeAgent.skip_onboarding(HashMap::new());
        assert!(result.contains_key(CLAUDE_CONFIG_FILE));
    }

    #[test]
    fn skip_onboarding_sets_completed_onboarding_flag() {
        let result = ClaudeAgent.skip_onboarding(HashMap::new());
        let json: serde_json::Value =
            serde_json::from_str(result[CLAUDE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(json["hasCompletedOnboarding"], true);
    }

    #[test]
    fn skip_onboarding_sets_trust_dialog_accepted_for_sandbox_home() {
        let result = ClaudeAgent.skip_onboarding(HashMap::new());
        let json: serde_json::Value =
            serde_json::from_str(result[CLAUDE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(
            json["projects"][SANDBOX_HOME]["hasTrustDialogAccepted"],
            true
        );
    }

    #[test]
    fn skip_onboarding_preserves_existing_fields() {
        let mut files = HashMap::new();
        files.insert(
            CLAUDE_CONFIG_FILE.to_string(),
            r#"{"existingField": "value"}"#.to_string(),
        );
        let result = ClaudeAgent.skip_onboarding(files);
        let json: serde_json::Value =
            serde_json::from_str(result[CLAUDE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(json["existingField"], "value");
    }

    #[test]
    fn skills_dir_is_claude_skills() {
        assert_eq!(ClaudeAgent.skills_dir(), "/sandbox/.claude/skills");
    }

    #[test]
    fn supported_inference_includes_anthropic() {
        assert!(
            ClaudeAgent
                .supported_inference()
                .contains(&inference::InferenceKind::Anthropic)
        );
    }

    #[test]
    fn supported_inference_includes_vertexai() {
        assert!(
            ClaudeAgent
                .supported_inference()
                .contains(&inference::InferenceKind::VertexAi)
        );
    }

    #[test]
    fn supported_inference_excludes_ollama() {
        assert!(
            !ClaudeAgent
                .supported_inference()
                .contains(&inference::InferenceKind::Ollama)
        );
    }

    #[test]
    fn env_vars_with_anthropic_and_endpoint_sets_anthropic_base_url() {
        let vars = ClaudeAgent.env_vars(
            Some(&inference::InferenceKind::Anthropic),
            Some("https://my-proxy.example.com"),
            None,
        );
        assert_eq!(
            vars.get("ANTHROPIC_BASE_URL").map(String::as_str),
            Some("https://my-proxy.example.com")
        );
    }

    #[test]
    fn env_vars_with_anthropic_and_no_endpoint_returns_empty() {
        let vars = ClaudeAgent.env_vars(Some(&inference::InferenceKind::Anthropic), None, None);
        assert!(vars.is_empty());
    }

    #[test]
    fn env_vars_with_no_inference_returns_empty() {
        let vars = ClaudeAgent.env_vars(None, Some("https://my-proxy.example.com"), None);
        assert!(vars.is_empty());
    }

    #[test]
    fn set_inference_with_model_writes_model_to_claude_settings() {
        let result = ClaudeAgent.set_inference(HashMap::new(), None, None, Some("claude-opus-4-5"));
        let json: serde_json::Value =
            serde_json::from_str(result[CLAUDE_SETTINGS_FILE].as_str()).unwrap();
        assert_eq!(json["model"], "claude-opus-4-5");
    }

    #[test]
    fn set_inference_with_model_preserves_existing_claude_settings_fields() {
        let mut files = HashMap::new();
        files.insert(
            CLAUDE_SETTINGS_FILE.to_string(),
            r#"{"theme": "dark"}"#.to_string(),
        );
        let result = ClaudeAgent.set_inference(files, None, None, Some("claude-opus-4-5"));
        let json: serde_json::Value =
            serde_json::from_str(result[CLAUDE_SETTINGS_FILE].as_str()).unwrap();
        assert_eq!(json["model"], "claude-opus-4-5");
        assert_eq!(json["theme"], "dark");
    }

    #[test]
    fn set_inference_without_model_returns_files_unchanged() {
        let mut files = HashMap::new();
        files.insert("other.json".to_string(), "content".to_string());
        let result = ClaudeAgent.set_inference(files.clone(), None, None, None);
        assert_eq!(result, files);
    }

    #[test]
    fn skip_onboarding_leaves_other_files_unchanged() {
        let mut files = HashMap::new();
        files.insert("other.json".to_string(), "content".to_string());
        let result = ClaudeAgent.skip_onboarding(files);
        assert_eq!(result["other.json"], "content");
    }

    // set_mcp_servers

    fn make_mcp_command(name: &str, command: &str) -> kdn_workspace_configuration::McpCommand {
        kdn_workspace_configuration::McpCommand {
            name: name.to_string(),
            command: command.to_string(),
            args: vec![],
            env: Default::default(),
        }
    }

    fn make_mcp_server(name: &str, url: &str) -> kdn_workspace_configuration::McpServer {
        kdn_workspace_configuration::McpServer {
            name: name.to_string(),
            url: url.to_string(),
            headers: Default::default(),
        }
    }

    fn make_mcp(
        commands: Vec<kdn_workspace_configuration::McpCommand>,
        servers: Vec<kdn_workspace_configuration::McpServer>,
    ) -> kdn_workspace_configuration::McpConfiguration {
        kdn_workspace_configuration::McpConfiguration { commands, servers }
    }

    #[test]
    fn set_mcp_servers_with_none_returns_files_unchanged() {
        let mut files = HashMap::new();
        files.insert("other.json".to_string(), "content".to_string());
        let result = ClaudeAgent.set_mcp_servers(files.clone(), None);
        assert_eq!(result, files);
    }

    #[test]
    fn set_mcp_servers_with_command_writes_stdio_entry() {
        let mcp = make_mcp(vec![make_mcp_command("my-server", "npx")], vec![]);
        let result = ClaudeAgent.set_mcp_servers(HashMap::new(), Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[CLAUDE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(json["mcpServers"]["my-server"]["type"], "stdio");
        assert_eq!(json["mcpServers"]["my-server"]["command"], "npx");
    }

    #[test]
    fn set_mcp_servers_with_command_writes_args_and_env() {
        let mut cmd = make_mcp_command("srv", "node");
        cmd.args = vec!["server.js".to_string()];
        cmd.env.insert("TOKEN".to_string(), "abc".to_string());
        let mcp = make_mcp(vec![cmd], vec![]);
        let result = ClaudeAgent.set_mcp_servers(HashMap::new(), Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[CLAUDE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(json["mcpServers"]["srv"]["args"][0], "server.js");
        assert_eq!(json["mcpServers"]["srv"]["env"]["TOKEN"], "abc");
    }

    #[test]
    fn set_mcp_servers_with_server_writes_sse_entry() {
        let mcp = make_mcp(
            vec![],
            vec![make_mcp_server("remote", "https://mcp.example.com")],
        );
        let result = ClaudeAgent.set_mcp_servers(HashMap::new(), Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[CLAUDE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(json["mcpServers"]["remote"]["type"], "sse");
        assert_eq!(
            json["mcpServers"]["remote"]["url"],
            "https://mcp.example.com"
        );
    }

    #[test]
    fn set_mcp_servers_with_server_omits_headers_when_empty() {
        let mcp = make_mcp(
            vec![],
            vec![make_mcp_server("remote", "https://mcp.example.com")],
        );
        let result = ClaudeAgent.set_mcp_servers(HashMap::new(), Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[CLAUDE_CONFIG_FILE].as_str()).unwrap();
        assert!(json["mcpServers"]["remote"]["headers"].is_null());
    }

    #[test]
    fn set_mcp_servers_with_server_writes_headers_when_present() {
        let mut srv = make_mcp_server("remote", "https://mcp.example.com");
        srv.headers
            .insert("Authorization".to_string(), "Bearer tok".to_string());
        let mcp = make_mcp(vec![], vec![srv]);
        let result = ClaudeAgent.set_mcp_servers(HashMap::new(), Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[CLAUDE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(
            json["mcpServers"]["remote"]["headers"]["Authorization"],
            "Bearer tok"
        );
    }

    #[test]
    fn set_mcp_servers_preserves_existing_claude_json_fields() {
        let mut files = HashMap::new();
        files.insert(
            CLAUDE_CONFIG_FILE.to_string(),
            r#"{"hasCompletedOnboarding": true}"#.to_string(),
        );
        let mcp = make_mcp(vec![make_mcp_command("s", "cmd")], vec![]);
        let result = ClaudeAgent.set_mcp_servers(files, Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[CLAUDE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(json["hasCompletedOnboarding"], true);
        assert!(json["mcpServers"]["s"].is_object());
    }

    #[test]
    fn set_mcp_servers_merges_with_existing_mcp_servers() {
        let mut files = HashMap::new();
        files.insert(
            CLAUDE_CONFIG_FILE.to_string(),
            r#"{"mcpServers": {"existing": {"type": "sse", "url": "https://old.example.com"}}}"#
                .to_string(),
        );
        let mcp = make_mcp(vec![make_mcp_command("new-srv", "npx")], vec![]);
        let result = ClaudeAgent.set_mcp_servers(files, Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[CLAUDE_CONFIG_FILE].as_str()).unwrap();
        assert!(json["mcpServers"]["existing"].is_object());
        assert!(json["mcpServers"]["new-srv"].is_object());
    }
}
