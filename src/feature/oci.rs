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

use std::io::{self, Read};
use std::path::Path;

use oci_spec::image::ImageManifest;
use sha2::{Digest, Sha256};

use super::FeatureError;
use super::metadata::{self, FeatureMetadata};

pub struct OciFeature {
    pub id: String,
}

impl OciFeature {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string() }
    }

    pub fn stage(&self, dest_dir: &Path) -> Result<FeatureMetadata, FeatureError> {
        let (registry, repository, reference) = parse_oci_ref(&self.id)?;
        let (manifest, token) = fetch_manifest(&registry, &repository, &reference)?;

        for layer in manifest.layers() {
            let digest = layer.digest().to_string();
            download_and_extract_layer(
                &registry,
                &repository,
                &digest,
                token.as_deref(),
                dest_dir,
            )?;
        }

        if !dest_dir.join("install.sh").exists() {
            return Err(FeatureError::Io(format!(
                "missing install.sh for OCI feature {:?}",
                self.id
            )));
        }
        metadata::parse(dest_dir)
    }
}

/// Parses an OCI reference into (registry, repository, reference).
/// Defaults to `ghcr.io` when no registry is specified.
/// Returns (registry, repository, tag-or-digest).
pub fn parse_oci_ref(id: &str) -> Result<(String, String, String), FeatureError> {
    let (id_no_digest, digest) = if let Some(pos) = id.find('@') {
        (&id[..pos], Some(id[pos + 1..].to_string()))
    } else {
        (id, None)
    };

    let (id_no_tag, tag) = {
        let first_slash = id_no_digest.find('/').unwrap_or(0);
        if let Some(pos) = id_no_digest.rfind(':') {
            if pos > first_slash {
                (
                    &id_no_digest[..pos],
                    Some(id_no_digest[pos + 1..].to_string()),
                )
            } else {
                (id_no_digest, None)
            }
        } else {
            (id_no_digest, None)
        }
    };

    let reference = digest.or(tag).unwrap_or_else(|| "latest".to_string());

    // Determine registry: first path component is a registry if it contains '.' or ':',
    // or equals "localhost".
    let parts: Vec<&str> = id_no_tag.splitn(2, '/').collect();
    let (registry, repository) = if parts.len() == 2
        && (parts[0].contains('.') || parts[0].contains(':') || parts[0] == "localhost")
    {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        ("ghcr.io".to_string(), id_no_tag.to_string())
    };

    Ok((registry, repository, reference))
}

fn fetch_manifest(
    registry: &str,
    repository: &str,
    reference: &str,
) -> Result<(ImageManifest, Option<String>), FeatureError> {
    let url = format!("https://{registry}/v2/{repository}/manifests/{reference}");

    let resp = ureq::get(&url)
        .set("Accept", "application/vnd.oci.image.manifest.v1+json")
        .call();

    match resp {
        Ok(r) => {
            let body = r
                .into_string()
                .map_err(|e| FeatureError::Oci(format!("reading manifest response: {e}")))?;
            let manifest = serde_json::from_str::<ImageManifest>(&body)
                .map_err(|e| FeatureError::Oci(format!("parsing manifest: {e}")))?;
            Ok((manifest, None))
        }
        Err(ureq::Error::Status(401, resp)) => {
            let www_auth = resp
                .header("WWW-Authenticate")
                .unwrap_or_default()
                .to_string();
            let token = fetch_bearer_token(&www_auth, repository)?;

            let body = ureq::get(&url)
                .set("Accept", "application/vnd.oci.image.manifest.v1+json")
                .set("Authorization", &format!("Bearer {token}"))
                .call()
                .map_err(|e| FeatureError::Oci(format!("fetching manifest with token: {e}")))?
                .into_string()
                .map_err(|e| FeatureError::Oci(format!("reading manifest response: {e}")))?;

            let manifest = serde_json::from_str::<ImageManifest>(&body)
                .map_err(|e| FeatureError::Oci(format!("parsing manifest: {e}")))?;
            Ok((manifest, Some(token)))
        }
        Err(e) => Err(FeatureError::Oci(format!("fetching manifest: {e}"))),
    }
}

fn fetch_bearer_token(www_auth: &str, repository: &str) -> Result<String, FeatureError> {
    let header = www_auth.trim_start_matches("Bearer ");
    let mut params = std::collections::HashMap::new();
    for part in header.split(',') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            params.insert(k.trim(), v.trim().trim_matches('"'));
        }
    }

    let realm = params
        .get("realm")
        .ok_or_else(|| FeatureError::Oci(format!("no realm in WWW-Authenticate: {www_auth:?}")))?;
    let service = params.get("service").copied().unwrap_or("");
    let scope = params.get("scope").copied().unwrap_or("").to_string();
    let scope = if scope.is_empty() {
        format!("repository:{repository}:pull")
    } else {
        scope
    };

    let mut url = format!("{realm}?scope={scope}");
    if !service.is_empty() {
        url.push_str(&format!("&service={service}"));
    }

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        #[serde(alias = "access_token")]
        token: Option<String>,
    }

    let body = ureq::get(&url)
        .call()
        .map_err(|e| FeatureError::Oci(format!("fetching bearer token: {e}")))?
        .into_string()
        .map_err(|e| FeatureError::Oci(format!("reading token response: {e}")))?;

    let resp: TokenResponse = serde_json::from_str(&body)
        .map_err(|e| FeatureError::Oci(format!("parsing token response: {e}")))?;

    resp.token
        .ok_or_else(|| FeatureError::Oci("no token in response".to_string()))
}

fn download_and_extract_layer(
    registry: &str,
    repository: &str,
    digest: &str,
    token: Option<&str>,
    dest_dir: &Path,
) -> Result<(), FeatureError> {
    if !digest.starts_with("sha256:") {
        return Err(FeatureError::Oci(format!(
            "unsupported digest algorithm: {digest}"
        )));
    }
    let expected_hex = &digest["sha256:".len()..];

    let url = format!("https://{registry}/v2/{repository}/blobs/{digest}");
    let mut req = ureq::get(&url);
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }

    let resp = req
        .call()
        .map_err(|e| FeatureError::Oci(format!("downloading blob {digest}: {e}")))?;

    // Buffer the blob while computing its SHA-256 — verify before extracting.
    let mut hasher = Sha256::new();
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| FeatureError::Oci(format!("reading blob {digest}: {e}")))?;
    hasher.update(&buf);
    let got = format!("{:x}", hasher.finalize());
    if got != expected_hex {
        return Err(FeatureError::Oci(format!(
            "blob digest mismatch: expected {expected_hex}, got {got}"
        )));
    }

    extract_tar(io::Cursor::new(buf), dest_dir)
}

fn extract_tar<R: Read>(mut r: R, dest_dir: &Path) -> Result<(), FeatureError> {
    let mut peek = [0u8; 2];
    let n = r
        .read(&mut peek)
        .map_err(|e| FeatureError::Oci(format!("reading blob header: {e}")))?;
    let combined = io::Read::chain(io::Cursor::new(peek[..n].to_vec()), r);

    if n >= 2 && peek[0] == 0x1f && peek[1] == 0x8b {
        let gz = flate2::read::GzDecoder::new(combined);
        extract_tar_entries(&mut tar::Archive::new(gz), dest_dir)
    } else {
        extract_tar_entries(&mut tar::Archive::new(combined), dest_dir)
    }
}

fn extract_tar_entries<R: Read>(
    archive: &mut tar::Archive<R>,
    dest_dir: &Path,
) -> Result<(), FeatureError> {
    for entry in archive
        .entries()
        .map_err(|e| FeatureError::Oci(format!("reading tar entries: {e}")))?
    {
        let mut entry = entry.map_err(|e| FeatureError::Oci(format!("reading tar entry: {e}")))?;
        let entry_type = entry.header().entry_type();

        if matches!(entry_type, tar::EntryType::Symlink | tar::EntryType::Link) {
            return Err(FeatureError::Oci(
                "symlinks and hard links are not supported in OCI feature layers".to_string(),
            ));
        }

        let path = entry
            .path()
            .map_err(|e| FeatureError::Oci(format!("reading tar entry path: {e}")))?;
        let target = safe_tar_target(dest_dir, &path)?;

        if entry_type.is_dir() {
            std::fs::create_dir_all(&target)
                .map_err(|e| FeatureError::Oci(format!("creating directory {target:?}: {e}")))?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    FeatureError::Oci(format!("creating parent directory {parent:?}: {e}"))
                })?;
            }
            entry
                .unpack(&target)
                .map_err(|e| FeatureError::Oci(format!("extracting {target:?}: {e}")))?;
            // Preserve executable bit from the tar header.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = entry.header().mode().unwrap_or(0o644);
                std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode)).map_err(
                    |e| FeatureError::Oci(format!("setting permissions on {target:?}: {e}")),
                )?;
            }
        }
    }
    Ok(())
}

/// Resolves `name` relative to `dest_dir`, rejecting paths that escape it.
/// Strips leading `./` components; rejects absolute paths and any path containing `..`.
fn safe_tar_target(dest_dir: &Path, name: &Path) -> Result<std::path::PathBuf, FeatureError> {
    let clean = name
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .collect::<std::path::PathBuf>();

    // Reject absolute paths or any path with a `..` component anywhere (not just at the start),
    // since `foo/../../../etc` would escape dest_dir after resolution.
    if clean.is_absolute()
        || clean
            .components()
            .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(FeatureError::Oci(format!(
            "invalid path in tar archive: {name:?}"
        )));
    }

    if clean.as_os_str().is_empty() {
        return Ok(dest_dir.to_path_buf());
    }

    Ok(dest_dir.join(&clean))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_oci_ref_with_registry_and_tag() {
        let (reg, repo, refer) = parse_oci_ref("ghcr.io/devcontainers/features/rust:1").unwrap();
        assert_eq!(reg, "ghcr.io");
        assert_eq!(repo, "devcontainers/features/rust");
        assert_eq!(refer, "1");
    }

    #[test]
    fn parse_oci_ref_defaults_registry_to_ghcr() {
        let (reg, repo, refer) = parse_oci_ref("devcontainers/features/rust:1").unwrap();
        assert_eq!(reg, "ghcr.io");
        assert_eq!(repo, "devcontainers/features/rust");
        assert_eq!(refer, "1");
    }

    #[test]
    fn parse_oci_ref_no_tag_defaults_to_latest() {
        let (_, _, refer) = parse_oci_ref("ghcr.io/devcontainers/features/rust").unwrap();
        assert_eq!(refer, "latest");
    }

    #[test]
    fn parse_oci_ref_with_digest() {
        let (reg, repo, refer) =
            parse_oci_ref("ghcr.io/devcontainers/features/rust@sha256:abc123").unwrap();
        assert_eq!(reg, "ghcr.io");
        assert_eq!(repo, "devcontainers/features/rust");
        assert_eq!(refer, "sha256:abc123");
    }

    #[test]
    fn parse_oci_ref_localhost() {
        let (reg, repo, refer) = parse_oci_ref("localhost/myfeature:v1").unwrap();
        assert_eq!(reg, "localhost");
        assert_eq!(repo, "myfeature");
        assert_eq!(refer, "v1");
    }

    #[test]
    fn safe_tar_target_normal_path() {
        let tmp = tempfile::tempdir().unwrap();
        let result = safe_tar_target(tmp.path(), Path::new("install.sh")).unwrap();
        assert_eq!(result, tmp.path().join("install.sh"));
    }

    #[test]
    #[cfg(not(windows))]
    fn safe_tar_target_rejects_absolute() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(matches!(
            safe_tar_target(tmp.path(), Path::new("/etc/passwd")),
            Err(FeatureError::Oci(_))
        ));
    }

    #[test]
    fn safe_tar_target_rejects_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(matches!(
            safe_tar_target(tmp.path(), Path::new("../escape")),
            Err(FeatureError::Oci(_))
        ));
    }

    #[test]
    fn safe_tar_target_dot_maps_to_dest_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = safe_tar_target(tmp.path(), Path::new(".")).unwrap();
        assert_eq!(result, tmp.path());
    }

    #[test]
    fn safe_tar_target_curdir_prefix_allowed() {
        let tmp = tempfile::tempdir().unwrap();
        let result = safe_tar_target(tmp.path(), Path::new("./NOTES.md")).unwrap();
        assert_eq!(result, tmp.path().join("NOTES.md"));
    }

    #[test]
    fn safe_tar_target_rejects_embedded_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(matches!(
            safe_tar_target(tmp.path(), Path::new("foo/../../etc")),
            Err(FeatureError::Oci(_))
        ));
    }
}
