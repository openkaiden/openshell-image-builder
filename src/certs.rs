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

use std::io;
use std::path::Path;

/// Ordered list of common CA bundle paths across Linux distributions.
pub const SYSTEM_CA_CERT_PATHS: &[&str] = &[
    "/etc/ssl/certs/ca-certificates.crt",
    "/etc/pki/tls/certs/ca-bundle.crt",
    "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem",
    "/etc/ssl/ca-bundle.pem",
    "/etc/ssl/certs/ca-bundle.crt",
    "/etc/ssl/cert.pem",
    "/etc/ca-certificates/extracted/tls-ca-bundle.pem",
];

/// Tries each path in order; returns the content of the first non-empty regular file found.
pub fn find_system_ca_certificates(cert_paths: &[&str]) -> Option<Vec<u8>> {
    for path in cert_paths {
        let p = Path::new(path);
        if !p.is_file() {
            continue;
        }
        if let Ok(content) = std::fs::read(p)
            && !content.is_empty()
        {
            return Some(content);
        }
    }
    None
}

fn write_cert_to_context(context_dir: &Path, content: &[u8]) -> io::Result<()> {
    let certs_dir = context_dir.join("certs");
    std::fs::create_dir_all(&certs_dir)?;
    std::fs::write(certs_dir.join("system-ca.crt"), content)
}

/// Auto-discover mode: copies the first found bundle to `<context_dir>/certs/system-ca.crt`.
/// Returns `true` if a cert was found and copied, `false` if none found.
pub fn copy_from_paths(context_dir: &Path, cert_paths: &[&str]) -> io::Result<bool> {
    match find_system_ca_certificates(cert_paths) {
        None => Ok(false),
        Some(content) => {
            write_cert_to_context(context_dir, &content)?;
            Ok(true)
        }
    }
}

/// Specific-file mode: reads from `path` and copies to `<context_dir>/certs/system-ca.crt`.
/// Returns an error if the file doesn't exist or can't be read.
pub fn copy_from_file(context_dir: &Path, path: &Path) -> io::Result<()> {
    let content = std::fs::read(path)?;
    write_cert_to_context(context_dir, &content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_returns_none_for_empty_paths() {
        assert!(find_system_ca_certificates(&[]).is_none());
    }

    #[test]
    fn find_reads_first_existing_path() {
        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("bundle.crt");
        std::fs::write(&cert_path, b"CERT_DATA").unwrap();
        let path_str = cert_path.to_string_lossy().into_owned();
        let result = find_system_ca_certificates(&[path_str.as_str()]);
        assert_eq!(result, Some(b"CERT_DATA".to_vec()));
    }

    #[test]
    fn find_skips_directories() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();
        let cert = dir.path().join("bundle.crt");
        std::fs::write(&cert, b"REAL_CERT").unwrap();
        let dir_str = subdir.to_string_lossy().into_owned();
        let cert_str = cert.to_string_lossy().into_owned();
        let result = find_system_ca_certificates(&[dir_str.as_str(), cert_str.as_str()]);
        assert_eq!(result, Some(b"REAL_CERT".to_vec()));
    }

    #[test]
    fn find_skips_empty_files() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty.crt");
        let real = dir.path().join("real.crt");
        std::fs::write(&empty, b"").unwrap();
        std::fs::write(&real, b"REAL_CERT").unwrap();
        let empty_str = empty.to_string_lossy().into_owned();
        let real_str = real.to_string_lossy().into_owned();
        let result = find_system_ca_certificates(&[empty_str.as_str(), real_str.as_str()]);
        assert_eq!(result, Some(b"REAL_CERT".to_vec()));
    }

    #[test]
    fn find_falls_through_to_second_path_when_first_missing() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.crt");
        std::fs::write(&real, b"SECOND_CERT").unwrap();
        let real_str = real.to_string_lossy().into_owned();
        let result = find_system_ca_certificates(&["/nonexistent/path.crt", real_str.as_str()]);
        assert_eq!(result, Some(b"SECOND_CERT".to_vec()));
    }

    #[test]
    fn copy_from_paths_returns_false_when_no_certs_found() {
        let ctx = tempfile::tempdir().unwrap();
        let copied = copy_from_paths(ctx.path(), &[]).unwrap();
        assert!(!copied);
        assert!(!ctx.path().join("certs").exists());
    }

    #[test]
    fn copy_from_paths_creates_certs_dir_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let cert = dir.path().join("bundle.crt");
        std::fs::write(&cert, b"MY_CERT").unwrap();
        let cert_str = cert.to_string_lossy().into_owned();

        let ctx = tempfile::tempdir().unwrap();
        copy_from_paths(ctx.path(), &[cert_str.as_str()]).unwrap();

        assert!(ctx.path().join("certs").join("system-ca.crt").exists());
    }

    #[test]
    fn copy_from_paths_returns_true_when_cert_found() {
        let dir = tempfile::tempdir().unwrap();
        let cert = dir.path().join("bundle.crt");
        std::fs::write(&cert, b"MY_CERT").unwrap();
        let cert_str = cert.to_string_lossy().into_owned();

        let ctx = tempfile::tempdir().unwrap();
        let copied = copy_from_paths(ctx.path(), &[cert_str.as_str()]).unwrap();
        assert!(copied);
    }

    #[test]
    fn copy_from_file_writes_content_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let cert = dir.path().join("bundle.crt");
        std::fs::write(&cert, b"CUSTOM_CERT").unwrap();

        let ctx = tempfile::tempdir().unwrap();
        copy_from_file(ctx.path(), &cert).unwrap();

        let written = std::fs::read(ctx.path().join("certs").join("system-ca.crt")).unwrap();
        assert_eq!(written, b"CUSTOM_CERT");
    }

    #[test]
    fn copy_from_file_errors_when_file_missing() {
        let ctx = tempfile::tempdir().unwrap();
        let result = copy_from_file(ctx.path(), Path::new("/nonexistent/bundle.crt"));
        assert!(result.is_err());
    }
}
