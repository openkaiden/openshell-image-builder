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

mod local;
mod metadata;
mod oci;

use std::collections::HashMap;
use std::path::Path;

use kdn_workspace_configuration::WorkspaceConfiguration;

#[derive(Debug)]
pub enum FeatureError {
    InvalidId(String),
    InvalidOption(String),
    InvalidMetadata(String),
    Io(String),
    Oci(String),
}

impl std::fmt::Display for FeatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeatureError::InvalidId(msg) => write!(f, "invalid feature ID: {msg}"),
            FeatureError::InvalidOption(msg) => write!(f, "invalid feature option: {msg}"),
            FeatureError::InvalidMetadata(msg) => write!(f, "invalid feature metadata: {msg}"),
            FeatureError::Io(msg) => write!(f, "I/O error: {msg}"),
            FeatureError::Oci(msg) => write!(f, "OCI error: {msg}"),
        }
    }
}

impl std::error::Error for FeatureError {}

pub struct StagedFeature {
    pub id: String,
    /// Name of the directory under `context_dir/features/` (e.g. `feature-0`).
    pub dir_name: String,
    /// Option values as normalized env var names (e.g. `VERSION=stable`).
    pub merged_options: HashMap<String, String>,
    /// Persistent env vars to set in the image after installation.
    pub container_env: HashMap<String, String>,
}

enum FeatureSource {
    Local(local::LocalFeature),
    Oci(oci::OciFeature),
}

struct FeatureEntry {
    id: String,
    source: FeatureSource,
    user_opts: serde_json::Map<String, serde_json::Value>,
}

/// Strips version tag or digest from an OCI reference, returning the base ID.
/// Used to match `installsAfter` values (always versionless per spec) against
/// the feature IDs that may carry a tag.
fn feature_base_id(id: &str) -> &str {
    // Digest takes priority.
    if let Some(pos) = id.find('@') {
        return &id[..pos];
    }
    let first_slash = id.find('/').unwrap_or(0);
    if let Some(pos) = id.rfind(':')
        && pos > first_slash
    {
        return &id[..pos];
    }
    id
}

/// Builds a list of feature entries from the workspace `features` map, sorted
/// by ID for deterministic ordering before the topological sort.
/// Returns an error for `http://`/`https://` URIs.
fn build_entries(
    features: &HashMap<String, serde_json::Map<String, serde_json::Value>>,
    workspace_config_dir: &Path,
) -> Result<Vec<FeatureEntry>, FeatureError> {
    let mut entries: Vec<FeatureEntry> = features
        .iter()
        .map(|(id, opts)| {
            let source = if id.starts_with("./") || id.starts_with("../") {
                FeatureSource::Local(local::LocalFeature::new(id, workspace_config_dir)?)
            } else if id.starts_with("http://") || id.starts_with("https://") {
                return Err(FeatureError::InvalidId(format!(
                    "direct HTTP(S) feature sources are not supported: {id}"
                )));
            } else {
                FeatureSource::Oci(oci::OciFeature::new(id))
            };
            Ok(FeatureEntry {
                id: id.clone(),
                source,
                user_opts: opts.clone(),
            })
        })
        .collect::<Result<Vec<_>, FeatureError>>()?;

    entries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(entries)
}

/// Returns entries in installation order, respecting `installsAfter` from metadata.
/// Uses Kahn's topological sort. Silently ignores `installsAfter` references to
/// features not in the set (per spec). Errors on cycles.
fn topological_order(
    entries: Vec<FeatureEntry>,
    metadata_map: &HashMap<String, metadata::FeatureMetadata>,
) -> Result<Vec<FeatureEntry>, FeatureError> {
    // Build base-ID → full-ID lookup for installsAfter matching.
    // Owned Strings avoid borrowing `entries` across the later into_iter().
    let mut base_to_id: HashMap<String, String> = HashMap::new();
    for entry in &entries {
        let base = feature_base_id(&entry.id).to_string();
        if base_to_id.insert(base.clone(), entry.id.clone()).is_some() {
            return Err(FeatureError::InvalidId(format!(
                "features share base ID {base:?}; declare only one variant"
            )));
        }
    }

    // in_degree[id] = number of features that must be installed before it.
    // dependents[id] = features that depend on id being installed first.
    let mut in_degree: HashMap<String, usize> = entries.iter().map(|e| (e.id.clone(), 0)).collect();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    for entry in &entries {
        let meta = metadata_map
            .get(&entry.id)
            .ok_or_else(|| FeatureError::InvalidId(format!("missing metadata for {}", entry.id)))?;
        for dep in meta.installs_after.iter() {
            let dep_id = match base_to_id.get(dep.as_str()) {
                Some(id) => id.clone(),
                None => continue, // not in our set, ignore per spec
            };
            dependents.entry(dep_id).or_default().push(entry.id.clone());
            *in_degree.get_mut(&entry.id).unwrap() += 1;
        }
    }

    // Kahn's algorithm: start with all zero-in-degree entries (sorted for determinism).
    let mut queue: Vec<String> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(id, _)| id.clone())
        .collect();
    queue.sort_unstable();

    // Map id → entry for extraction.
    let mut entry_map: HashMap<String, FeatureEntry> =
        entries.into_iter().map(|e| (e.id.clone(), e)).collect();

    let mut ordered: Vec<FeatureEntry> = Vec::with_capacity(entry_map.len());
    while let Some(id) = queue.first().cloned() {
        queue.remove(0);
        ordered.push(entry_map.remove(&id).unwrap());

        if let Some(deps) = dependents.get(&id) {
            let mut next: Vec<String> = deps
                .iter()
                .filter_map(|dep_id| {
                    let deg = in_degree.get_mut(dep_id)?;
                    *deg -= 1;
                    if *deg == 0 {
                        Some(dep_id.clone())
                    } else {
                        None
                    }
                })
                .collect();
            next.sort_unstable();
            queue.extend(next);
            queue.sort_unstable();
        }
    }

    if !entry_map.is_empty() {
        return Err(FeatureError::InvalidId(
            "cycle detected in feature installsAfter dependencies".to_string(),
        ));
    }

    Ok(ordered)
}

/// Stages all features from the workspace into `context_dir/features/`, returning them
/// in installation order. If no workspace is provided, returns an empty list.
pub fn stage_all(
    workspace: Option<&WorkspaceConfiguration>,
    context_dir: &Path,
) -> Result<Vec<StagedFeature>, FeatureError> {
    let ws = match workspace {
        Some(ws) if !ws.features.is_empty() => ws,
        _ => return Ok(vec![]),
    };

    // Workspace config dir is always .kaiden/
    let workspace_config_dir = Path::new(".kaiden");

    let entries = build_entries(&ws.features, workspace_config_dir)?;

    let features_dir = context_dir.join("features");
    std::fs::create_dir_all(&features_dir)
        .map_err(|e| FeatureError::Io(format!("creating features directory: {e}")))?;

    let mut metadata_map: HashMap<String, metadata::FeatureMetadata> = HashMap::new();
    let mut dir_names: HashMap<String, String> = HashMap::new();

    for (n, entry) in entries.iter().enumerate() {
        let dir_name = format!("feature-{n}");
        let dest = features_dir.join(&dir_name);
        std::fs::create_dir_all(&dest)
            .map_err(|e| FeatureError::Io(format!("creating feature directory: {e}")))?;

        let meta = match &entry.source {
            FeatureSource::Local(f) => f.stage(&dest)?,
            FeatureSource::Oci(f) => f.stage(&dest)?,
        };

        metadata_map.insert(entry.id.clone(), meta);
        dir_names.insert(entry.id.clone(), dir_name);
    }

    let ordered = topological_order(entries, &metadata_map)?;

    let mut staged = Vec::with_capacity(ordered.len());
    for entry in ordered {
        let meta = metadata_map.remove(&entry.id).unwrap();
        let dir_name = dir_names.remove(&entry.id).unwrap();
        let merged_options = meta.merge_options(&entry.user_opts)?;
        staged.push(StagedFeature {
            id: entry.id,
            dir_name,
            merged_options,
            container_env: meta.container_env,
        });
    }

    Ok(staged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_base_id_strips_tag() {
        assert_eq!(
            feature_base_id("ghcr.io/devcontainers/features/rust:1"),
            "ghcr.io/devcontainers/features/rust"
        );
    }

    #[test]
    fn feature_base_id_strips_digest() {
        assert_eq!(
            feature_base_id("ghcr.io/devcontainers/features/rust@sha256:abc"),
            "ghcr.io/devcontainers/features/rust"
        );
    }

    #[test]
    fn feature_base_id_no_tag_unchanged() {
        assert_eq!(
            feature_base_id("ghcr.io/devcontainers/features/rust"),
            "ghcr.io/devcontainers/features/rust"
        );
    }

    #[test]
    fn build_entries_rejects_https_uri() {
        let mut features = HashMap::new();
        features.insert(
            "https://example.com/feature.tar.gz".to_string(),
            serde_json::Map::new(),
        );
        assert!(matches!(
            build_entries(&features, Path::new(".kaiden")),
            Err(FeatureError::InvalidId(_))
        ));
    }

    #[test]
    fn build_entries_rejects_http_uri() {
        let mut features = HashMap::new();
        features.insert(
            "http://example.com/feature.tar.gz".to_string(),
            serde_json::Map::new(),
        );
        assert!(matches!(
            build_entries(&features, Path::new(".kaiden")),
            Err(FeatureError::InvalidId(_))
        ));
    }

    #[test]
    fn topological_order_respects_installs_after() {
        // Feature B installsAfter A; expect A before B.
        let entries = vec![
            FeatureEntry {
                id: "B".to_string(),
                source: FeatureSource::Oci(oci::OciFeature::new("B")),
                user_opts: serde_json::Map::new(),
            },
            FeatureEntry {
                id: "A".to_string(),
                source: FeatureSource::Oci(oci::OciFeature::new("A")),
                user_opts: serde_json::Map::new(),
            },
        ];

        let mut metadata_map = HashMap::new();
        metadata_map.insert(
            "A".to_string(),
            metadata::FeatureMetadata::for_test(HashMap::new(), vec![]),
        );
        metadata_map.insert(
            "B".to_string(),
            metadata::FeatureMetadata::for_test(HashMap::new(), vec!["A".to_string()]),
        );

        let ordered = topological_order(entries, &metadata_map).unwrap();
        let ids: Vec<&str> = ordered.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["A", "B"]);
    }

    #[test]
    fn topological_order_detects_cycle() {
        let entries = vec![
            FeatureEntry {
                id: "A".to_string(),
                source: FeatureSource::Oci(oci::OciFeature::new("A")),
                user_opts: serde_json::Map::new(),
            },
            FeatureEntry {
                id: "B".to_string(),
                source: FeatureSource::Oci(oci::OciFeature::new("B")),
                user_opts: serde_json::Map::new(),
            },
        ];

        let mut metadata_map = HashMap::new();
        metadata_map.insert(
            "A".to_string(),
            metadata::FeatureMetadata::for_test(HashMap::new(), vec!["B".to_string()]),
        );
        metadata_map.insert(
            "B".to_string(),
            metadata::FeatureMetadata::for_test(HashMap::new(), vec!["A".to_string()]),
        );

        assert!(matches!(
            topological_order(entries, &metadata_map),
            Err(FeatureError::InvalidId(_))
        ));
    }

    #[test]
    fn topological_order_ignores_unknown_installs_after() {
        let entries = vec![FeatureEntry {
            id: "A".to_string(),
            source: FeatureSource::Oci(oci::OciFeature::new("A")),
            user_opts: serde_json::Map::new(),
        }];

        let mut metadata_map = HashMap::new();
        metadata_map.insert(
            "A".to_string(),
            metadata::FeatureMetadata::for_test(HashMap::new(), vec!["not-in-set".to_string()]),
        );

        let ordered = topological_order(entries, &metadata_map).unwrap();
        assert_eq!(ordered.len(), 1);
    }

    #[test]
    fn stage_all_returns_empty_with_no_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let result = stage_all(None, tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn stage_all_returns_empty_with_empty_features() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = WorkspaceConfiguration::default();
        let result = stage_all(Some(&ws), tmp.path()).unwrap();
        assert!(result.is_empty());
    }
}
