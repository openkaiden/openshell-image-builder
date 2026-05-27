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
use std::path::Path;

use serde::Deserialize;

use super::FeatureError;

#[derive(Deserialize)]
struct RawMetadata {
    #[serde(rename = "containerEnv", default)]
    container_env: HashMap<String, String>,
    #[serde(default)]
    options: HashMap<String, OptionSpec>,
    #[serde(rename = "installsAfter", default)]
    installs_after: Vec<String>,
}

#[derive(Deserialize, Clone)]
struct OptionSpec {
    #[serde(rename = "type", default)]
    kind: String,
    default: Option<serde_json::Value>,
    #[serde(default)]
    r#enum: Vec<String>,
}

pub struct FeatureMetadata {
    pub container_env: HashMap<String, String>,
    pub installs_after: Vec<String>,
    options: HashMap<String, OptionSpec>,
}

impl FeatureMetadata {
    pub fn merge_options(
        &self,
        user_opts: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<HashMap<String, String>, FeatureError> {
        let mut result = HashMap::new();

        // Apply spec defaults first.
        for (key, spec) in &self.options {
            if let Some(default) = &spec.default {
                let val = coerce_value(key, spec, default)?;
                result.insert(normalize_key(key), val);
            }
        }

        // Apply and validate user-supplied options.
        for (key, val) in user_opts {
            let spec = self
                .options
                .get(key)
                .ok_or_else(|| FeatureError::InvalidOption(format!("unknown option: {key}")))?;
            result.insert(normalize_key(key), coerce_value(key, spec, val)?);
        }

        Ok(result)
    }
}

/// Normalizes an option key to an env var name: uppercase, non-alphanumeric → `_`.
fn normalize_key(key: &str) -> String {
    let upper = key.to_uppercase();
    let mut result = String::with_capacity(upper.len());
    for ch in upper.chars() {
        if ch.is_alphanumeric() {
            result.push(ch);
        } else {
            result.push('_');
        }
    }
    result
}

fn coerce_value(
    key: &str,
    spec: &OptionSpec,
    val: &serde_json::Value,
) -> Result<String, FeatureError> {
    match spec.kind.as_str() {
        "boolean" => match val {
            serde_json::Value::Bool(b) => Ok(b.to_string()),
            serde_json::Value::String(s) => {
                let lower = s.to_lowercase();
                if lower == "true" || lower == "false" {
                    Ok(lower)
                } else {
                    Err(FeatureError::InvalidOption(format!(
                        "option {key}: expected boolean, got {s:?}"
                    )))
                }
            }
            other => Err(FeatureError::InvalidOption(format!(
                "option {key}: expected boolean, got {other}"
            ))),
        },
        // "string" and "" (unspecified) both treated as string.
        _ => match val {
            serde_json::Value::String(s) => {
                if !spec.r#enum.is_empty() && !spec.r#enum.contains(s) {
                    return Err(FeatureError::InvalidOption(format!(
                        "option {key}: value {s:?} is not in enum {:?}",
                        spec.r#enum
                    )));
                }
                Ok(s.clone())
            }
            other => Err(FeatureError::InvalidOption(format!(
                "option {key}: expected string, got {other}"
            ))),
        },
    }
}

#[cfg(test)]
impl FeatureMetadata {
    pub(super) fn for_test(
        container_env: HashMap<String, String>,
        installs_after: Vec<String>,
    ) -> Self {
        Self {
            container_env,
            installs_after,
            options: HashMap::new(),
        }
    }
}

pub fn parse(dir: &Path) -> Result<FeatureMetadata, FeatureError> {
    let path = dir.join("devcontainer-feature.json");
    let data = std::fs::read_to_string(&path)
        .map_err(|e| FeatureError::Io(format!("reading devcontainer-feature.json: {e}")))?;
    let raw: RawMetadata = serde_json::from_str(&data).map_err(|e| {
        FeatureError::InvalidMetadata(format!("parsing devcontainer-feature.json: {e}"))
    })?;
    Ok(FeatureMetadata {
        container_env: raw.container_env,
        installs_after: raw.installs_after,
        options: raw.options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_metadata(dir: &Path, json: &str) {
        let mut f = std::fs::File::create(dir.join("devcontainer-feature.json")).unwrap();
        f.write_all(json.as_bytes()).unwrap();
    }

    #[test]
    fn parse_minimal_metadata() {
        let dir = tempfile::tempdir().unwrap();
        write_metadata(dir.path(), r#"{"id":"foo"}"#);
        let meta = parse(dir.path()).unwrap();
        assert!(meta.container_env.is_empty());
        assert!(meta.installs_after.is_empty());
    }

    #[test]
    fn parse_full_metadata() {
        let dir = tempfile::tempdir().unwrap();
        write_metadata(
            dir.path(),
            r#"{
                "id": "foo",
                "containerEnv": {"FOO": "bar"},
                "installsAfter": ["ghcr.io/devcontainers/features/rust"]
            }"#,
        );
        let meta = parse(dir.path()).unwrap();
        assert_eq!(meta.container_env["FOO"], "bar");
        assert_eq!(meta.installs_after, ["ghcr.io/devcontainers/features/rust"]);
    }

    #[test]
    fn parse_missing_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(parse(dir.path()), Err(FeatureError::Io(_))));
    }

    #[test]
    fn parse_invalid_json_errors() {
        let dir = tempfile::tempdir().unwrap();
        write_metadata(dir.path(), "not json");
        assert!(matches!(
            parse(dir.path()),
            Err(FeatureError::InvalidMetadata(_))
        ));
    }

    #[test]
    fn normalize_key_uppercases_and_replaces_nonalnum() {
        assert_eq!(normalize_key("install-tools"), "INSTALL_TOOLS");
        assert_eq!(normalize_key("go.version"), "GO_VERSION");
        assert_eq!(normalize_key("version"), "VERSION");
    }

    #[test]
    fn merge_options_applies_defaults() {
        let dir = tempfile::tempdir().unwrap();
        write_metadata(
            dir.path(),
            r#"{"id":"x","options":{"version":{"type":"string","default":"stable"}}}"#,
        );
        let meta = parse(dir.path()).unwrap();
        let merged = meta.merge_options(&serde_json::Map::new()).unwrap();
        assert_eq!(merged["VERSION"], "stable");
    }

    #[test]
    fn merge_options_user_overrides_default() {
        let dir = tempfile::tempdir().unwrap();
        write_metadata(
            dir.path(),
            r#"{"id":"x","options":{"version":{"type":"string","default":"stable"}}}"#,
        );
        let meta = parse(dir.path()).unwrap();
        let mut user = serde_json::Map::new();
        user.insert(
            "version".into(),
            serde_json::Value::String("nightly".into()),
        );
        let merged = meta.merge_options(&user).unwrap();
        assert_eq!(merged["VERSION"], "nightly");
    }

    #[test]
    fn merge_options_bool_true() {
        let dir = tempfile::tempdir().unwrap();
        write_metadata(
            dir.path(),
            r#"{"id":"x","options":{"install_rustfmt":{"type":"boolean","default":false}}}"#,
        );
        let meta = parse(dir.path()).unwrap();
        let mut user = serde_json::Map::new();
        user.insert("install_rustfmt".into(), serde_json::Value::Bool(true));
        let merged = meta.merge_options(&user).unwrap();
        assert_eq!(merged["INSTALL_RUSTFMT"], "true");
    }

    #[test]
    fn merge_options_bool_string_accepted() {
        let dir = tempfile::tempdir().unwrap();
        write_metadata(
            dir.path(),
            r#"{"id":"x","options":{"flag":{"type":"boolean"}}}"#,
        );
        let meta = parse(dir.path()).unwrap();
        let mut user = serde_json::Map::new();
        user.insert("flag".into(), serde_json::Value::String("false".into()));
        let merged = meta.merge_options(&user).unwrap();
        assert_eq!(merged["FLAG"], "false");
    }

    #[test]
    fn merge_options_enum_valid() {
        let dir = tempfile::tempdir().unwrap();
        write_metadata(
            dir.path(),
            r#"{"id":"x","options":{"channel":{"type":"string","enum":["stable","nightly"]}}}"#,
        );
        let meta = parse(dir.path()).unwrap();
        let mut user = serde_json::Map::new();
        user.insert(
            "channel".into(),
            serde_json::Value::String("nightly".into()),
        );
        assert!(meta.merge_options(&user).is_ok());
    }

    #[test]
    fn merge_options_enum_invalid_errors() {
        let dir = tempfile::tempdir().unwrap();
        write_metadata(
            dir.path(),
            r#"{"id":"x","options":{"channel":{"type":"string","enum":["stable","nightly"]}}}"#,
        );
        let meta = parse(dir.path()).unwrap();
        let mut user = serde_json::Map::new();
        user.insert("channel".into(), serde_json::Value::String("beta".into()));
        assert!(matches!(
            meta.merge_options(&user),
            Err(FeatureError::InvalidOption(_))
        ));
    }

    #[test]
    fn merge_options_unknown_key_errors() {
        let dir = tempfile::tempdir().unwrap();
        write_metadata(dir.path(), r#"{"id":"x"}"#);
        let meta = parse(dir.path()).unwrap();
        let mut user = serde_json::Map::new();
        user.insert("unknown".into(), serde_json::Value::String("x".into()));
        assert!(matches!(
            meta.merge_options(&user),
            Err(FeatureError::InvalidOption(_))
        ));
    }
}
