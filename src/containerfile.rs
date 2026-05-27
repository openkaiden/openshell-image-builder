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

pub fn generate(config: &Config, agent: Option<&dyn Agent>) -> Result<String, ContainerfileError> {
    match config.base_image.image.as_str() {
        "ubuntu" => Ok(ubuntu(config, agent)),
        image => Err(ContainerfileError::NotSupported {
            image: image.to_string(),
        }),
    }
}

fn ubuntu(config: &Config, agent: Option<&dyn Agent>) -> String {
    let tag = &config.base_image.tag;
    let agent_section = agent
        .map(|a| format!("{}\n\n", a.install()))
        .unwrap_or_default();
    format!(
        r#"# System base
FROM docker.io/library/ubuntu:{tag} AS system

ENV DEBIAN_FRONTEND=noninteractive \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1

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

# Final base image
FROM system AS final

RUN printf 'export PS1="\\u@\\h:\\w\\$ "\n' \
        > /sandbox/.bashrc && \
    printf '[ -f ~/.bashrc ] && . ~/.bashrc\n' > /sandbox/.profile && \
    chown sandbox:sandbox /sandbox/.bashrc /sandbox/.profile && \
    chown -R sandbox:sandbox /sandbox

USER sandbox

{agent_section}ENTRYPOINT ["/bin/bash"]
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
        fn install(&self) -> String {
            "RUN echo mock-agent".to_string()
        }
    }

    #[test]
    fn ubuntu_generates_successfully() {
        let config = ubuntu_config("noble-20251013");
        assert!(generate(&config, None).is_ok());
    }

    #[test]
    fn ubuntu_containerfile_contains_tag() {
        let config = ubuntu_config("noble-20251013");
        let content = generate(&config, None).unwrap();
        assert!(content.contains("FROM docker.io/library/ubuntu:noble-20251013 AS system"));
    }

    #[test]
    fn ubuntu_containerfile_tag_is_substituted() {
        let content = generate(&ubuntu_config("24.04"), None).unwrap();
        assert!(content.contains("FROM docker.io/library/ubuntu:24.04 AS system"));
        assert!(!content.contains("{tag}"));
    }

    #[test]
    fn ubuntu_with_agent_includes_install() {
        let content = generate(&ubuntu_config("noble-20251013"), Some(&MockAgent)).unwrap();
        assert!(content.contains("RUN echo mock-agent"));
    }

    #[test]
    fn ubuntu_agent_install_runs_as_sandbox_user() {
        let content = generate(&ubuntu_config("noble-20251013"), Some(&MockAgent)).unwrap();
        let user_pos = content.find("USER sandbox").unwrap();
        let install_pos = content.find("RUN echo mock-agent").unwrap();
        assert!(
            install_pos > user_pos,
            "agent install must appear after USER sandbox"
        );
    }

    #[test]
    fn ubuntu_without_agent_omits_install() {
        let content = generate(&ubuntu_config("noble-20251013"), None).unwrap();
        assert!(!content.contains("RUN echo mock-agent"));
    }

    #[test]
    fn fedora_returns_not_supported() {
        let err = generate(&fedora_config(), None).unwrap_err();
        assert_eq!(
            err,
            ContainerfileError::NotSupported {
                image: "fedora".to_string()
            }
        );
    }

    #[test]
    fn not_supported_error_message() {
        let err = ContainerfileError::NotSupported {
            image: "fedora".to_string(),
        };
        assert_eq!(err.to_string(), "base image 'fedora' is not supported");
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
        let err = generate(&config, None).unwrap_err();
        assert_eq!(
            err,
            ContainerfileError::NotSupported {
                image: "centos".to_string()
            }
        );
    }
}
