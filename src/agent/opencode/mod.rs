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

mod anthropic;
mod ollama;
mod openai;
mod vertexai;

use std::collections::HashMap;

use kdn_workspace_configuration::McpConfiguration;

use super::Agent;
use crate::inference;

const OPENCODE_CONFIG_FILE: &str = ".config/opencode/config.json";

pub struct OpencodeAgent;

impl Agent for OpencodeAgent {
    fn id(&self) -> &str {
        "opencode"
    }

    fn install(&self) -> String {
        // The installer places the binary in ~/.opencode/bin/ which is not in PATH,
        // so we symlink it into ~/.local/bin/.
        "RUN cd /tmp && curl -fsSL https://opencode.ai/install | bash && \\\n    \
             mkdir -p /sandbox/.local/bin && \\\n    \
             ln -sf /sandbox/.opencode/bin/opencode /sandbox/.local/bin/opencode && \\\n    \
             mkdir -p /sandbox/.config/opencode\n\
         ENV PATH=/sandbox/.local/bin:$PATH"
            .to_string()
    }

    fn binary_path(&self) -> &str {
        "/sandbox/.local/bin/opencode"
    }

    fn supported_inference(&self) -> Vec<inference::InferenceKind> {
        vec![
            inference::InferenceKind::Anthropic,
            inference::InferenceKind::Ollama,
            inference::InferenceKind::OpenAi,
            inference::InferenceKind::VertexAi,
        ]
    }

    fn set_inference(
        &self,
        files: HashMap<String, String>,
        inference: Option<&inference::InferenceKind>,
        base_url: Option<&str>,
        model: Option<&str>,
    ) -> HashMap<String, String> {
        match inference {
            Some(inference::InferenceKind::Ollama) if base_url.is_some() => {
                ollama::configure(files, base_url.unwrap(), model)
            }
            Some(inference::InferenceKind::Anthropic) if base_url.is_some() || model.is_some() => {
                anthropic::configure(files, base_url, model)
            }
            Some(inference::InferenceKind::OpenAi) if base_url.is_some() || model.is_some() => {
                openai::configure(files, base_url, model)
            }
            Some(inference::InferenceKind::VertexAi) if model.is_some() => {
                vertexai::configure(files, model.unwrap())
            }
            _ => files,
        }
    }

    fn skills_dir(&self) -> &str {
        "/sandbox/.opencode/skills"
    }

    fn set_mcp_servers(
        &self,
        mut files: HashMap<String, String>,
        mcp: Option<&McpConfiguration>,
    ) -> HashMap<String, String> {
        let Some(mcp) = mcp else { return files };
        let content = files.get(OPENCODE_CONFIG_FILE).cloned().unwrap_or_else(|| {
            serde_json::json!({ "$schema": "https://opencode.ai/config.json" }).to_string()
        });
        let mut config: serde_json::Value =
            serde_json::from_str(&content).unwrap_or(serde_json::json!({}));

        let mut mcp_map = config
            .get("mcp")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        for cmd in &mcp.commands {
            let command: Vec<serde_json::Value> = std::iter::once(cmd.command.as_str())
                .chain(cmd.args.iter().map(String::as_str))
                .map(|s| serde_json::json!(s))
                .collect();
            let mut entry = serde_json::json!({
                "type": "local",
                "command": command,
                "enabled": true,
            });
            if !cmd.env.is_empty() {
                entry["environment"] = serde_json::json!(cmd.env);
            }
            mcp_map.insert(cmd.name.clone(), entry);
        }
        for srv in &mcp.servers {
            let mut entry = serde_json::json!({
                "type": "remote",
                "url": srv.url,
                "enabled": true,
            });
            if !srv.headers.is_empty() {
                entry["headers"] = serde_json::json!(srv.headers);
            }
            mcp_map.insert(srv.name.clone(), entry);
        }

        config["mcp"] = serde_json::Value::Object(mcp_map);
        files.insert(
            OPENCODE_CONFIG_FILE.to_string(),
            serde_json::to_string_pretty(&config).expect("valid json value"),
        );
        files
    }

    fn policy_yaml(&self) -> &str {
        r#"version: 1
network_policies:
  opencode:
    name: opencode
    endpoints:
      - { host: models.dev, port: 443, protocol: rest, enforcement: enforce, access: full, tls: terminate }
      - { host: opencode.ai, port: 443, protocol: rest, enforcement: enforce, access: full, tls: terminate }
      - { host: registry.npmjs.org, port: 443, protocol: rest, enforcement: enforce, access: read-only, tls: terminate }
      - { host: api.github.com, port: 443, protocol: rest, tls: terminate, enforcement: enforce, access: read-only }
    binaries:
      - { path: /sandbox/.local/bin/opencode }
"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_id_is_opencode() {
        assert_eq!(OpencodeAgent.id(), "opencode");
    }

    #[test]
    fn install_is_nonempty() {
        assert!(!OpencodeAgent.install().is_empty());
    }

    #[test]
    fn install_contains_opencode_installer() {
        assert!(
            OpencodeAgent
                .install()
                .contains("https://opencode.ai/install")
        );
    }

    #[test]
    fn install_symlinks_binary_to_local_bin() {
        assert!(
            OpencodeAgent
                .install()
                .contains("/sandbox/.local/bin/opencode")
        );
    }

    #[test]
    fn install_adds_local_bin_to_path() {
        assert!(
            OpencodeAgent
                .install()
                .contains("ENV PATH=/sandbox/.local/bin:$PATH")
        );
    }

    #[test]
    fn binary_path_is_local_bin_opencode() {
        assert_eq!(OpencodeAgent.binary_path(), "/sandbox/.local/bin/opencode");
    }

    #[test]
    fn skills_dir_is_opencode_skills() {
        assert_eq!(OpencodeAgent.skills_dir(), "/sandbox/.opencode/skills");
    }

    #[test]
    fn policy_yaml_is_nonempty() {
        assert!(!OpencodeAgent.policy_yaml().is_empty());
    }

    #[test]
    fn policy_yaml_contains_opencode_endpoint() {
        assert!(OpencodeAgent.policy_yaml().contains("opencode.ai"));
    }

    #[test]
    fn policy_yaml_contains_npm_registry() {
        assert!(OpencodeAgent.policy_yaml().contains("registry.npmjs.org"));
    }

    #[test]
    fn policy_yaml_references_opencode_binary() {
        assert!(
            OpencodeAgent
                .policy_yaml()
                .contains("/sandbox/.local/bin/opencode")
        );
    }

    #[test]
    fn supported_inference_includes_anthropic() {
        assert!(
            OpencodeAgent
                .supported_inference()
                .contains(&inference::InferenceKind::Anthropic)
        );
    }

    #[test]
    fn supported_inference_includes_ollama() {
        assert!(
            OpencodeAgent
                .supported_inference()
                .contains(&inference::InferenceKind::Ollama)
        );
    }

    #[test]
    fn supported_inference_includes_vertexai() {
        assert!(
            OpencodeAgent
                .supported_inference()
                .contains(&inference::InferenceKind::VertexAi)
        );
    }

    #[test]
    fn set_inference_with_none_returns_files_unchanged() {
        let mut files = HashMap::new();
        files.insert("existing.json".to_string(), "content".to_string());
        let result = OpencodeAgent.set_inference(files.clone(), None, None, None);
        assert_eq!(result, files);
    }

    #[test]
    fn set_inference_with_ollama_creates_opencode_config() {
        let result = OpencodeAgent.set_inference(
            HashMap::new(),
            Some(&inference::InferenceKind::Ollama),
            Some("http://host.openshell.internal:11434/v1"),
            None,
        );
        assert!(result.contains_key(".config/opencode/config.json"));
    }

    #[test]
    fn set_inference_with_ollama_without_url_returns_files_unchanged() {
        let mut files = HashMap::new();
        files.insert("existing.json".to_string(), "content".to_string());
        let result = OpencodeAgent.set_inference(
            files.clone(),
            Some(&inference::InferenceKind::Ollama),
            None,
            None,
        );
        assert_eq!(result, files);
    }

    #[test]
    fn set_inference_with_anthropic_without_url_or_model_returns_files_unchanged() {
        let mut files = HashMap::new();
        files.insert("existing.json".to_string(), "content".to_string());
        let result = OpencodeAgent.set_inference(
            files.clone(),
            Some(&inference::InferenceKind::Anthropic),
            None,
            None,
        );
        assert_eq!(result, files);
    }

    #[test]
    fn set_inference_with_anthropic_and_url_creates_opencode_config() {
        let result = OpencodeAgent.set_inference(
            HashMap::new(),
            Some(&inference::InferenceKind::Anthropic),
            Some("https://my-anthropic-proxy.example.com"),
            None,
        );
        assert!(result.contains_key(".config/opencode/config.json"));
    }

    #[test]
    fn set_inference_with_anthropic_and_url_embeds_url() {
        let result = OpencodeAgent.set_inference(
            HashMap::new(),
            Some(&inference::InferenceKind::Anthropic),
            Some("https://my-anthropic-proxy.example.com"),
            None,
        );
        let config = result.get(".config/opencode/config.json").unwrap();
        assert!(config.contains("https://my-anthropic-proxy.example.com"));
    }

    #[test]
    fn set_inference_with_anthropic_and_model_creates_opencode_config() {
        let result = OpencodeAgent.set_inference(
            HashMap::new(),
            Some(&inference::InferenceKind::Anthropic),
            None,
            Some("claude-opus-4-5"),
        );
        assert!(result.contains_key(".config/opencode/config.json"));
    }

    #[test]
    fn set_inference_with_vertexai_and_model_creates_opencode_config() {
        let result = OpencodeAgent.set_inference(
            HashMap::new(),
            Some(&inference::InferenceKind::VertexAi),
            None,
            Some("vertex/claude-opus-4-5"),
        );
        assert!(result.contains_key(".config/opencode/config.json"));
    }

    #[test]
    fn set_inference_with_vertexai_without_model_returns_files_unchanged() {
        let mut files = HashMap::new();
        files.insert("existing.json".to_string(), "content".to_string());
        let result = OpencodeAgent.set_inference(
            files.clone(),
            Some(&inference::InferenceKind::VertexAi),
            None,
            None,
        );
        assert_eq!(result, files);
    }

    #[test]
    fn supported_inference_includes_openai() {
        assert!(
            OpencodeAgent
                .supported_inference()
                .contains(&inference::InferenceKind::OpenAi)
        );
    }

    #[test]
    fn set_inference_with_openai_and_model_creates_opencode_config() {
        let result = OpencodeAgent.set_inference(
            HashMap::new(),
            Some(&inference::InferenceKind::OpenAi),
            None,
            Some("gpt-4o"),
        );
        assert!(result.contains_key(".config/opencode/config.json"));
    }

    #[test]
    fn set_inference_with_openai_and_endpoint_creates_opencode_config() {
        let result = OpencodeAgent.set_inference(
            HashMap::new(),
            Some(&inference::InferenceKind::OpenAi),
            Some("https://my-openai-proxy.example.com"),
            None,
        );
        assert!(result.contains_key(".config/opencode/config.json"));
    }

    #[test]
    fn set_inference_with_openai_without_url_or_model_returns_files_unchanged() {
        let mut files = HashMap::new();
        files.insert("existing.json".to_string(), "content".to_string());
        let result = OpencodeAgent.set_inference(
            files.clone(),
            Some(&inference::InferenceKind::OpenAi),
            None,
            None,
        );
        assert_eq!(result, files);
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
        let result = OpencodeAgent.set_mcp_servers(files.clone(), None);
        assert_eq!(result, files);
    }

    #[test]
    fn set_mcp_servers_with_command_writes_local_entry() {
        let mcp = make_mcp(vec![make_mcp_command("my-mcp", "npx")], vec![]);
        let result = OpencodeAgent.set_mcp_servers(HashMap::new(), Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[OPENCODE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(json["mcp"]["my-mcp"]["type"], "local");
        assert_eq!(json["mcp"]["my-mcp"]["command"][0], "npx");
        assert_eq!(json["mcp"]["my-mcp"]["enabled"], true);
    }

    #[test]
    fn set_mcp_servers_with_command_merges_args_into_command_list() {
        let mut cmd = make_mcp_command("srv", "npx");
        cmd.args = vec!["-y".to_string(), "my-pkg".to_string()];
        let mcp = make_mcp(vec![cmd], vec![]);
        let result = OpencodeAgent.set_mcp_servers(HashMap::new(), Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[OPENCODE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(json["mcp"]["srv"]["command"][0], "npx");
        assert_eq!(json["mcp"]["srv"]["command"][1], "-y");
        assert_eq!(json["mcp"]["srv"]["command"][2], "my-pkg");
    }

    #[test]
    fn set_mcp_servers_with_command_env_writes_environment_field() {
        let mut cmd = make_mcp_command("srv", "node");
        cmd.env.insert("TOKEN".to_string(), "abc".to_string());
        let mcp = make_mcp(vec![cmd], vec![]);
        let result = OpencodeAgent.set_mcp_servers(HashMap::new(), Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[OPENCODE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(json["mcp"]["srv"]["environment"]["TOKEN"], "abc");
    }

    #[test]
    fn set_mcp_servers_with_command_empty_env_omits_environment_field() {
        let mcp = make_mcp(vec![make_mcp_command("srv", "cmd")], vec![]);
        let result = OpencodeAgent.set_mcp_servers(HashMap::new(), Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[OPENCODE_CONFIG_FILE].as_str()).unwrap();
        assert!(json["mcp"]["srv"]["environment"].is_null());
    }

    #[test]
    fn set_mcp_servers_with_server_writes_remote_entry() {
        let mcp = make_mcp(
            vec![],
            vec![make_mcp_server("remote", "https://mcp.example.com")],
        );
        let result = OpencodeAgent.set_mcp_servers(HashMap::new(), Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[OPENCODE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(json["mcp"]["remote"]["type"], "remote");
        assert_eq!(json["mcp"]["remote"]["url"], "https://mcp.example.com");
        assert_eq!(json["mcp"]["remote"]["enabled"], true);
    }

    #[test]
    fn set_mcp_servers_with_server_headers_writes_headers_field() {
        let mut srv = make_mcp_server("remote", "https://mcp.example.com");
        srv.headers
            .insert("Authorization".to_string(), "Bearer tok".to_string());
        let mcp = make_mcp(vec![], vec![srv]);
        let result = OpencodeAgent.set_mcp_servers(HashMap::new(), Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[OPENCODE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(
            json["mcp"]["remote"]["headers"]["Authorization"],
            "Bearer tok"
        );
    }

    #[test]
    fn set_mcp_servers_with_server_empty_headers_omits_headers_field() {
        let mcp = make_mcp(
            vec![],
            vec![make_mcp_server("remote", "https://mcp.example.com")],
        );
        let result = OpencodeAgent.set_mcp_servers(HashMap::new(), Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[OPENCODE_CONFIG_FILE].as_str()).unwrap();
        assert!(json["mcp"]["remote"]["headers"].is_null());
    }

    #[test]
    fn set_mcp_servers_preserves_existing_config_fields() {
        let mut files = HashMap::new();
        files.insert(
            OPENCODE_CONFIG_FILE.to_string(),
            r#"{"$schema":"https://opencode.ai/config.json","model":"claude-opus-4-5"}"#
                .to_string(),
        );
        let mcp = make_mcp(vec![make_mcp_command("s", "cmd")], vec![]);
        let result = OpencodeAgent.set_mcp_servers(files, Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[OPENCODE_CONFIG_FILE].as_str()).unwrap();
        assert_eq!(json["model"], "claude-opus-4-5");
        assert!(json["mcp"]["s"].is_object());
    }

    #[test]
    fn set_mcp_servers_merges_with_existing_mcp_entries() {
        let mut files = HashMap::new();
        files.insert(
            OPENCODE_CONFIG_FILE.to_string(),
            r#"{"mcp":{"old":{"type":"remote","url":"https://old.example.com","enabled":true}}}"#
                .to_string(),
        );
        let mcp = make_mcp(vec![make_mcp_command("new-srv", "npx")], vec![]);
        let result = OpencodeAgent.set_mcp_servers(files, Some(&mcp));
        let json: serde_json::Value =
            serde_json::from_str(result[OPENCODE_CONFIG_FILE].as_str()).unwrap();
        assert!(json["mcp"]["old"].is_object());
        assert!(json["mcp"]["new-srv"].is_object());
    }
}
