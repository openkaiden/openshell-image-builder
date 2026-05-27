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

use std::io::Write;
use std::process::{Command, Output};
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Image build helpers
// ---------------------------------------------------------------------------

fn fedora_config_file() -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    writeln!(
        f,
        "[openshell_image_builder.base_image]\nimage = \"fedora\"\ntag = \"latest\""
    )
    .unwrap();
    f
}

fn build_image(tag: &str, extra_args: &[&str]) -> String {
    let binary = env!("CARGO_BIN_EXE_openshell-image-builder");
    let status = Command::new(binary)
        .args(extra_args)
        .arg(tag)
        .status()
        .expect("binary should run");
    assert!(status.success(), "image build failed for tag {tag}");
    tag.to_string()
}

fn run_in_image(image: &str, cmd: &str) -> Output {
    Command::new("podman")
        .args(["run", "--rm", image, "-c", cmd])
        .output()
        .expect("podman run should execute")
}

// ---------------------------------------------------------------------------
// One OnceLock per image variant — each image is built at most once
// ---------------------------------------------------------------------------

static UBUNTU_IMAGE: OnceLock<String> = OnceLock::new();
static UBUNTU_CLAUDE_IMAGE: OnceLock<String> = OnceLock::new();
static FEDORA_IMAGE: OnceLock<String> = OnceLock::new();
static FEDORA_CLAUDE_IMAGE: OnceLock<String> = OnceLock::new();

fn ubuntu_image() -> &'static str {
    UBUNTU_IMAGE.get_or_init(|| build_image("openshell-test-ubuntu:integration", &[]))
}

fn ubuntu_claude_image() -> &'static str {
    UBUNTU_CLAUDE_IMAGE.get_or_init(|| {
        build_image(
            "openshell-test-ubuntu-claude:integration",
            &["--agent", "claude"],
        )
    })
}

fn fedora_image() -> &'static str {
    FEDORA_IMAGE.get_or_init(|| {
        let config = fedora_config_file();
        build_image(
            "openshell-test-fedora:integration",
            &["--config", config.path().to_str().unwrap()],
        )
    })
}

fn fedora_claude_image() -> &'static str {
    FEDORA_CLAUDE_IMAGE.get_or_init(|| {
        let config = fedora_config_file();
        build_image(
            "openshell-test-fedora-claude:integration",
            &[
                "--config",
                config.path().to_str().unwrap(),
                "--agent",
                "claude",
            ],
        )
    })
}

// ---------------------------------------------------------------------------
// Shared assertion helpers
// ---------------------------------------------------------------------------

fn check_users_and_groups(image: &str) {
    for user in ["sandbox", "supervisor"] {
        let out = run_in_image(image, &format!("id {user}"));
        assert!(out.status.success(), "{user} user not found in image");
    }

    for group in ["sandbox", "supervisor"] {
        let out = run_in_image(image, &format!("getent group {group}"));
        assert!(out.status.success(), "{group} group not found in image");
    }

    let out = run_in_image(image, "whoami");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "sandbox",
        "default image user is not sandbox"
    );

    let out = run_in_image(image, "echo $HOME");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "/sandbox",
        "sandbox home directory is not /sandbox"
    );
}

fn check_packages(image: &str) {
    for pkg in ["curl", "ip", "ping", "traceroute"] {
        let out = run_in_image(image, &format!("which {pkg}"));
        assert!(out.status.success(), "{pkg} not found in image");
    }
}

fn check_bash_entrypoint(image: &str) {
    let out = Command::new("podman")
        .args(["inspect", "--format", "{{json .Config.Entrypoint}}", image])
        .output()
        .expect("podman inspect should execute");
    assert!(out.status.success(), "podman inspect failed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("/bin/bash"),
        "expected /bin/bash entrypoint, got: {stdout}"
    );
}

fn check_claude_in_path(image: &str, expected: bool) {
    let out = run_in_image(image, "which claude");
    if expected {
        assert!(out.status.success(), "claude not found in PATH");
    } else {
        assert!(!out.status.success(), "claude should not be in PATH");
    }
}

// ---------------------------------------------------------------------------
// Matrix: base_image × agent — one test module per variant
// ---------------------------------------------------------------------------

macro_rules! image_tests {
    ($mod_name:ident, $image_fn:ident, has_claude: $has_claude:literal) => {
        mod $mod_name {
            use super::*;

            #[test]
            #[ignore]
            fn users_and_groups_exist() {
                check_users_and_groups($image_fn());
            }

            #[test]
            #[ignore]
            fn packages_installed() {
                check_packages($image_fn());
            }

            #[test]
            #[ignore]
            fn bash_entrypoint() {
                check_bash_entrypoint($image_fn());
            }

            #[test]
            #[ignore]
            fn claude_in_path() {
                check_claude_in_path($image_fn(), $has_claude);
            }
        }
    };
}

image_tests!(ubuntu,        ubuntu_image,        has_claude: false);
image_tests!(ubuntu_claude, ubuntu_claude_image, has_claude: true);
image_tests!(fedora,        fedora_image,        has_claude: false);
image_tests!(fedora_claude, fedora_claude_image, has_claude: true);

// ---------------------------------------------------------------------------
// Workspace helpers for feature-based builds
// ---------------------------------------------------------------------------

fn workspace_dir(workspace_json: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let kaiden = dir.path().join(".kaiden");
    std::fs::create_dir_all(&kaiden).unwrap();
    std::fs::write(kaiden.join("workspace.json"), workspace_json).unwrap();
    dir
}

fn build_image_with_workspace(tag: &str, workspace_json: &str, extra_args: &[&str]) -> String {
    let dir = workspace_dir(workspace_json);
    let binary = env!("CARGO_BIN_EXE_openshell-image-builder");
    let status = Command::new(binary)
        .current_dir(dir.path())
        .args(extra_args)
        .arg(tag)
        .status()
        .expect("binary should run");
    assert!(status.success(), "image build failed for tag {tag}");
    tag.to_string()
}

// ---------------------------------------------------------------------------
// Feature image singletons — ubuntu (default) and fedora variants
// ---------------------------------------------------------------------------

static FEATURE_COMMON_UTILS_UBUNTU_IMAGE: OnceLock<String> = OnceLock::new();
static FEATURE_NODE_UBUNTU_IMAGE: OnceLock<String> = OnceLock::new();
static FEATURE_PYTHON_UBUNTU_IMAGE: OnceLock<String> = OnceLock::new();
static FEATURE_COMMON_UTILS_FEDORA_IMAGE: OnceLock<String> = OnceLock::new();
static FEATURE_NODE_FEDORA_IMAGE: OnceLock<String> = OnceLock::new();
static FEATURE_PYTHON_FEDORA_IMAGE: OnceLock<String> = OnceLock::new();
static FEATURE_LOCAL_UBUNTU_IMAGE: OnceLock<String> = OnceLock::new();
static FEATURE_LOCAL_FEDORA_IMAGE: OnceLock<String> = OnceLock::new();

const COMMON_UTILS_WORKSPACE: &str = r#"{
    "features": {
        "ghcr.io/devcontainers/features/common-utils:2": {
            "installZsh": true
        }
    }
}"#;

const NODE_WORKSPACE: &str = r#"{
    "features": {
        "ghcr.io/devcontainers/features/node:1": {
            "version": "22"
        }
    }
}"#;

const PYTHON_WORKSPACE: &str = r#"{
    "features": {
        "ghcr.io/devcontainers/features/python:1": {
            "version": "os-provided",
            "installTools": true
        }
    }
}"#;

fn feature_common_utils_ubuntu_image() -> &'static str {
    FEATURE_COMMON_UTILS_UBUNTU_IMAGE.get_or_init(|| {
        build_image_with_workspace(
            "openshell-test-feature-common-utils-ubuntu:integration",
            COMMON_UTILS_WORKSPACE,
            &[],
        )
    })
}

fn feature_node_ubuntu_image() -> &'static str {
    FEATURE_NODE_UBUNTU_IMAGE.get_or_init(|| {
        build_image_with_workspace(
            "openshell-test-feature-node-ubuntu:integration",
            NODE_WORKSPACE,
            &[],
        )
    })
}

fn feature_python_ubuntu_image() -> &'static str {
    FEATURE_PYTHON_UBUNTU_IMAGE.get_or_init(|| {
        build_image_with_workspace(
            "openshell-test-feature-python-ubuntu:integration",
            PYTHON_WORKSPACE,
            &[],
        )
    })
}

fn feature_common_utils_fedora_image() -> &'static str {
    FEATURE_COMMON_UTILS_FEDORA_IMAGE.get_or_init(|| {
        let config = fedora_config_file();
        build_image_with_workspace(
            "openshell-test-feature-common-utils-fedora:integration",
            COMMON_UTILS_WORKSPACE,
            &["--config", config.path().to_str().unwrap()],
        )
    })
}

fn feature_node_fedora_image() -> &'static str {
    FEATURE_NODE_FEDORA_IMAGE.get_or_init(|| {
        let config = fedora_config_file();
        build_image_with_workspace(
            "openshell-test-feature-node-fedora:integration",
            NODE_WORKSPACE,
            &["--config", config.path().to_str().unwrap()],
        )
    })
}

fn feature_python_fedora_image() -> &'static str {
    FEATURE_PYTHON_FEDORA_IMAGE.get_or_init(|| {
        let config = fedora_config_file();
        build_image_with_workspace(
            "openshell-test-feature-python-fedora:integration",
            PYTHON_WORKSPACE,
            &["--config", config.path().to_str().unwrap()],
        )
    })
}

fn local_feature_workspace_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let kaiden = dir.path().join(".kaiden");
    let feature_dir = kaiden.join("my-feature");
    std::fs::create_dir_all(&feature_dir).unwrap();
    std::fs::write(
        kaiden.join("workspace.json"),
        r#"{"features": {"./my-feature": {"filename": "hello-from-feature"}}}"#,
    )
    .unwrap();
    std::fs::write(
        feature_dir.join("devcontainer-feature.json"),
        r#"{"id": "my-feature", "version": "1.0.0", "name": "My Test Feature", "options": {"filename": {"type": "string", "default": "default-filename"}}}"#,
    )
    .unwrap();
    std::fs::write(
        feature_dir.join("install.sh"),
        "#!/bin/sh\nsh \"$(dirname \"$0\")/main.sh\"\n",
    )
    .unwrap();
    std::fs::write(
        feature_dir.join("main.sh"),
        "#!/bin/sh\ntouch \"$_REMOTE_USER_HOME/$FILENAME\"\n",
    )
    .unwrap();
    dir
}

fn build_image_with_local_feature(tag: &str, extra_args: &[&str]) -> String {
    let dir = local_feature_workspace_dir();
    let binary = env!("CARGO_BIN_EXE_openshell-image-builder");
    let status = Command::new(binary)
        .current_dir(dir.path())
        .args(extra_args)
        .arg(tag)
        .status()
        .expect("binary should run");
    assert!(status.success(), "image build failed for tag {tag}");
    tag.to_string()
}

fn feature_local_ubuntu_image() -> &'static str {
    FEATURE_LOCAL_UBUNTU_IMAGE.get_or_init(|| {
        build_image_with_local_feature("openshell-test-feature-local-ubuntu:integration", &[])
    })
}

fn feature_local_fedora_image() -> &'static str {
    FEATURE_LOCAL_FEDORA_IMAGE.get_or_init(|| {
        let config = fedora_config_file();
        build_image_with_local_feature(
            "openshell-test-feature-local-fedora:integration",
            &["--config", config.path().to_str().unwrap()],
        )
    })
}

// ---------------------------------------------------------------------------
// Feature integration tests — one macro per feature, instantiated per base image
// ---------------------------------------------------------------------------

macro_rules! feature_common_utils_tests {
    ($mod_name:ident, $image_fn:ident, $base_image_fn:ident) => {
        mod $mod_name {
            use super::*;

            #[test]
            #[ignore]
            fn zsh_installed() {
                let out = run_in_image($image_fn(), "which zsh");
                assert!(out.status.success(), "zsh not found in image");
            }

            #[test]
            #[ignore]
            fn zsh_not_in_base_image() {
                let out = run_in_image($base_image_fn(), "which zsh");
                assert!(!out.status.success(), "zsh should not be in base image");
            }
        }
    };
}

macro_rules! feature_node_tests {
    ($mod_name:ident, $image_fn:ident, $base_image_fn:ident) => {
        mod $mod_name {
            use super::*;

            #[test]
            #[ignore]
            fn node_in_path() {
                let out = run_in_image($image_fn(), "node --version");
                assert!(out.status.success(), "node not found in PATH");
            }

            #[test]
            #[ignore]
            fn node_not_in_base_image() {
                let out = run_in_image($base_image_fn(), "which node");
                assert!(!out.status.success(), "node should not be in base image");
            }

            #[test]
            #[ignore]
            fn npm_in_path() {
                let out = run_in_image($image_fn(), "npm --version");
                assert!(out.status.success(), "npm not found in PATH");
            }

            #[test]
            #[ignore]
            fn npm_not_in_base_image() {
                let out = run_in_image($base_image_fn(), "which npm");
                assert!(!out.status.success(), "npm should not be in base image");
            }
        }
    };
}

macro_rules! feature_python_tests {
    ($mod_name:ident, $image_fn:ident, $base_image_fn:ident) => {
        mod $mod_name {
            use super::*;

            #[test]
            #[ignore]
            fn python3_available() {
                let out = run_in_image($image_fn(), "python3 --version");
                assert!(out.status.success(), "python3 not found in image");
            }

            #[test]
            #[ignore]
            fn flake8_installed() {
                let out = run_in_image($image_fn(), "which flake8");
                assert!(out.status.success(), "flake8 not found in PATH");
            }

            #[test]
            #[ignore]
            fn flake8_not_in_base_image() {
                let out = run_in_image($base_image_fn(), "which flake8");
                assert!(!out.status.success(), "flake8 should not be in base image");
            }

            #[test]
            #[ignore]
            fn pylint_installed() {
                let out = run_in_image($image_fn(), "which pylint");
                assert!(out.status.success(), "pylint not found in PATH");
            }

            #[test]
            #[ignore]
            fn pylint_not_in_base_image() {
                let out = run_in_image($base_image_fn(), "which pylint");
                assert!(!out.status.success(), "pylint should not be in base image");
            }
        }
    };
}

macro_rules! feature_local_tests {
    ($mod_name:ident, $image_fn:ident, $base_image_fn:ident) => {
        mod $mod_name {
            use super::*;

            #[test]
            #[ignore]
            fn file_created_by_feature() {
                let out = run_in_image($image_fn(), "test -f /sandbox/hello-from-feature");
                assert!(out.status.success(), "file not created by local feature");
            }

            #[test]
            #[ignore]
            fn file_not_in_base_image() {
                let out = run_in_image($base_image_fn(), "test -f /sandbox/hello-from-feature");
                assert!(!out.status.success(), "file should not exist in base image");
            }
        }
    };
}

feature_local_tests!(
    feature_local_ubuntu,
    feature_local_ubuntu_image,
    ubuntu_image
);
feature_local_tests!(
    feature_local_fedora,
    feature_local_fedora_image,
    fedora_image
);

feature_common_utils_tests!(
    feature_common_utils_ubuntu,
    feature_common_utils_ubuntu_image,
    ubuntu_image
);
feature_common_utils_tests!(
    feature_common_utils_fedora,
    feature_common_utils_fedora_image,
    fedora_image
);
feature_node_tests!(feature_node_ubuntu, feature_node_ubuntu_image, ubuntu_image);
feature_node_tests!(feature_node_fedora, feature_node_fedora_image, fedora_image);
feature_python_tests!(
    feature_python_ubuntu,
    feature_python_ubuntu_image,
    ubuntu_image
);
feature_python_tests!(
    feature_python_fedora,
    feature_python_fedora_image,
    fedora_image
);

// ---------------------------------------------------------------------------
// Cleanup — runs when the test process exits, after all tests complete
// ---------------------------------------------------------------------------

#[ctor::dtor]
fn cleanup_images() {
    for tag in [
        "openshell-test-ubuntu:integration",
        "openshell-test-ubuntu-claude:integration",
        "openshell-test-fedora:integration",
        "openshell-test-fedora-claude:integration",
        "openshell-test-feature-common-utils-ubuntu:integration",
        "openshell-test-feature-node-ubuntu:integration",
        "openshell-test-feature-python-ubuntu:integration",
        "openshell-test-feature-common-utils-fedora:integration",
        "openshell-test-feature-node-fedora:integration",
        "openshell-test-feature-python-fedora:integration",
        "openshell-test-feature-local-ubuntu:integration",
        "openshell-test-feature-local-fedora:integration",
    ] {
        Command::new("podman")
            .args(["rmi", "--force", tag])
            .status()
            .ok();
    }
}
