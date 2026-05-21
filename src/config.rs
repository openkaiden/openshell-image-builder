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

use std::path::PathBuf;

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
    "fedora".to_string()
}

fn default_tag() -> String {
    "latest".to_string()
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

fn find_config_file(explicit_path: Option<PathBuf>) -> Result<Option<PathBuf>, std::io::Error> {
    if let Some(path) = explicit_path {
        if path.exists() {
            info!("Config file found at {}", path.display());
            return Ok(Some(path));
        }
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Config file not found: {}", path.display()),
        ));
    }
    let xdg_path =
        dirs::config_dir().map(|d| d.join("openshell-image-builder").join("config.toml"));
    match xdg_path {
        Some(ref path) if path.exists() => {
            info!("Config file found at XDG path {}", path.display());
            Ok(xdg_path)
        }
        Some(ref path) => {
            info!("No config file at XDG path {}", path.display());
            Ok(None)
        }
        None => {
            info!("XDG config directory not available");
            Ok(None)
        }
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

    #[test]
    fn default_config() {
        let config = Config::default();
        assert_eq!(config.version, 1);
        assert_eq!(config.base_image.image, "fedora");
        assert_eq!(config.base_image.tag, "latest");
    }

    #[test]
    fn load_fails_when_explicit_path_not_found() {
        let path = std::env::temp_dir().join("openshell-image-builder-nonexistent.toml");
        assert!(!path.exists());
        assert!(load(Some(path)).is_err());
    }

    #[test]
    fn load_returns_defaults_for_empty_file() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "   ").unwrap();
        let config = load(Some(f.path().to_path_buf())).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn load_parses_valid_toml() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            f,
            r#"
[openshell_image_builder]
version = 2

[openshell_image_builder.base_image]
image = "ubuntu"
tag = "24.04"
"#
        )
        .unwrap();
        let config = load(Some(f.path().to_path_buf())).unwrap();
        assert_eq!(config.version, 2);
        assert_eq!(config.base_image.image, "ubuntu");
        assert_eq!(config.base_image.tag, "24.04");
    }

    #[test]
    fn load_returns_error_for_invalid_toml() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "not valid toml [[[").unwrap();
        assert!(load(Some(f.path().to_path_buf())).is_err());
    }
}
