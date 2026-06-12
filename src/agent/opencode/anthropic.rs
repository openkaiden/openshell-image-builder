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

use super::OPENCODE_CONFIG_FILE;

pub(super) fn configure(
    mut files: HashMap<String, String>,
    base_url: Option<&str>,
    model: Option<&str>,
) -> HashMap<String, String> {
    let mut anthropic_config = serde_json::json!({});
    if let Some(url) = base_url {
        anthropic_config["options"] = serde_json::json!({ "baseURL": url });
    }
    let mut config = serde_json::json!({
        "$schema": "https://opencode.ai/config.json",
        "provider": { "anthropic": anthropic_config }
    });
    if let Some(m) = model {
        config["model"] = serde_json::json!(m);
    }
    files.insert(
        OPENCODE_CONFIG_FILE.to_string(),
        serde_json::to_string_pretty(&config).expect("valid json value"),
    );
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configure_creates_opencode_config() {
        let result = configure(
            HashMap::new(),
            Some("https://my-anthropic-proxy.example.com"),
            None,
        );
        assert!(result.contains_key(OPENCODE_CONFIG_FILE));
    }

    #[test]
    fn configure_config_contains_base_url() {
        let result = configure(
            HashMap::new(),
            Some("https://my-anthropic-proxy.example.com"),
            None,
        );
        let config = result.get(OPENCODE_CONFIG_FILE).unwrap();
        assert!(config.contains("https://my-anthropic-proxy.example.com"));
    }

    #[test]
    fn configure_config_contains_anthropic_provider() {
        let result = configure(
            HashMap::new(),
            Some("https://my-anthropic-proxy.example.com"),
            None,
        );
        let config: serde_json::Value =
            serde_json::from_str(result.get(OPENCODE_CONFIG_FILE).unwrap()).unwrap();
        assert!(config["provider"]["anthropic"].is_object());
    }

    #[test]
    fn configure_config_is_valid_json() {
        let result = configure(
            HashMap::new(),
            Some("https://my-anthropic-proxy.example.com"),
            None,
        );
        let config = result.get(OPENCODE_CONFIG_FILE).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(config).is_ok());
    }

    #[test]
    fn configure_with_model_sets_model_field() {
        let result = configure(
            HashMap::new(),
            Some("https://my-anthropic-proxy.example.com"),
            Some("claude-opus-4-5"),
        );
        let config: serde_json::Value =
            serde_json::from_str(result.get(OPENCODE_CONFIG_FILE).unwrap()).unwrap();
        assert_eq!(config["model"], "claude-opus-4-5");
    }

    #[test]
    fn configure_without_url_sets_model_only() {
        let result = configure(HashMap::new(), None, Some("claude-opus-4-5"));
        let config: serde_json::Value =
            serde_json::from_str(result.get(OPENCODE_CONFIG_FILE).unwrap()).unwrap();
        assert_eq!(config["model"], "claude-opus-4-5");
        assert!(config["provider"]["anthropic"]["options"].is_null());
    }

    #[test]
    fn configure_with_url_and_model_sets_both() {
        let result = configure(
            HashMap::new(),
            Some("https://my-anthropic-proxy.example.com"),
            Some("claude-opus-4-5"),
        );
        let config: serde_json::Value =
            serde_json::from_str(result.get(OPENCODE_CONFIG_FILE).unwrap()).unwrap();
        assert_eq!(config["model"], "claude-opus-4-5");
        assert_eq!(
            config["provider"]["anthropic"]["options"]["baseURL"],
            "https://my-anthropic-proxy.example.com"
        );
    }
}
