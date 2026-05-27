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
use std::path::Path;
use std::process::{Command, ExitStatus};

use tempfile::NamedTempFile;

pub trait Runner {
    fn run(&self, cmd: &mut Command) -> std::io::Result<ExitStatus>;
}

pub struct PodmanRunner;

impl Runner for PodmanRunner {
    fn run(&self, cmd: &mut Command) -> std::io::Result<ExitStatus> {
        cmd.status()
    }
}

#[derive(Debug)]
pub enum BuildError {
    Io(std::io::Error),
    Failed { exit_code: Option<i32> },
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::Io(e) => write!(f, "I/O error: {e}"),
            BuildError::Failed {
                exit_code: Some(code),
            } => write!(f, "podman build failed with exit code {code}"),
            BuildError::Failed { exit_code: None } => {
                write!(f, "podman build was terminated by a signal")
            }
        }
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BuildError::Io(e) => Some(e),
            BuildError::Failed { .. } => None,
        }
    }
}

impl From<std::io::Error> for BuildError {
    fn from(e: std::io::Error) -> Self {
        BuildError::Io(e)
    }
}

pub fn build(
    containerfile: &str,
    tag: &str,
    runner: &impl Runner,
    context_dir: &Path,
) -> Result<(), BuildError> {
    let mut tmpfile = NamedTempFile::new()?;
    tmpfile.write_all(containerfile.as_bytes())?;

    log::debug!(
        "running: podman build -f {} -t {tag} {}",
        tmpfile.path().display(),
        context_dir.display()
    );

    let mut cmd = Command::new("podman");
    cmd.args(["build", "-f"])
        .arg(tmpfile.path())
        .args(["-t", tag])
        .arg(context_dir);

    let status = runner.run(&mut cmd)?;

    if status.success() {
        Ok(())
    } else {
        Err(BuildError::Failed {
            exit_code: status.code(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    struct FakeRunner(fn() -> io::Result<ExitStatus>);

    impl Runner for FakeRunner {
        fn run(&self, _cmd: &mut Command) -> io::Result<ExitStatus> {
            self.0()
        }
    }

    fn exit_with(code: i32) -> ExitStatus {
        Command::new("sh")
            .args(["-c", &format!("exit {code}")])
            .status()
            .unwrap()
    }

    const CONTAINERFILE: &str = "FROM scratch";
    const TAG: &str = "test:latest";

    #[test]
    fn build_succeeds() {
        let runner = FakeRunner(|| Ok(exit_with(0)));
        assert!(build(CONTAINERFILE, TAG, &runner, Path::new(".")).is_ok());
    }

    #[test]
    fn build_fails_with_exit_code() {
        let runner = FakeRunner(|| Ok(exit_with(1)));
        let err = build(CONTAINERFILE, TAG, &runner, Path::new(".")).unwrap_err();
        assert!(matches!(err, BuildError::Failed { exit_code: Some(1) }));
    }

    #[test]
    fn build_propagates_io_error() {
        let runner =
            FakeRunner(|| Err(io::Error::new(io::ErrorKind::NotFound, "podman not found")));
        let err = build(CONTAINERFILE, TAG, &runner, Path::new(".")).unwrap_err();
        assert!(matches!(err, BuildError::Io(_)));
    }

    #[cfg(unix)]
    #[test]
    fn build_signal_killed() {
        use std::os::unix::process::ExitStatusExt;
        let runner = FakeRunner(|| Ok(ExitStatus::from_raw(9)));
        let err = build(CONTAINERFILE, TAG, &runner, Path::new(".")).unwrap_err();
        assert!(matches!(err, BuildError::Failed { exit_code: None }));
    }

    #[test]
    fn io_error_display() {
        let err = BuildError::Io(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        assert_eq!(err.to_string(), "I/O error: file not found");
    }

    #[test]
    fn failed_with_exit_code_display() {
        let err = BuildError::Failed { exit_code: Some(1) };
        assert_eq!(err.to_string(), "podman build failed with exit code 1");
    }

    #[test]
    fn failed_without_exit_code_display() {
        let err = BuildError::Failed { exit_code: None };
        assert_eq!(err.to_string(), "podman build was terminated by a signal");
    }

    #[test]
    fn from_io_error_wraps_correctly() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let build_err = BuildError::from(io_err);
        assert!(matches!(build_err, BuildError::Io(_)));
    }
}
