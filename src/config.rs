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

use log::info;
use serde::Deserialize;

#[derive(Deserialize)]
struct ConfigFile {
    openshell_image_builder: Config,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub base_image: BaseImageConfig,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct BaseImageConfig {
    #[serde(default = "default_image")]
    pub image: String,
    #[serde(default = "default_tag")]
    pub tag: String,
}

fn default_version() -> u32 {
    1
}

fn default_image() -> String {
    "ubuntu".to_string()
}

fn default_tag() -> String {
    "24.04".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: default_version(),
            base_image: BaseImageConfig::default(),
        }
    }
}

impl Default for BaseImageConfig {
    fn default() -> Self {
        BaseImageConfig {
            image: default_image(),
            tag: default_tag(),
        }
    }
}

fn find_settings_dir(explicit_dir: Option<&Path>) -> Result<Option<PathBuf>, std::io::Error> {
    if let Some(dir) = explicit_dir {
        if !dir.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Config directory not found: {}", dir.display()),
            ));
        }
        return Ok(Some(dir.to_path_buf()));
    }
    Ok(dirs::config_dir().map(|d| d.join("openshell-image-builder")))
}

fn find_config_file(explicit_dir: Option<PathBuf>) -> Result<Option<PathBuf>, std::io::Error> {
    let Some(dir) = find_settings_dir(explicit_dir.as_deref())? else {
        info!("XDG config directory not available");
        return Ok(None);
    };
    let path = dir.join("config.toml");
    if path.exists() {
        info!("Config file found at {}", path.display());
        Ok(Some(path))
    } else {
        info!("No config file at {}", path.display());
        Ok(None)
    }
}

pub fn agent_settings_dir(
    explicit_dir: Option<&Path>,
    agent_name: &str,
) -> Result<Option<PathBuf>, std::io::Error> {
    let Some(settings_dir) = find_settings_dir(explicit_dir)? else {
        return Ok(None);
    };
    let dir = settings_dir.join("agents").join(agent_name);
    if dir.is_dir() {
        Ok(Some(dir))
    } else {
        Ok(None)
    }
}

pub fn load(explicit_path: Option<PathBuf>) -> Result<Config, Box<dyn std::error::Error>> {
    let Some(path) = find_config_file(explicit_path)? else {
        info!("No config file found, using built-in defaults");
        return Ok(Config::default());
    };

    let content = std::fs::read_to_string(&path).unwrap_or_default();
    if content.trim().is_empty() {
        info!(
            "Config file {} is empty, using built-in defaults",
            path.display()
        );
        return Ok(Config::default());
    }

    let file: ConfigFile = toml::from_str(&content)?;
    info!("Config loaded from {}", path.display());
    Ok(file.openshell_image_builder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_config_dir(content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let mut f = std::fs::File::create(dir.path().join("config.toml")).unwrap();
        write!(f, "{content}").unwrap();
        dir
    }

    #[test]
    fn default_config() {
        let config = Config::default();
        assert_eq!(config.version, 1);
        assert_eq!(config.base_image.image, "ubuntu");
        assert_eq!(config.base_image.tag, "24.04");
    }

    #[test]
    fn load_fails_when_explicit_dir_not_found() {
        let path = std::env::temp_dir().join("openshell-image-builder-nonexistent-dir");
        assert!(!path.exists());
        assert!(load(Some(path)).is_err());
    }

    #[test]
    fn load_returns_defaults_when_no_config_toml_in_dir() {
        let dir = tempfile::tempdir().unwrap();
        let config = load(Some(dir.path().to_path_buf())).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn load_returns_defaults_for_empty_file() {
        let dir = write_config_dir("   ");
        let config = load(Some(dir.path().to_path_buf())).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn load_parses_valid_toml() {
        let dir = write_config_dir(
            r#"
[openshell_image_builder]
version = 2

[openshell_image_builder.base_image]
image = "ubuntu"
tag = "24.04"
"#,
        );
        let config = load(Some(dir.path().to_path_buf())).unwrap();
        assert_eq!(config.version, 2);
        assert_eq!(config.base_image.image, "ubuntu");
        assert_eq!(config.base_image.tag, "24.04");
    }

    #[test]
    fn load_returns_error_for_invalid_toml() {
        let dir = write_config_dir("not valid toml [[[");
        assert!(load(Some(dir.path().to_path_buf())).is_err());
    }

    #[test]
    fn load_returns_error_when_required_section_missing() {
        let dir = write_config_dir("[some_other_section]\nkey = \"value\"");
        assert!(load(Some(dir.path().to_path_buf())).is_err());
    }

    #[test]
    fn load_parses_toml_with_only_version() {
        let dir = write_config_dir("[openshell_image_builder]\nversion = 2");
        let config = load(Some(dir.path().to_path_buf())).unwrap();
        assert_eq!(config.version, 2);
        assert_eq!(config.base_image, BaseImageConfig::default());
    }

    #[test]
    fn load_parses_toml_with_only_base_image() {
        let dir = write_config_dir(
            "[openshell_image_builder.base_image]\nimage = \"ubuntu\"\ntag = \"24.04\"",
        );
        let config = load(Some(dir.path().to_path_buf())).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.base_image.image, "ubuntu");
        assert_eq!(config.base_image.tag, "24.04");
    }

    #[test]
    fn load_parses_toml_with_only_image_in_base_image() {
        let dir = write_config_dir("[openshell_image_builder.base_image]\nimage = \"centos\"");
        let config = load(Some(dir.path().to_path_buf())).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.base_image.image, "centos");
        assert_eq!(config.base_image.tag, "24.04");
    }

    #[test]
    fn load_parses_toml_with_only_tag_in_base_image() {
        let dir = write_config_dir("[openshell_image_builder.base_image]\ntag = \"40\"");
        let config = load(Some(dir.path().to_path_buf())).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.base_image.image, "ubuntu");
        assert_eq!(config.base_image.tag, "40");
    }

    #[test]
    fn load_with_no_explicit_path_returns_ok() {
        // Exercises the XDG config-directory lookup; result is environment-dependent
        // but must always be Ok (either defaults or a valid XDG config).
        assert!(load(None).is_ok());
    }

    #[test]
    fn agent_settings_dir_returns_none_when_subdir_missing() {
        let dir = tempfile::tempdir().unwrap();
        let result = agent_settings_dir(Some(dir.path()), "claude").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn agent_settings_dir_returns_path_when_subdir_exists() {
        let dir = tempfile::tempdir().unwrap();
        let agent_dir = dir.path().join("agents").join("claude");
        std::fs::create_dir_all(&agent_dir).unwrap();
        let result = agent_settings_dir(Some(dir.path()), "claude").unwrap();
        assert_eq!(result, Some(agent_dir));
    }

    #[test]
    fn agent_settings_dir_fails_when_explicit_dir_not_found() {
        let path = std::env::temp_dir().join("openshell-image-builder-nonexistent-dir");
        assert!(!path.exists());
        assert!(agent_settings_dir(Some(&path), "claude").is_err());
    }

    #[test]
    fn agent_settings_dir_with_no_explicit_path_returns_ok() {
        // XDG lookup — result is environment-dependent but must always be Ok.
        assert!(agent_settings_dir(None, "claude").is_ok());
    }
}
