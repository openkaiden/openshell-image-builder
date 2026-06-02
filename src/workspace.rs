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

use std::path::Path;

pub use kdn_workspace_configuration::WorkspaceConfiguration;

const WORKSPACE_PATH: &str = ".kaiden/workspace.json";

pub(crate) fn load_from(
    base: &Path,
) -> Result<Option<WorkspaceConfiguration>, Box<dyn std::error::Error>> {
    let path = base.join(WORKSPACE_PATH);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let workspace: WorkspaceConfiguration = serde_json::from_str(&content)?;
    log::info!("Workspace loaded from {}", path.display());
    Ok(Some(workspace))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_workspace_dir(content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let kaiden = dir.path().join(".kaiden");
        std::fs::create_dir_all(&kaiden).unwrap();
        std::fs::write(kaiden.join("workspace.json"), content.as_bytes()).unwrap();
        dir
    }

    #[test]
    fn load_returns_none_when_no_workspace_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_from(dir.path()).unwrap().is_none());
    }

    #[test]
    fn load_fails_on_malformed_json() {
        let dir = make_workspace_dir("not json");
        assert!(load_from(dir.path()).is_err());
    }

    #[test]
    fn load_parses_oci_and_local_feature_refs() {
        let dir = make_workspace_dir(
            r#"{"features": {"ghcr.io/devcontainers/features/rust:1": {}, "./tools/my-feature": {"version": "1.0"}}}"#,
        );
        let ws = load_from(dir.path()).unwrap().unwrap();
        assert!(
            ws.features
                .contains_key("ghcr.io/devcontainers/features/rust:1")
        );
        assert!(ws.features.contains_key("./tools/my-feature"));
    }

    #[test]
    fn load_parses_empty_features() {
        let dir = make_workspace_dir(r#"{}"#);
        let ws = load_from(dir.path()).unwrap().unwrap();
        assert!(ws.features.is_empty());
    }

    #[test]
    fn load_parses_bool_and_string_options() {
        let dir = make_workspace_dir(
            r#"{"features": {"./my-feature": {"flag": true, "name": "hello"}}}"#,
        );
        let ws = load_from(dir.path()).unwrap().unwrap();
        let opts = &ws.features["./my-feature"];
        assert_eq!(opts["flag"], serde_json::Value::Bool(true));
        assert_eq!(opts["name"], serde_json::Value::String("hello".into()));
    }
}
