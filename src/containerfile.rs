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

use crate::agent::Agent;
use crate::config::Config;
use crate::feature::StagedFeature;

#[derive(Debug, PartialEq)]
pub enum ContainerfileError {
    NotSupported { image: String },
}

impl std::fmt::Display for ContainerfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerfileError::NotSupported { image } => {
                write!(f, "base image '{image}' is not supported")
            }
        }
    }
}

impl std::error::Error for ContainerfileError {}

pub fn generate(
    config: &Config,
    agent: Option<&dyn Agent>,
    features: &[StagedFeature],
    with_agent_settings: bool,
    skill_names: &[String],
) -> Result<String, ContainerfileError> {
    let tag = &config.base_image.tag;
    let system_stage = match config.base_image.image.as_str() {
        "fedora" => fedora_system_stage(tag),
        "ubuntu" => ubuntu_system_stage(tag),
        image => {
            return Err(ContainerfileError::NotSupported {
                image: image.to_string(),
            });
        }
    };
    Ok(format!(
        "{system_stage}\n{}",
        final_stage(agent, features, with_agent_settings, skill_names)
    ))
}

fn skills_section(agent: Option<&dyn Agent>, skill_names: &[String]) -> String {
    if skill_names.is_empty() {
        return String::new();
    }
    let skills_dir = match agent.map(|a| a.skills_dir()).filter(|d| !d.is_empty()) {
        Some(d) => d,
        None => return String::new(),
    };
    let mut out = String::new();
    for name in skill_names {
        out.push_str(&format!(
            "COPY --chown=sandbox:sandbox skills/{name}/ {skills_dir}/{name}/\n"
        ));
    }
    out.push('\n');
    out
}

/// Renders the feature installation section for the `final` stage.
///
/// Each feature's files are COPYed from the build context into the image, then
/// install.sh is run with options passed as env var assignments. `_REMOTE_USER`
/// and `_REMOTE_USER_HOME` are set before the block so scripts can resolve the
/// target user. Each feature's `containerEnv` is set immediately after its
/// install so subsequent features can reference it.
fn features_section(features: &[StagedFeature]) -> String {
    if features.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str("ENV _REMOTE_USER=\"sandbox\"\n");
    out.push_str("ENV _REMOTE_USER_HOME=\"/sandbox\"\n");

    for feature in features {
        out.push('\n');
        out.push_str(&format!("# Feature: {}\n", feature.id));

        let install_dir = format!("/tmp/feature-install/{}", feature.dir_name);
        out.push_str(&format!(
            "COPY features/{}/ {install_dir}/\n",
            feature.dir_name
        ));

        // Build sorted option assignments: VAR="value" (embedded " escaped).
        let mut opt_pairs: Vec<(&String, &String)> = feature.merged_options.iter().collect();
        opt_pairs.sort_by_key(|(k, _)| k.as_str());
        let opts_prefix = if opt_pairs.is_empty() {
            String::new()
        } else {
            let opts = opt_pairs
                .iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, v.replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(" ");
            format!("{opts} ")
        };

        out.push_str(&format!(
            "RUN chmod +x {install_dir}/install.sh && \\\n    {opts_prefix}{install_dir}/install.sh\n"
        ));

        // containerEnv: one ENV per variable, sorted, double-quoted.
        if !feature.container_env.is_empty() {
            let mut env_pairs: Vec<(&String, &String)> = feature.container_env.iter().collect();
            env_pairs.sort_by_key(|(k, _)| k.as_str());
            for (k, v) in env_pairs {
                let escaped = v.replace('"', "\\\"");
                out.push_str(&format!("ENV {k}=\"{escaped}\"\n"));
            }
        }
    }
    out.push_str("RUN rm -rf /tmp/feature-install\n");
    out.push('\n');
    out
}

fn ubuntu_system_stage(tag: &str) -> String {
    format!(
        r#"# System base
FROM docker.io/library/ubuntu:{tag} AS system

ENV DEBIAN_FRONTEND=noninteractive

WORKDIR /sandbox

# Core system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        dnsutils \
        iproute2 \
        iptables \
        nftables \
        iputils-ping \
        net-tools \
        netcat-openbsd \
        openssh-sftp-server \
        procps \
        traceroute \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r supervisor && useradd -r -g supervisor -s /usr/sbin/nologin supervisor && \
    groupadd -r sandbox && useradd -r -g sandbox -d /sandbox -s /bin/bash sandbox
"#
    )
}

fn fedora_system_stage(tag: &str) -> String {
    format!(
        r#"# System base
FROM registry.fedoraproject.org/fedora:{tag} AS system

WORKDIR /sandbox

# Core system dependencies
RUN dnf install -y --setopt=install_weak_deps=False \
        bind-utils \
        ca-certificates \
        curl \
        iproute \
        iptables \
        iputils \
        net-tools \
        nftables \
        nmap-ncat \
        openssh-server \
        procps-ng \
        traceroute \
        which \
    && dnf clean all

RUN groupadd -r supervisor && useradd -r -g supervisor -s /usr/sbin/nologin supervisor && \
    groupadd -r sandbox && useradd -r -g sandbox -d /sandbox -s /bin/bash sandbox
"#
    )
}

fn final_stage(
    agent: Option<&dyn Agent>,
    features: &[StagedFeature],
    with_agent_settings: bool,
    skill_names: &[String],
) -> String {
    let agent_section = agent
        .map(|a| format!("{}\n\n", a.install()))
        .unwrap_or_default();
    let agent_settings_section = if with_agent_settings {
        "COPY --chown=sandbox:sandbox agent-settings/ /sandbox/\n\n"
    } else {
        ""
    };
    let skills_section = skills_section(agent, skill_names);
    let features_section = features_section(features);
    format!(
        r#"# Final base image
FROM system AS final

{features_section}COPY policy.yaml /etc/openshell/policy.yaml

RUN printf 'export PS1="\\u@\\h:\\w\\$ "\n' \
        > /sandbox/.bashrc && \
    printf '[ -f ~/.bashrc ] && . ~/.bashrc\n' > /sandbox/.profile && \
    chown sandbox:sandbox /sandbox/.bashrc /sandbox/.profile && \
    chown -R sandbox:sandbox /sandbox

USER sandbox

{agent_settings_section}{skills_section}{agent_section}ENTRYPOINT ["/bin/bash"]
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::config::{BaseImageConfig, Config};

    fn ubuntu_config(tag: &str) -> Config {
        Config {
            version: 1,
            base_image: BaseImageConfig {
                image: "ubuntu".to_string(),
                tag: tag.to_string(),
            },
        }
    }

    fn fedora_config() -> Config {
        Config {
            version: 1,
            base_image: BaseImageConfig {
                image: "fedora".to_string(),
                tag: "latest".to_string(),
            },
        }
    }

    struct MockAgent;

    impl Agent for MockAgent {
        fn id(&self) -> &str {
            "mock"
        }

        fn install(&self) -> String {
            "RUN echo mock-agent".to_string()
        }

        fn binary_path(&self) -> &str {
            "/sandbox/.local/bin/mock-agent"
        }

        fn skills_dir(&self) -> &str {
            "/sandbox/.mock/skills"
        }
    }

    fn mock_feature(id: &str, dir_name: &str) -> StagedFeature {
        StagedFeature {
            id: id.to_string(),
            dir_name: dir_name.to_string(),
            merged_options: std::collections::HashMap::new(),
            container_env: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn ubuntu_generates_successfully() {
        let config = ubuntu_config("noble-20251013");
        assert!(generate(&config, None, &[], false, &[]).is_ok());
    }

    #[test]
    fn ubuntu_containerfile_contains_tag() {
        let config = ubuntu_config("noble-20251013");
        let content = generate(&config, None, &[], false, &[]).unwrap();
        assert!(content.contains("FROM docker.io/library/ubuntu:noble-20251013 AS system"));
    }

    #[test]
    fn ubuntu_containerfile_tag_is_substituted() {
        let content = generate(&ubuntu_config("24.04"), None, &[], false, &[]).unwrap();
        assert!(content.contains("FROM docker.io/library/ubuntu:24.04 AS system"));
        assert!(!content.contains("{tag}"));
    }

    #[test]
    fn ubuntu_with_agent_includes_install() {
        let content = generate(
            &ubuntu_config("noble-20251013"),
            Some(&MockAgent),
            &[],
            false,
            &[],
        )
        .unwrap();
        assert!(content.contains("RUN echo mock-agent"));
    }

    #[test]
    fn ubuntu_agent_install_runs_as_sandbox_user() {
        let content = generate(
            &ubuntu_config("noble-20251013"),
            Some(&MockAgent),
            &[],
            false,
            &[],
        )
        .unwrap();
        let user_pos = content.find("USER sandbox").unwrap();
        let install_pos = content.find("RUN echo mock-agent").unwrap();
        assert!(
            install_pos > user_pos,
            "agent install must appear after USER sandbox"
        );
    }

    #[test]
    fn ubuntu_without_agent_omits_install() {
        let content = generate(&ubuntu_config("noble-20251013"), None, &[], false, &[]).unwrap();
        assert!(!content.contains("RUN echo mock-agent"));
    }

    #[test]
    fn fedora_generates_successfully() {
        assert!(generate(&fedora_config(), None, &[], false, &[]).is_ok());
    }

    #[test]
    fn fedora_containerfile_contains_tag() {
        let content = generate(&fedora_config(), None, &[], false, &[]).unwrap();
        assert!(content.contains("FROM registry.fedoraproject.org/fedora:latest AS system"));
    }

    #[test]
    fn fedora_containerfile_tag_is_substituted() {
        let content = generate(&fedora_config(), None, &[], false, &[]).unwrap();
        assert!(!content.contains("{tag}"));
    }

    #[test]
    fn fedora_with_agent_includes_install() {
        let content = generate(&fedora_config(), Some(&MockAgent), &[], false, &[]).unwrap();
        assert!(content.contains("RUN echo mock-agent"));
    }

    #[test]
    fn fedora_agent_install_runs_as_sandbox_user() {
        let content = generate(&fedora_config(), Some(&MockAgent), &[], false, &[]).unwrap();
        let user_pos = content.find("USER sandbox").unwrap();
        let install_pos = content.find("RUN echo mock-agent").unwrap();
        assert!(
            install_pos > user_pos,
            "agent install must appear after USER sandbox"
        );
    }

    #[test]
    fn fedora_without_agent_omits_install() {
        let content = generate(&fedora_config(), None, &[], false, &[]).unwrap();
        assert!(!content.contains("RUN echo mock-agent"));
    }

    #[test]
    fn not_supported_error_message() {
        let err = ContainerfileError::NotSupported {
            image: "centos".to_string(),
        };
        assert_eq!(err.to_string(), "base image 'centos' is not supported");
    }

    #[test]
    fn unknown_image_returns_not_supported() {
        let config = Config {
            version: 1,
            base_image: BaseImageConfig {
                image: "centos".to_string(),
                tag: "latest".to_string(),
            },
        };
        let err = generate(&config, None, &[], false, &[]).unwrap_err();
        assert_eq!(
            err,
            ContainerfileError::NotSupported {
                image: "centos".to_string()
            }
        );
    }

    #[test]
    fn feature_section_appears_before_profile_setup() {
        let feature = mock_feature("./tools/my-feature", "feature-0");
        let content = generate(&ubuntu_config("24.04"), None, &[feature], false, &[]).unwrap();
        let feature_pos = content.find("# Feature:").unwrap();
        let profile_pos = content.find("printf 'export PS1").unwrap();
        assert!(
            feature_pos < profile_pos,
            "feature block must appear before profile setup"
        );
    }

    #[test]
    fn feature_copy_instruction_present() {
        let feature = mock_feature("./tools/my-feature", "feature-0");
        let content = generate(&ubuntu_config("24.04"), None, &[feature], false, &[]).unwrap();
        assert!(content.contains("COPY features/feature-0/"));
        assert!(content.contains("/tmp/feature-install/feature-0/install.sh"));
    }

    #[test]
    fn feature_remote_user_env_vars_set() {
        let feature = mock_feature("./tools/my-feature", "feature-0");
        let content = generate(&ubuntu_config("24.04"), None, &[feature], false, &[]).unwrap();
        assert!(content.contains("_REMOTE_USER=\"sandbox\""));
        assert!(content.contains("_REMOTE_USER_HOME=\"/sandbox\""));
    }

    #[test]
    fn feature_options_in_run_command() {
        let mut feature = mock_feature("./tools/my-feature", "feature-0");
        feature
            .merged_options
            .insert("VERSION".to_string(), "1.0".to_string());
        let content = generate(&ubuntu_config("24.04"), None, &[feature], false, &[]).unwrap();
        assert!(content.contains("VERSION=\"1.0\""));
    }

    #[test]
    fn feature_container_env_emitted_as_env_instruction() {
        let mut feature = mock_feature("./tools/my-feature", "feature-0");
        feature
            .container_env
            .insert("CARGO_HOME".to_string(), "/home/sandbox/.cargo".to_string());
        let content = generate(&ubuntu_config("24.04"), None, &[feature], false, &[]).unwrap();
        assert!(content.contains("ENV CARGO_HOME=\"/home/sandbox/.cargo\""));
    }

    #[test]
    fn feature_block_before_user_sandbox() {
        let feature = mock_feature("./tools/my-feature", "feature-0");
        let content = generate(&ubuntu_config("24.04"), None, &[feature], false, &[]).unwrap();
        let feature_pos = content.find("# Feature:").unwrap();
        let user_sandbox_pos = content.find("USER sandbox").unwrap();
        assert!(
            feature_pos < user_sandbox_pos,
            "feature must be installed before USER sandbox"
        );
    }

    #[test]
    fn feature_install_dir_cleaned_up() {
        let feature = mock_feature("./tools/my-feature", "feature-0");
        let content = generate(&ubuntu_config("24.04"), None, &[feature], false, &[]).unwrap();
        assert!(content.contains("RUN rm -rf /tmp/feature-install\n"));
    }

    #[test]
    fn no_features_produces_same_output_as_before() {
        let with_empty = generate(&ubuntu_config("24.04"), None, &[], false, &[]).unwrap();
        assert!(!with_empty.contains("# Feature:"));
        assert!(!with_empty.contains("_REMOTE_USER"));
        assert!(!with_empty.contains("rm -rf /tmp/feature-install"));
    }

    #[test]
    fn ubuntu_copies_policy_yaml() {
        let content = generate(&ubuntu_config("24.04"), None, &[], false, &[]).unwrap();
        assert!(content.contains("COPY policy.yaml /etc/openshell/policy.yaml"));
    }

    #[test]
    fn fedora_copies_policy_yaml() {
        let content = generate(&fedora_config(), None, &[], false, &[]).unwrap();
        assert!(content.contains("COPY policy.yaml /etc/openshell/policy.yaml"));
    }

    #[test]
    fn policy_copy_appears_before_user_sandbox() {
        let content = generate(&ubuntu_config("24.04"), None, &[], false, &[]).unwrap();
        let copy_pos = content
            .find("COPY policy.yaml /etc/openshell/policy.yaml")
            .unwrap();
        let user_pos = content.find("USER sandbox").unwrap();
        assert!(
            copy_pos < user_pos,
            "policy.yaml COPY must appear before USER sandbox"
        );
    }

    #[test]
    fn ubuntu_with_agent_settings_includes_copy() {
        let content = generate(&ubuntu_config("24.04"), None, &[], true, &[]).unwrap();
        assert!(content.contains("COPY --chown=sandbox:sandbox agent-settings/ /sandbox/"));
    }

    #[test]
    fn ubuntu_without_agent_settings_omits_copy() {
        let content = generate(&ubuntu_config("24.04"), None, &[], false, &[]).unwrap();
        assert!(!content.contains("agent-settings/"));
    }

    #[test]
    fn fedora_with_agent_settings_includes_copy() {
        let content = generate(&fedora_config(), None, &[], true, &[]).unwrap();
        assert!(content.contains("COPY --chown=sandbox:sandbox agent-settings/ /sandbox/"));
    }

    #[test]
    fn agent_settings_copy_uses_chown_sandbox() {
        let content = generate(&ubuntu_config("24.04"), None, &[], true, &[]).unwrap();
        assert!(content.contains("--chown=sandbox:sandbox"));
    }

    #[test]
    fn agent_settings_copy_appears_after_user_sandbox() {
        let content = generate(&ubuntu_config("24.04"), None, &[], true, &[]).unwrap();
        let user_pos = content.find("USER sandbox").unwrap();
        let copy_pos = content
            .find("COPY --chown=sandbox:sandbox agent-settings/")
            .unwrap();
        assert!(
            copy_pos > user_pos,
            "agent-settings COPY must appear after USER sandbox"
        );
    }

    #[test]
    fn agent_settings_copy_appears_before_agent_install() {
        let content = generate(&ubuntu_config("24.04"), Some(&MockAgent), &[], true, &[]).unwrap();
        let copy_pos = content
            .find("COPY --chown=sandbox:sandbox agent-settings/")
            .unwrap();
        let install_pos = content.find("RUN echo mock-agent").unwrap();
        assert!(
            copy_pos < install_pos,
            "agent-settings COPY must appear before agent install"
        );
    }

    #[test]
    fn skills_copy_present_for_agent_with_skills_dir() {
        let skills = vec!["my-skill".to_string()];
        let content = generate(
            &ubuntu_config("24.04"),
            Some(&MockAgent),
            &[],
            false,
            &skills,
        )
        .unwrap();
        assert!(content.contains("COPY --chown=sandbox:sandbox skills/my-skill/"));
    }

    #[test]
    fn skills_copy_uses_agent_skills_dir() {
        let skills = vec!["my-skill".to_string()];
        let content = generate(
            &ubuntu_config("24.04"),
            Some(&MockAgent),
            &[],
            false,
            &skills,
        )
        .unwrap();
        assert!(content.contains("/sandbox/.mock/skills/my-skill/"));
    }

    #[test]
    fn skills_copy_omitted_when_no_skills() {
        let content = generate(&ubuntu_config("24.04"), Some(&MockAgent), &[], false, &[]).unwrap();
        assert!(!content.contains("skills/"));
    }

    #[test]
    fn skills_copy_omitted_when_no_agent() {
        let skills = vec!["my-skill".to_string()];
        let content = generate(&ubuntu_config("24.04"), None, &[], false, &skills).unwrap();
        assert!(!content.contains("skills/"));
    }

    #[test]
    fn skills_copy_appears_after_user_sandbox() {
        let skills = vec!["my-skill".to_string()];
        let content = generate(
            &ubuntu_config("24.04"),
            Some(&MockAgent),
            &[],
            false,
            &skills,
        )
        .unwrap();
        let user_pos = content.find("USER sandbox").unwrap();
        let skills_pos = content
            .find("COPY --chown=sandbox:sandbox skills/my-skill/")
            .unwrap();
        assert!(
            skills_pos > user_pos,
            "skills COPY must appear after USER sandbox"
        );
    }

    #[test]
    fn skills_copy_appears_before_agent_install() {
        let skills = vec!["my-skill".to_string()];
        let content = generate(
            &ubuntu_config("24.04"),
            Some(&MockAgent),
            &[],
            false,
            &skills,
        )
        .unwrap();
        let skills_pos = content
            .find("COPY --chown=sandbox:sandbox skills/my-skill/")
            .unwrap();
        let install_pos = content.find("RUN echo mock-agent").unwrap();
        assert!(
            skills_pos < install_pos,
            "skills COPY must appear before agent install"
        );
    }

    #[test]
    fn multiple_skills_each_get_copy_instruction() {
        let skills = vec!["skill-a".to_string(), "skill-b".to_string()];
        let content = generate(
            &ubuntu_config("24.04"),
            Some(&MockAgent),
            &[],
            false,
            &skills,
        )
        .unwrap();
        assert!(content.contains("COPY --chown=sandbox:sandbox skills/skill-a/"));
        assert!(content.contains("COPY --chown=sandbox:sandbox skills/skill-b/"));
    }
}
