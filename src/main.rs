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

mod agent;
mod build;
mod config;
mod containerfile;
mod feature;
mod inference;
mod policy;
mod workspace;

use std::path::{Path, PathBuf};

const BASE_POLICY_YAML: &str = include_str!("../assets/policy.yaml");

use clap::Parser;
use log::LevelFilter;

#[derive(Parser)]
#[command(
    name = "openshell-image-builder",
    version,
    about = "OpenShell image builder"
)]
struct Cli {
    #[arg(help = "Tag for the built image (e.g. myimage:latest)")]
    tag: String,
    #[arg(
        long,
        env = "OPENSHELL_IMAGE_BUILDER_CONFIG",
        help = "Path to config directory (must contain config.toml)"
    )]
    config: Option<PathBuf>,
    #[arg(
        short = 'v',
        action = clap::ArgAction::Count,
        help = "Increase log verbosity (-v info, -vv debug)"
    )]
    verbose: u8,
    #[arg(long, value_enum, help = "Agent to install in the image")]
    agent: Option<agent::AgentKind>,
    #[arg(long, value_enum, help = "Inference server the agent will connect to")]
    inference: Option<inference::InferenceKind>,
}

fn main() {
    let cli = Cli::parse();
    // TODO: when JSON output is added, logs written to stderr may interfere with
    // structured output — revisit whether logs should be suppressed or embedded in the JSON.
    let log_level = match cli.verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    };
    env_logger::Builder::new().filter_level(log_level).init();
    let config = config::load(cli.config.clone()).unwrap_or_else(|e| {
        eprintln!("Error reading config file: {e}");
        std::process::exit(1);
    });
    let workspace = workspace::load().unwrap_or_else(|e| {
        eprintln!("Error reading workspace file: {e}");
        std::process::exit(1);
    });
    let agent = cli.agent.map(agent::from_kind);
    let inference = cli.inference.map(inference::from_kind);
    let context_dir = tempfile::Builder::new()
        .prefix("openshell-image-builder")
        .tempdir()
        .unwrap_or_else(|e| {
            eprintln!("Error creating build context directory: {e}");
            std::process::exit(1);
        });
    let features = feature::stage_all(workspace.as_ref(), context_dir.path()).unwrap_or_else(|e| {
        eprintln!("Error staging features: {e}");
        std::process::exit(1);
    });
    let has_agent_settings = agent.as_deref().is_some_and(|a| {
        match config::agent_settings_dir(cli.config.as_deref(), a.id()) {
            Ok(Some(dir)) => {
                stage_agent_settings(&dir, context_dir.path()).unwrap_or_else(|e| {
                    eprintln!("Error staging agent settings: {e}");
                    std::process::exit(1);
                });
                true
            }
            Ok(None) => false,
            Err(e) => {
                eprintln!("Error finding agent settings directory: {e}");
                std::process::exit(1);
            }
        }
    });
    let policy_yaml = build_policy(BASE_POLICY_YAML, agent.as_deref(), inference.as_deref());
    std::fs::write(context_dir.path().join("policy.yaml"), policy_yaml).unwrap_or_else(|e| {
        eprintln!("Error writing policy.yaml to build context: {e}");
        std::process::exit(1);
    });
    let output = containerfile::generate(&config, agent.as_deref(), &features, has_agent_settings)
        .unwrap_or_else(|e| {
            eprintln!("Error generating Containerfile: {e}");
            std::process::exit(1);
        });
    build::build(&output, &cli.tag, &build::PodmanRunner, context_dir.path()).unwrap_or_else(|e| {
        eprintln!("Error building image: {e}");
        std::process::exit(1);
    });
}

fn copy_dir(src: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(format!(
                "symlinks are not allowed in agent settings: {}",
                entry.path().display()
            )
            .into());
        }
        let dest_path = dest.join(entry.file_name());
        if file_type.is_dir() {
            std::fs::create_dir_all(&dest_path)?;
            copy_dir(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

fn stage_agent_settings(
    settings_dir: &Path,
    context_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let dest = context_dir.join("agent-settings");
    std::fs::create_dir_all(&dest)?;
    copy_dir(settings_dir, &dest)?;
    Ok(())
}

fn build_policy(
    base_yaml: &str,
    agent: Option<&dyn agent::Agent>,
    inference: Option<&dyn inference::Inference>,
) -> String {
    let mut sandbox_policy = policy::parse_sandbox_policy(base_yaml).unwrap_or_else(|e| {
        eprintln!("Error parsing base policy.yaml: {e}");
        std::process::exit(1);
    });
    if let (Some(inference), Some(agent)) = (inference, agent) {
        let inference_yaml = inference.policy_yaml(agent.binary_path());
        let inference_policy = policy::parse_sandbox_policy(&inference_yaml).unwrap_or_else(|e| {
            eprintln!("Error parsing inference policy: {e}");
            std::process::exit(1);
        });
        sandbox_policy
            .network_policies
            .extend(inference_policy.network_policies);
    }
    if let Some(agent) = agent {
        let agent_yaml = agent.policy_yaml();
        if !agent_yaml.is_empty() {
            let agent_policy = policy::parse_sandbox_policy(agent_yaml).unwrap_or_else(|e| {
                eprintln!("Error parsing agent policy: {e}");
                std::process::exit(1);
            });
            sandbox_policy
                .network_policies
                .extend(agent_policy.network_policies);
        }
    }
    policy::serialize_sandbox_policy(&sandbox_policy).unwrap_or_else(|e| {
        eprintln!("Error serializing policy.yaml: {e}");
        std::process::exit(1);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn version_matches_cargo_toml() {
        let cmd = Cli::command();
        assert_eq!(cmd.get_version(), Some(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn build_policy_without_agent_has_no_claude_code_rule() {
        let yaml = build_policy(BASE_POLICY_YAML, None, None);
        assert!(!yaml.contains("name: claude-code"));
    }

    #[test]
    fn build_policy_with_claude_agent_includes_claude_code_rule() {
        let yaml = build_policy(BASE_POLICY_YAML, Some(&agent::ClaudeAgent), None);
        assert!(yaml.contains("name: claude-code"));
    }

    #[test]
    fn build_policy_without_inference_has_no_anthropic_rule() {
        let yaml = build_policy(BASE_POLICY_YAML, Some(&agent::ClaudeAgent), None);
        assert!(!yaml.contains("api.anthropic.com"));
    }

    #[test]
    fn build_policy_with_inference_includes_anthropic_rule() {
        let yaml = build_policy(
            BASE_POLICY_YAML,
            Some(&agent::ClaudeAgent),
            Some(&inference::AnthropicInference),
        );
        assert!(yaml.contains("api.anthropic.com"));
    }

    #[test]
    fn build_policy_with_inference_uses_agent_binary() {
        let yaml = build_policy(
            BASE_POLICY_YAML,
            Some(&agent::ClaudeAgent),
            Some(&inference::AnthropicInference),
        );
        assert!(yaml.contains("/sandbox/.local/bin/claude"));
    }

    #[test]
    fn build_policy_with_vertexai_inference_includes_aiplatform_rule() {
        let yaml = build_policy(
            BASE_POLICY_YAML,
            Some(&agent::ClaudeAgent),
            Some(&inference::VertexAiInference),
        );
        assert!(yaml.contains("aiplatform.googleapis.com"));
    }

    // copy_dir

    #[test]
    fn copy_dir_copies_file_with_content() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("file.txt"), "hello").unwrap();
        let dest = tempfile::tempdir().unwrap();
        copy_dir(src.path(), dest.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.path().join("file.txt")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn copy_dir_recurses_into_subdirectories() {
        let src = tempfile::tempdir().unwrap();
        let subdir = src.path().join("sub");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join("nested.txt"), "nested").unwrap();
        let dest = tempfile::tempdir().unwrap();
        copy_dir(src.path(), dest.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.path().join("sub").join("nested.txt")).unwrap(),
            "nested"
        );
    }

    #[test]
    fn copy_dir_empty_source_succeeds() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        assert!(copy_dir(src.path(), dest.path()).is_ok());
    }

    #[test]
    fn copy_dir_fails_when_source_missing() {
        let dest = tempfile::tempdir().unwrap();
        let result = copy_dir(Path::new("/nonexistent/path"), dest.path());
        assert!(result.is_err());
    }

    #[test]
    #[cfg(unix)]
    fn copy_dir_rejects_symlinks() {
        let src = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/etc/passwd", src.path().join("link")).unwrap();
        let dest = tempfile::tempdir().unwrap();
        let err = copy_dir(src.path(), dest.path()).unwrap_err();
        assert!(
            err.to_string().contains("symlinks are not allowed"),
            "unexpected error: {err}"
        );
    }

    // stage_agent_settings

    #[test]
    fn stage_agent_settings_creates_agent_settings_subdir() {
        let settings = tempfile::tempdir().unwrap();
        let context = tempfile::tempdir().unwrap();
        stage_agent_settings(settings.path(), context.path()).unwrap();
        assert!(context.path().join("agent-settings").is_dir());
    }

    #[test]
    fn stage_agent_settings_copies_files_into_subdir() {
        let settings = tempfile::tempdir().unwrap();
        std::fs::write(settings.path().join("myfile"), "data").unwrap();
        let context = tempfile::tempdir().unwrap();
        stage_agent_settings(settings.path(), context.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(context.path().join("agent-settings").join("myfile")).unwrap(),
            "data"
        );
    }

    #[test]
    fn stage_agent_settings_preserves_nested_structure() {
        let settings = tempfile::tempdir().unwrap();
        let subdir = settings.path().join(".claude");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join("settings.json"), "{}").unwrap();
        let context = tempfile::tempdir().unwrap();
        stage_agent_settings(settings.path(), context.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(
                context
                    .path()
                    .join("agent-settings")
                    .join(".claude")
                    .join("settings.json")
            )
            .unwrap(),
            "{}"
        );
    }
}
