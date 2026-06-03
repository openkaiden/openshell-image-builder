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

pub(super) fn configure(
    mut files: HashMap<String, String>,
    base_url: &str,
    model: Option<&str>,
) -> HashMap<String, String> {
    let models = if let Some(m) = model {
        serde_json::json!({ m: { "tools": true } })
    } else {
        serde_json::json!({
            "lfm2.5":          { "tools": true },
            "qwen3-coder:30b": { "tools": true }
        })
    };
    let mut config = serde_json::json!({
        "$schema": "https://opencode.ai/config.json",
        "provider": {
            "ollama": {
                "npm": "@ai-sdk/openai-compatible",
                "options": { "baseURL": base_url },
                "models": models
            }
        }
    });
    if let Some(m) = model {
        config["model"] = serde_json::json!(format!("ollama/{m}"));
    }
    files.insert(
        ".config/opencode/config.json".to_string(),
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
            "http://host.openshell.internal:11434/v1",
            None,
        );
        assert!(result.contains_key(".config/opencode/config.json"));
    }

    #[test]
    fn configure_config_contains_base_url() {
        let result = configure(
            HashMap::new(),
            "http://host.openshell.internal:11434/v1",
            None,
        );
        let config = result.get(".config/opencode/config.json").unwrap();
        assert!(config.contains("http://host.openshell.internal:11434/v1"));
    }

    #[test]
    fn configure_config_contains_ollama_provider() {
        let result = configure(
            HashMap::new(),
            "http://host.openshell.internal:11434/v1",
            None,
        );
        let config = result.get(".config/opencode/config.json").unwrap();
        assert!(config.contains("ollama"));
        assert!(config.contains("@ai-sdk/openai-compatible"));
    }

    #[test]
    fn configure_without_model_keeps_preset_models() {
        let result = configure(
            HashMap::new(),
            "http://host.openshell.internal:11434/v1",
            None,
        );
        let config: serde_json::Value =
            serde_json::from_str(result.get(".config/opencode/config.json").unwrap()).unwrap();
        let models = &config["provider"]["ollama"]["models"];
        assert!(models["lfm2.5"]["tools"].as_bool().unwrap());
        assert!(models["qwen3-coder:30b"]["tools"].as_bool().unwrap());
    }

    #[test]
    fn configure_config_is_valid_json() {
        let result = configure(
            HashMap::new(),
            "http://host.openshell.internal:11434/v1",
            None,
        );
        let config = result.get(".config/opencode/config.json").unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(config).is_ok());
    }

    #[test]
    fn configure_with_model_sets_model_field_with_provider_prefix() {
        let result = configure(
            HashMap::new(),
            "http://host.openshell.internal:11434/v1",
            Some("qwen3-coder:30b"),
        );
        let config: serde_json::Value =
            serde_json::from_str(result.get(".config/opencode/config.json").unwrap()).unwrap();
        assert_eq!(config["model"], "ollama/qwen3-coder:30b");
    }

    #[test]
    fn configure_with_model_replaces_preset_models() {
        let result = configure(
            HashMap::new(),
            "http://host.openshell.internal:11434/v1",
            Some("my-custom-model"),
        );
        let config: serde_json::Value =
            serde_json::from_str(result.get(".config/opencode/config.json").unwrap()).unwrap();
        let models = &config["provider"]["ollama"]["models"];
        assert!(models["my-custom-model"]["tools"].as_bool().unwrap());
        assert!(models["lfm2.5"].is_null());
        assert!(models["qwen3-coder:30b"].is_null());
    }
}
