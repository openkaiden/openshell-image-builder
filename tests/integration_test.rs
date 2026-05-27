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

use std::process::{Command, Output};

struct ImageGuard(String);

impl Drop for ImageGuard {
    fn drop(&mut self) {
        Command::new("podman")
            .args(["rmi", "--force", &self.0])
            .status()
            .ok();
    }
}

fn build_image(name: &str, extra_args: &[&str]) -> ImageGuard {
    let tag = format!("openshell-test-{name}:integration");
    let binary = env!("CARGO_BIN_EXE_openshell-image-builder");
    let status = Command::new(binary)
        .args(extra_args)
        .arg(&tag)
        .status()
        .expect("binary should run");
    assert!(status.success(), "image build failed for tag {tag}");
    ImageGuard(tag)
}

fn run_in_image(image: &str, cmd: &str) -> Output {
    Command::new("podman")
        .args(["run", "--rm", image, "-c", cmd])
        .output()
        .expect("podman run should execute")
}

#[test]
#[ignore]
fn default_image_users_and_groups_exist() {
    let guard = build_image("users", &[]);

    for user in ["sandbox", "supervisor"] {
        let out = run_in_image(&guard.0, &format!("id {user}"));
        assert!(out.status.success(), "{user} user not found in image");
    }

    for group in ["sandbox", "supervisor"] {
        let out = run_in_image(&guard.0, &format!("getent group {group}"));
        assert!(out.status.success(), "{group} group not found in image");
    }

    let out = run_in_image(&guard.0, "whoami");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "sandbox",
        "default image user is not sandbox"
    );

    let out = run_in_image(&guard.0, "echo $HOME");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "/sandbox",
        "sandbox home directory is not /sandbox"
    );
}

#[test]
#[ignore]
fn default_image_packages_installed() {
    let guard = build_image("packages", &[]);

    for pkg in ["curl", "ip", "ping", "traceroute"] {
        let out = run_in_image(&guard.0, &format!("which {pkg}"));
        assert!(out.status.success(), "{pkg} not found in image");
    }
}

#[test]
#[ignore]
fn without_agent_claude_is_not_in_path() {
    let guard = build_image("no-agent", &[]);
    let out = run_in_image(&guard.0, "which claude");
    assert!(
        !out.status.success(),
        "claude should not be in PATH without --agent claude"
    );
}

#[test]
#[ignore]
fn with_claude_agent_claude_is_in_path() {
    let guard = build_image("with-claude", &["--agent", "claude"]);
    let out = run_in_image(&guard.0, "which claude");
    assert!(
        out.status.success(),
        "claude not found in PATH with --agent claude"
    );
}

#[test]
#[ignore]
fn default_image_bash_entrypoint() {
    let guard = build_image("entrypoint", &[]);
    let out = Command::new("podman")
        .args([
            "inspect",
            "--format",
            "{{json .Config.Entrypoint}}",
            &guard.0,
        ])
        .output()
        .expect("podman inspect should execute");
    assert!(out.status.success(), "podman inspect failed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("/bin/bash"),
        "expected /bin/bash entrypoint, got: {stdout}"
    );
}
