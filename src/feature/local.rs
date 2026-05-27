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

use std::path::{Path, PathBuf};

use super::FeatureError;
use super::metadata::{self, FeatureMetadata};

pub struct LocalFeature {
    pub id: String,
    /// Absolute path to the feature directory.
    path: PathBuf,
}

impl LocalFeature {
    /// Creates a local feature. `workspace_config_dir` is the directory containing
    /// workspace.json (i.e. `.kaiden/`); relative IDs are resolved from there.
    pub fn new(id: &str, workspace_config_dir: &Path) -> Result<Self, FeatureError> {
        let abs = workspace_config_dir
            .join(id)
            .canonicalize()
            .map_err(|e| FeatureError::Io(format!("resolving local feature {id:?}: {e}")))?;
        Ok(Self {
            id: id.to_string(),
            path: abs,
        })
    }

    /// Copies the entire feature directory into `dest_dir` and returns the parsed metadata.
    pub fn stage(&self, dest_dir: &Path) -> Result<FeatureMetadata, FeatureError> {
        copy_dir_recursive(&self.path, dest_dir)?;
        if !dest_dir.join("install.sh").exists() {
            return Err(FeatureError::Io(format!(
                "missing install.sh for feature {:?}",
                self.id
            )));
        }
        metadata::parse(dest_dir)
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), FeatureError> {
    for entry in std::fs::read_dir(src)
        .map_err(|e| FeatureError::Io(format!("reading directory {src:?}: {e}")))?
    {
        let entry = entry.map_err(|e| FeatureError::Io(format!("reading directory entry: {e}")))?;
        let file_type = entry
            .file_type()
            .map_err(|e| FeatureError::Io(format!("reading file type: {e}")))?;

        if file_type.is_symlink() {
            return Err(FeatureError::Io(
                "symlinks are not supported in local features".to_string(),
            ));
        }

        let src_path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            std::fs::create_dir_all(&dest_path)
                .map_err(|e| FeatureError::Io(format!("creating directory {dest_path:?}: {e}")))?;
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path).map_err(|e| {
                FeatureError::Io(format!("copying {src_path:?} to {dest_path:?}: {e}"))
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_feature_dir(base: &Path, name: &str, meta: &str, script: &str) -> PathBuf {
        let dir = base.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("devcontainer-feature.json"), meta).unwrap();
        std::fs::write(dir.join("install.sh"), script).unwrap();
        dir
    }

    #[test]
    fn stages_feature_files_to_dest() {
        let src = tempfile::tempdir().unwrap();
        make_feature_dir(
            src.path(),
            "my-feature",
            r#"{"id":"my-feature"}"#,
            "#!/bin/bash\necho ok",
        );
        let dest = tempfile::tempdir().unwrap();
        let feature = LocalFeature::new("./my-feature", src.path()).unwrap();
        feature.stage(dest.path()).unwrap();
        assert!(dest.path().join("install.sh").exists());
        assert!(dest.path().join("devcontainer-feature.json").exists());
    }

    #[test]
    fn stages_nested_files() {
        let src = tempfile::tempdir().unwrap();
        let dir = src.path().join("nested-feature");
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::write(dir.join("devcontainer-feature.json"), r#"{"id":"x"}"#).unwrap();
        std::fs::write(dir.join("install.sh"), "#!/bin/bash").unwrap();
        std::fs::write(dir.join("scripts/helper.sh"), "echo helper").unwrap();
        let dest = tempfile::tempdir().unwrap();
        let feature = LocalFeature::new("./nested-feature", src.path()).unwrap();
        feature.stage(dest.path()).unwrap();
        assert!(dest.path().join("scripts/helper.sh").exists());
    }

    #[test]
    fn missing_directory_errors() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(matches!(
            LocalFeature::new("./nonexistent", tmp.path()),
            Err(FeatureError::Io(_))
        ));
    }

    #[test]
    fn missing_install_sh_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("no-script");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("devcontainer-feature.json"), r#"{"id":"x"}"#).unwrap();
        let dest = tempfile::tempdir().unwrap();
        let feature = LocalFeature::new("./no-script", tmp.path()).unwrap();
        assert!(matches!(
            feature.stage(dest.path()),
            Err(FeatureError::Io(_))
        ));
    }

    #[test]
    fn missing_metadata_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("no-meta");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("install.sh"), "#!/bin/bash").unwrap();
        let dest = tempfile::tempdir().unwrap();
        let feature = LocalFeature::new("./no-meta", tmp.path()).unwrap();
        assert!(matches!(
            feature.stage(dest.path()),
            Err(FeatureError::Io(_))
        ));
    }
}
