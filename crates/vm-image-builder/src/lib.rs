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

//! Build container images inside a lightweight Linux microVM.
//!
//! Where [`container-image-builder`] shells out to a container CLI already
//! installed on the host, this crate boots a microVM with [libkrun] and runs
//! `buildah` inside it. Nothing is installed on the host beyond libkrun itself,
//! and the build cannot touch host state: the VM runs its own Linux kernel with
//! its own process namespace and sees only the three directories explicitly
//! shared with it.
//!
//! The result is a **flattened rootfs tarball**, not an image in a local image
//! store — the VM has no access to the host's container storage.
//!
//! # How a build runs
//!
//! 1. The Containerfile is written into the build context as `Containerfile`.
//! 2. A microVM is configured with three [virtio-fs] shares: the build rootfs
//!    (mounted as `/`), the build context, and the output directory.
//! 3. The VM execs `/usr/local/bin/vm-build`, a script baked into the build
//!    rootfs, which mounts the shares and runs `buildah bud`.
//! 4. `buildah` flattens the built image into a tarball written to the shared
//!    output directory, where it appears on the host.
//!
//! # Build rootfs
//!
//! The VM boots from a directory on the host that must contain a Linux
//! userspace with `buildah` installed and the `vm-build` helper at
//! `/usr/local/bin/vm-build`. The `vm-image/` directory in this crate has a
//! `Containerfile` and a `make-rootfs.sh` script that produce a suitable one.
//!
//! # Platform support
//!
//! libkrun's macOS backend uses Apple's Hypervisor.framework, so real builds
//! require macOS on Apple Silicon, the `krun` cargo feature, and a binary
//! signed with the `com.apple.security.hypervisor` entitlement. The crate
//! compiles on every platform regardless; [`KrunRunner`] returns
//! [`VmBuildError::Unsupported`] where any of those are missing.
//!
//! # Quick start
//!
//! ```no_run
//! use std::path::Path;
//! use vm_image_builder::{KrunRunner, VmConfig, build};
//!
//! let config = VmConfig::new(Path::new("/opt/openshell/vm-rootfs"));
//!
//! // Fail fast if the rootfs is missing or unusable.
//! config.check_rootfs()?;
//!
//! build(
//!     "FROM alpine:latest\nRUN echo hello",
//!     "myimage:latest",
//!     &config,
//!     &KrunRunner,
//!     Path::new("./context"),
//!     Path::new("./myimage-latest.tar"),
//! )?;
//! # Ok::<(), vm_image_builder::VmBuildError>(())
//! ```
//!
//! # Testability
//!
//! [`VmRunner`] is a trait so that tests can assert on the fully resolved
//! [`VmBuild`] without booting a VM. See the trait documentation for an
//! example.
//!
//! [`container-image-builder`]: https://docs.rs/container-image-builder
//! [libkrun]: https://github.com/containers/libkrun
//! [virtio-fs]: https://virtio-fs.gitlab.io/

use std::path::{Path, PathBuf};

#[cfg(all(feature = "krun", target_os = "macos", target_arch = "aarch64"))]
mod krun;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default number of vCPUs given to the build VM.
pub const DEFAULT_CPUS: u8 = 2;

/// Default RAM in MiB given to the build VM.
///
/// `buildah` uses the `vfs` storage driver inside the VM (virtio-fs does not
/// support overlayfs), which stores a full copy of every layer, so builds need
/// noticeably more memory than the same build under overlayfs.
pub const DEFAULT_MEMORY_MIB: u32 = 4096;

/// Path of the build helper script inside the build rootfs.
pub const VM_BUILD_SCRIPT: &str = "/usr/local/bin/vm-build";

/// virtio-fs tag the bundled kernel mounts as `/`, libkrun's `KRUN_FS_ROOT_TAG`.
pub const TAG_ROOT: &str = "/dev/root";

/// virtio-fs tag for the build context.
///
/// Unlike [`TAG_ROOT`], this share is not mounted by the kernel — the
/// [`VM_BUILD_SCRIPT`] helper mounts it at `/build/context`, so the tag is
/// part of the contract between this crate and that script.
pub const TAG_CONTEXT: &str = "openshell-context";

/// virtio-fs tag for the output directory, mounted at `/build/output` by the
/// [`VM_BUILD_SCRIPT`] helper.
pub const TAG_OUTPUT: &str = "openshell-output";

// ---------------------------------------------------------------------------
// VmConfig
// ---------------------------------------------------------------------------

/// The build rootfs and the resources given to the VM that boots from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmConfig {
    /// Host directory used as the VM's root filesystem. Must contain `buildah`
    /// and the [`VM_BUILD_SCRIPT`] helper.
    pub rootfs: PathBuf,
    /// Number of vCPUs.
    pub cpus: u8,
    /// RAM in MiB.
    pub memory_mib: u32,
}

impl VmConfig {
    /// Creates a config for `rootfs` with [`DEFAULT_CPUS`] and
    /// [`DEFAULT_MEMORY_MIB`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// use vm_image_builder::{DEFAULT_CPUS, DEFAULT_MEMORY_MIB, VmConfig};
    ///
    /// let config = VmConfig::new(Path::new("/tmp/rootfs"));
    /// assert_eq!(config.cpus, DEFAULT_CPUS);
    /// assert_eq!(config.memory_mib, DEFAULT_MEMORY_MIB);
    /// ```
    pub fn new(rootfs: &Path) -> Self {
        VmConfig {
            rootfs: rootfs.to_path_buf(),
            cpus: DEFAULT_CPUS,
            memory_mib: DEFAULT_MEMORY_MIB,
        }
    }

    /// Returns `Ok(())` when [`rootfs`] looks like a usable build rootfs.
    ///
    /// This is the VM counterpart of `ContainerCli::check_in_path`: it lets the
    /// caller fail before doing any staging work. It checks that the directory
    /// exists and that the [`VM_BUILD_SCRIPT`] helper is present inside it —
    /// the two failures that are otherwise only reported from inside the VM,
    /// where the diagnostics are far worse.
    ///
    /// # Errors
    ///
    /// Returns [`VmBuildError::Rootfs`] if the directory is missing, is not a
    /// directory, or has no `vm-build` helper.
    ///
    /// [`rootfs`]: VmConfig::rootfs
    pub fn check_rootfs(&self) -> Result<(), VmBuildError> {
        if !self.rootfs.is_dir() {
            return Err(VmBuildError::Rootfs {
                path: self.rootfs.clone(),
                reason: "not a directory — build it with vm-image/make-rootfs.sh",
            });
        }
        // VM_BUILD_SCRIPT is absolute (a path inside the VM); strip the
        // leading separator so it joins onto the host-side rootfs.
        let helper = self.rootfs.join(VM_BUILD_SCRIPT.trim_start_matches('/'));
        if !helper.is_file() {
            return Err(VmBuildError::Rootfs {
                path: self.rootfs.clone(),
                reason: "missing /usr/local/bin/vm-build — not a build rootfs",
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// VmBuild
// ---------------------------------------------------------------------------

/// A build request with every host path resolved and validated.
///
/// [`build`] produces one of these and hands it to a [`VmRunner`]. All paths
/// are canonical, so they can be passed to libkrun as-is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmBuild {
    /// Canonical path to the build rootfs, shared as `/` inside the VM.
    pub rootfs: PathBuf,
    /// Canonical path to the build context, mounted at `/build/context`.
    pub context: PathBuf,
    /// Canonical path to the output directory, mounted at `/build/output`.
    pub output_dir: PathBuf,
    /// Filename of the tarball written inside [`output_dir`].
    ///
    /// [`output_dir`]: VmBuild::output_dir
    pub output_filename: String,
    /// Image tag `buildah` builds under, inside the VM.
    pub tag: String,
    /// Number of vCPUs.
    pub cpus: u8,
    /// RAM in MiB.
    pub memory_mib: u32,
}

impl VmBuild {
    /// Returns the path the Containerfile has *inside* the VM.
    ///
    /// [`build`] always writes the Containerfile into the build context, so it
    /// is reachable through the context share and needs no mount of its own.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::PathBuf;
    /// # use vm_image_builder::VmBuild;
    /// # let build = VmBuild {
    /// #     rootfs: PathBuf::from("/rootfs"),
    /// #     context: PathBuf::from("/ctx"),
    /// #     output_dir: PathBuf::from("/out"),
    /// #     output_filename: "out.tar".to_string(),
    /// #     tag: "t:latest".to_string(),
    /// #     cpus: 2,
    /// #     memory_mib: 4096,
    /// # };
    /// assert_eq!(build.containerfile_vm_path(), "/build/context/Containerfile");
    /// ```
    pub fn containerfile_vm_path(&self) -> String {
        format!("/build/context/{CONTAINERFILE_NAME}")
    }

    /// Returns the host path of the tarball this build produces.
    pub fn output_path(&self) -> PathBuf {
        self.output_dir.join(&self.output_filename)
    }
}

/// Name the Containerfile is written under inside the build context.
const CONTAINERFILE_NAME: &str = "Containerfile";

// ---------------------------------------------------------------------------
// VmBuildError
// ---------------------------------------------------------------------------

/// Errors that can occur during [`build`].
#[derive(Debug)]
pub enum VmBuildError {
    /// An I/O error occurred writing the Containerfile or creating the output
    /// directory.
    Io(std::io::Error),
    /// A path could not be resolved. `what` names the role of the path so the
    /// message can say which one, since they all look alike to the user.
    Path {
        what: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    /// The build rootfs is missing or is not a build rootfs.
    Rootfs { path: PathBuf, reason: &'static str },
    /// This binary cannot run VM builds — wrong platform, the `krun` feature is
    /// off, or the required entitlement is missing.
    Unsupported(&'static str),
    /// A libkrun call failed. `call` names the C function.
    Krun { call: &'static str, code: i32 },
    /// The build inside the VM exited non-zero.
    ///
    /// `exit_code` is `None` when the VM process was killed by a signal.
    Failed { exit_code: Option<i32> },
}

impl std::fmt::Display for VmBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmBuildError::Io(e) => write!(f, "I/O error: {e}"),
            VmBuildError::Path { what, path, source } => {
                write!(f, "{what} '{}': {source}", path.display())
            }
            VmBuildError::Rootfs { path, reason } => {
                write!(f, "VM rootfs '{}': {reason}", path.display())
            }
            VmBuildError::Unsupported(reason) => {
                write!(f, "VM builds are not available: {reason}")
            }
            VmBuildError::Krun { call, code } => {
                write!(f, "libkrun call {call} failed with code {code}")
            }
            VmBuildError::Failed {
                exit_code: Some(code),
            } => write!(f, "VM build failed with exit code {code}"),
            VmBuildError::Failed { exit_code: None } => {
                write!(f, "VM build was terminated by a signal")
            }
        }
    }
}

impl std::error::Error for VmBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VmBuildError::Io(e) => Some(e),
            VmBuildError::Path { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for VmBuildError {
    fn from(e: std::io::Error) -> Self {
        VmBuildError::Io(e)
    }
}

// ---------------------------------------------------------------------------
// VmRunner
// ---------------------------------------------------------------------------

/// Boots a microVM and runs a resolved [`VmBuild`] in it.
///
/// The trait exists so that production code uses [`KrunRunner`] while tests
/// substitute a fake that inspects the [`VmBuild`] without starting a VM.
///
/// # Implementing a fake runner
///
/// ```
/// use vm_image_builder::{VmBuild, VmBuildError, VmRunner};
///
/// struct FakeRunner;
///
/// impl VmRunner for FakeRunner {
///     fn run(&self, build: &VmBuild) -> Result<(), VmBuildError> {
///         assert_eq!(build.containerfile_vm_path(), "/build/context/Containerfile");
///         Ok(())
///     }
/// }
/// ```
pub trait VmRunner {
    /// Runs `build` to completion.
    fn run(&self, build: &VmBuild) -> Result<(), VmBuildError>;
}

// ---------------------------------------------------------------------------
// KrunRunner
// ---------------------------------------------------------------------------

/// The real [`VmRunner`]: drives libkrun.
///
/// Use this in production. For tests, implement [`VmRunner`] on a local fake
/// instead.
///
/// On a platform or build where libkrun is unavailable, [`run`] returns
/// [`VmBuildError::Unsupported`] rather than failing to compile, so the rest of
/// the program builds everywhere.
///
/// [`run`]: VmRunner::run
pub struct KrunRunner;

impl VmRunner for KrunRunner {
    #[cfg(all(feature = "krun", target_os = "macos", target_arch = "aarch64"))]
    fn run(&self, build: &VmBuild) -> Result<(), VmBuildError> {
        krun::run(build)
    }

    #[cfg(not(all(feature = "krun", target_os = "macos", target_arch = "aarch64")))]
    fn run(&self, _build: &VmBuild) -> Result<(), VmBuildError> {
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        return Err(VmBuildError::Unsupported(
            "libkrun requires macOS on Apple Silicon",
        ));
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        return Err(VmBuildError::Unsupported(
            "this binary was built without the 'vm' feature",
        ));
    }
}

// ---------------------------------------------------------------------------
// build
// ---------------------------------------------------------------------------

/// Builds a container image from an in-memory Containerfile inside a microVM
/// and writes the flattened result to `output`.
///
/// The Containerfile is written into `context_dir` as `Containerfile`, so the
/// VM reaches it through the context share and no extra mount is needed. Any
/// existing `Containerfile` in the context is overwritten.
///
/// `runner` abstracts VM execution so that callers can inject a fake in tests.
/// Pass [`KrunRunner`] in production.
///
/// # Arguments
///
/// - `containerfile` — the full Containerfile content (not a path).
/// - `tag` — the tag `buildah` builds under inside the VM.
/// - `config` — the build rootfs and VM resources.
/// - `runner` — runs the VM; use [`KrunRunner`] in production.
/// - `context_dir` — the build context, shared into the VM.
/// - `output` — host path for the flattened rootfs tarball. Its parent
///   directory is created if missing.
///
/// # Errors
///
/// - [`VmBuildError::Io`] if the Containerfile cannot be written or the output
///   directory cannot be created.
/// - [`VmBuildError::Path`] if the rootfs, context, or output directory cannot
///   be canonicalized, or `output` has no filename.
/// - [`VmBuildError::Unsupported`], [`VmBuildError::Krun`], or
///   [`VmBuildError::Failed`] from the runner.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use vm_image_builder::{KrunRunner, VmConfig, build};
///
/// build(
///     "FROM scratch",
///     "empty:latest",
///     &VmConfig::new(Path::new("/tmp/rootfs")),
///     &KrunRunner,
///     Path::new("."),
///     Path::new("./empty.tar"),
/// )?;
/// # Ok::<(), vm_image_builder::VmBuildError>(())
/// ```
pub fn build(
    containerfile: &str,
    tag: &str,
    config: &VmConfig,
    runner: &dyn VmRunner,
    context_dir: &Path,
    output: &Path,
) -> Result<(), VmBuildError> {
    std::fs::write(context_dir.join(CONTAINERFILE_NAME), containerfile)?;

    let rootfs = canonicalize(config.rootfs.as_path(), "VM rootfs")?;
    let context = canonicalize(context_dir, "build context")?;

    let output_filename = output
        .file_name()
        .ok_or_else(|| VmBuildError::Path {
            what: "output path has no filename",
            path: output.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "no filename"),
        })?
        .to_string_lossy()
        .into_owned();

    // An empty parent means a bare filename like "out.tar" — write it to the
    // current directory rather than to the filesystem root.
    let output_parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(output_parent)?;
    let output_dir = canonicalize(output_parent, "output directory")?;

    let build = VmBuild {
        rootfs,
        context,
        output_dir,
        output_filename,
        tag: tag.to_string(),
        cpus: config.cpus,
        memory_mib: config.memory_mib,
    };

    log::debug!(
        "building '{}' in a microVM ({} vCPUs, {} MiB): context={} rootfs={} output={}",
        build.tag,
        build.cpus,
        build.memory_mib,
        build.context.display(),
        build.rootfs.display(),
        build.output_path().display(),
    );

    runner.run(&build)
}

/// Canonicalizes `path`, tagging the error with what the path was for.
fn canonicalize(path: &Path, what: &'static str) -> Result<PathBuf, VmBuildError> {
    path.canonicalize().map_err(|source| VmBuildError::Path {
        what,
        path: path.to_path_buf(),
        source,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::sync::Mutex;

    const CONTAINERFILE: &str = "FROM scratch";
    const TAG: &str = "test:latest";

    /// Captures the `VmBuild` it is handed so tests can assert on it.
    struct CaptureRunner(Mutex<Option<VmBuild>>);

    impl CaptureRunner {
        fn new() -> Self {
            CaptureRunner(Mutex::new(None))
        }

        fn captured(&self) -> VmBuild {
            self.0
                .lock()
                .unwrap()
                .clone()
                .expect("runner was not called")
        }
    }

    impl VmRunner for CaptureRunner {
        fn run(&self, build: &VmBuild) -> Result<(), VmBuildError> {
            *self.0.lock().unwrap() = Some(build.clone());
            Ok(())
        }
    }

    /// Always fails, to check that `build` propagates runner errors.
    struct FailingRunner(fn() -> VmBuildError);

    impl VmRunner for FailingRunner {
        fn run(&self, _build: &VmBuild) -> Result<(), VmBuildError> {
            Err(self.0())
        }
    }

    /// Creates a directory that passes `check_rootfs`.
    fn fake_rootfs(dir: &Path) -> PathBuf {
        let rootfs = dir.join("rootfs");
        let bin = rootfs.join("usr/local/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("vm-build"), "#!/bin/sh\n").unwrap();
        rootfs
    }

    // --- VmConfig ---

    #[test]
    fn vm_config_new_uses_defaults() {
        let config = VmConfig::new(Path::new("/tmp/rootfs"));
        assert_eq!(config.rootfs, PathBuf::from("/tmp/rootfs"));
        assert_eq!(config.cpus, DEFAULT_CPUS);
        assert_eq!(config.memory_mib, DEFAULT_MEMORY_MIB);
    }

    // --- VmConfig::check_rootfs ---

    #[test]
    fn check_rootfs_accepts_a_rootfs_with_the_helper() {
        let tmp = tempdir();
        let config = VmConfig::new(&fake_rootfs(tmp.path()));
        assert!(config.check_rootfs().is_ok());
    }

    #[test]
    fn check_rootfs_rejects_a_missing_directory() {
        let config = VmConfig::new(Path::new("/a/really/improbable/rootfs"));
        let err = config.check_rootfs().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not a directory"), "unexpected: {msg}");
        assert!(msg.contains("make-rootfs.sh"), "unexpected: {msg}");
    }

    #[test]
    fn check_rootfs_rejects_a_directory_without_the_helper() {
        let tmp = tempdir();
        let config = VmConfig::new(tmp.path());
        let err = config.check_rootfs().unwrap_err();
        assert!(err.to_string().contains("vm-build"), "unexpected: {err}");
    }

    // --- build ---

    #[test]
    fn build_writes_the_containerfile_into_the_context() {
        let tmp = tempdir();
        let context = tmp.path().join("context");
        std::fs::create_dir_all(&context).unwrap();

        let runner = CaptureRunner::new();
        build(
            CONTAINERFILE,
            TAG,
            &VmConfig::new(&fake_rootfs(tmp.path())),
            &runner,
            &context,
            &tmp.path().join("out.tar"),
        )
        .unwrap();

        let written = std::fs::read_to_string(context.join("Containerfile")).unwrap();
        assert_eq!(written, CONTAINERFILE);
    }

    #[test]
    fn build_resolves_paths_and_splits_the_output() {
        let tmp = tempdir();
        let context = tmp.path().join("context");
        std::fs::create_dir_all(&context).unwrap();
        let rootfs = fake_rootfs(tmp.path());

        let runner = CaptureRunner::new();
        build(
            CONTAINERFILE,
            TAG,
            &VmConfig::new(&rootfs),
            &runner,
            &context,
            &tmp.path().join("nested").join("out.tar"),
        )
        .unwrap();

        let captured = runner.captured();
        assert_eq!(captured.tag, TAG);
        assert_eq!(captured.output_filename, "out.tar");
        assert_eq!(captured.rootfs, rootfs.canonicalize().unwrap());
        assert_eq!(captured.context, context.canonicalize().unwrap());
        assert_eq!(
            captured.output_dir,
            tmp.path().join("nested").canonicalize().unwrap()
        );
        assert_eq!(captured.cpus, DEFAULT_CPUS);
        assert_eq!(captured.memory_mib, DEFAULT_MEMORY_MIB);
    }

    #[test]
    fn build_creates_the_output_directory() {
        let tmp = tempdir();
        let nested = tmp.path().join("a").join("b");

        build(
            CONTAINERFILE,
            TAG,
            &VmConfig::new(&fake_rootfs(tmp.path())),
            &CaptureRunner::new(),
            tmp.path(),
            &nested.join("out.tar"),
        )
        .unwrap();

        assert!(nested.is_dir(), "output directory was not created");
    }

    #[test]
    fn build_passes_through_cpus_and_memory() {
        let tmp = tempdir();
        let config = VmConfig {
            rootfs: fake_rootfs(tmp.path()),
            cpus: 8,
            memory_mib: 16384,
        };

        let runner = CaptureRunner::new();
        build(
            CONTAINERFILE,
            TAG,
            &config,
            &runner,
            tmp.path(),
            &tmp.path().join("out.tar"),
        )
        .unwrap();

        let captured = runner.captured();
        assert_eq!(captured.cpus, 8);
        assert_eq!(captured.memory_mib, 16384);
    }

    #[test]
    fn build_reports_which_path_failed_to_resolve() {
        let tmp = tempdir();
        let err = build(
            CONTAINERFILE,
            TAG,
            &VmConfig::new(Path::new("/a/really/improbable/rootfs")),
            &CaptureRunner::new(),
            tmp.path(),
            &tmp.path().join("out.tar"),
        )
        .unwrap_err();

        assert!(matches!(err, VmBuildError::Path { .. }));
        assert!(err.to_string().contains("VM rootfs"), "unexpected: {err}");
    }

    #[test]
    fn build_propagates_runner_errors() {
        let tmp = tempdir();
        let runner = FailingRunner(|| VmBuildError::Failed { exit_code: Some(2) });
        let err = build(
            CONTAINERFILE,
            TAG,
            &VmConfig::new(&fake_rootfs(tmp.path())),
            &runner,
            tmp.path(),
            &tmp.path().join("out.tar"),
        )
        .unwrap_err();

        assert!(matches!(err, VmBuildError::Failed { exit_code: Some(2) }));
    }

    // --- VmBuild ---

    #[test]
    fn containerfile_vm_path_is_inside_the_context_mount() {
        let tmp = tempdir();
        let runner = CaptureRunner::new();
        build(
            CONTAINERFILE,
            TAG,
            &VmConfig::new(&fake_rootfs(tmp.path())),
            &runner,
            tmp.path(),
            &tmp.path().join("out.tar"),
        )
        .unwrap();

        assert_eq!(
            runner.captured().containerfile_vm_path(),
            "/build/context/Containerfile"
        );
    }

    #[test]
    fn output_path_joins_dir_and_filename() {
        let build = VmBuild {
            rootfs: PathBuf::from("/rootfs"),
            context: PathBuf::from("/ctx"),
            output_dir: PathBuf::from("/out"),
            output_filename: "image.tar".to_string(),
            tag: TAG.to_string(),
            cpus: 2,
            memory_mib: 4096,
        };
        assert_eq!(build.output_path(), PathBuf::from("/out/image.tar"));
    }

    // --- KrunRunner ---

    #[test]
    #[cfg(not(all(feature = "krun", target_os = "macos", target_arch = "aarch64")))]
    fn krun_runner_reports_unsupported_without_the_feature() {
        let build = VmBuild {
            rootfs: PathBuf::from("/rootfs"),
            context: PathBuf::from("/ctx"),
            output_dir: PathBuf::from("/out"),
            output_filename: "out.tar".to_string(),
            tag: TAG.to_string(),
            cpus: 2,
            memory_mib: 4096,
        };
        let err = KrunRunner.run(&build).unwrap_err();
        assert!(matches!(err, VmBuildError::Unsupported(_)));
        assert!(
            err.to_string().contains("not available"),
            "unexpected: {err}"
        );
    }

    // --- VmBuildError display ---

    #[test]
    fn io_error_display() {
        let err = VmBuildError::Io(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        assert_eq!(err.to_string(), "I/O error: file not found");
    }

    #[test]
    fn krun_error_display_names_the_call() {
        let err = VmBuildError::Krun {
            call: "krun_create_ctx",
            code: -22,
        };
        let msg = err.to_string();
        assert!(msg.contains("krun_create_ctx"), "unexpected: {msg}");
        assert!(msg.contains("-22"), "unexpected: {msg}");
    }

    #[test]
    fn failed_with_exit_code_display() {
        let err = VmBuildError::Failed { exit_code: Some(1) };
        assert_eq!(err.to_string(), "VM build failed with exit code 1");
    }

    #[test]
    fn failed_without_exit_code_display() {
        let err = VmBuildError::Failed { exit_code: None };
        assert_eq!(err.to_string(), "VM build was terminated by a signal");
    }

    #[test]
    fn from_io_error_wraps_correctly() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        assert!(matches!(VmBuildError::from(io_err), VmBuildError::Io(_)));
    }

    // --- helpers ---

    /// Minimal stand-in for `tempfile::TempDir` so the crate needs no dev
    /// dependency for a handful of tests.
    struct TempDir(PathBuf);

    impl TempDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn tempdir() -> TempDir {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("vm-image-builder-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}
