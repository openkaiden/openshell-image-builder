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
    let mut config = serde_json::json!({
        "$schema": "https://opencode.ai/config.json"
    });
    if let Some(url) = base_url {
        // Custom endpoint: route through @ai-sdk/openai-compatible as a "custom" provider.
        let model_id = model.unwrap_or("gpt-4o");
        config["provider"] = serde_json::json!({
            "custom": {
                "name": "openai-compatible",
                "npm": "@ai-sdk/openai-compatible",
                "options": { "baseURL": url },
                "models": { model_id: { "_launch": true, "name": model_id } }
            }
        });
        config["model"] = serde_json::json!(format!("custom/{model_id}"));
    } else if let Some(m) = model {
        // Native OpenAI: opencode has built-in support, just set the model.
        config["model"] = serde_json::json!(format!("openai/{m}"));
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
    fn configure_with_model_creates_opencode_config() {
        let result = configure(HashMap::new(), None, Some("gpt-4o"));
        assert!(result.contains_key(OPENCODE_CONFIG_FILE));
    }

    #[test]
    fn configure_with_model_sets_native_openai_prefix() {
        let result = configure(HashMap::new(), None, Some("gpt-4o"));
        let config: serde_json::Value =
            serde_json::from_str(result.get(OPENCODE_CONFIG_FILE).unwrap()).unwrap();
        assert_eq!(config["model"], "openai/gpt-4o");
    }

    #[test]
    fn configure_with_model_does_not_add_provider_block() {
        let result = configure(HashMap::new(), None, Some("gpt-4o"));
        let config: serde_json::Value =
            serde_json::from_str(result.get(OPENCODE_CONFIG_FILE).unwrap()).unwrap();
        assert!(config["provider"].is_null());
    }

    #[test]
    fn configure_with_endpoint_creates_opencode_config() {
        let result = configure(
            HashMap::new(),
            Some("https://my-openai-proxy.example.com"),
            None,
        );
        assert!(result.contains_key(OPENCODE_CONFIG_FILE));
    }

    #[test]
    fn configure_with_endpoint_uses_custom_provider() {
        let result = configure(
            HashMap::new(),
            Some("https://my-openai-proxy.example.com"),
            None,
        );
        let config: serde_json::Value =
            serde_json::from_str(result.get(OPENCODE_CONFIG_FILE).unwrap()).unwrap();
        assert!(config["provider"]["custom"].is_object());
        assert_eq!(
            config["provider"]["custom"]["npm"],
            "@ai-sdk/openai-compatible"
        );
    }

    #[test]
    fn configure_with_endpoint_embeds_base_url() {
        let result = configure(
            HashMap::new(),
            Some("https://my-openai-proxy.example.com"),
            None,
        );
        let config = result.get(OPENCODE_CONFIG_FILE).unwrap();
        assert!(config.contains("https://my-openai-proxy.example.com"));
    }

    #[test]
    fn configure_with_endpoint_and_model_sets_custom_model_prefix() {
        let result = configure(
            HashMap::new(),
            Some("https://my-openai-proxy.example.com"),
            Some("gpt-4o"),
        );
        let config: serde_json::Value =
            serde_json::from_str(result.get(OPENCODE_CONFIG_FILE).unwrap()).unwrap();
        assert_eq!(config["model"], "custom/gpt-4o");
    }

    #[test]
    fn configure_with_endpoint_without_model_defaults_to_gpt4o() {
        let result = configure(
            HashMap::new(),
            Some("https://my-openai-proxy.example.com"),
            None,
        );
        let config: serde_json::Value =
            serde_json::from_str(result.get(OPENCODE_CONFIG_FILE).unwrap()).unwrap();
        assert_eq!(config["model"], "custom/gpt-4o");
        assert!(config["provider"]["custom"]["models"]["gpt-4o"].is_object());
    }

    #[test]
    fn configure_config_is_valid_json() {
        let result = configure(HashMap::new(), None, Some("gpt-4o"));
        let config = result.get(OPENCODE_CONFIG_FILE).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(config).is_ok());
    }
}
