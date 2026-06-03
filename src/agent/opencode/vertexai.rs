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
    model: &str,
) -> HashMap<String, String> {
    let config = serde_json::json!({
        "$schema": "https://opencode.ai/config.json",
        "model": model
    });
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
        let result = configure(HashMap::new(), "vertex/claude-opus-4-5");
        assert!(result.contains_key(".config/opencode/config.json"));
    }

    #[test]
    fn configure_sets_model_field() {
        let result = configure(HashMap::new(), "vertex/claude-opus-4-5");
        let config: serde_json::Value =
            serde_json::from_str(result.get(".config/opencode/config.json").unwrap()).unwrap();
        assert_eq!(config["model"], "vertex/claude-opus-4-5");
    }

    #[test]
    fn configure_config_is_valid_json() {
        let result = configure(HashMap::new(), "vertex/claude-opus-4-5");
        let config = result.get(".config/opencode/config.json").unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(config).is_ok());
    }
}
