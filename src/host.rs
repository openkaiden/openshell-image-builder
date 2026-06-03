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

use url::Url;

/// Rewrites `localhost` in the URL host component to `host.openshell.internal`,
/// the hostname used to reach the container host from within an OpenShell sandbox.
/// URLs with any other host are returned unchanged.
pub(crate) fn rewrite_localhost(base_url: &str) -> String {
    let Ok(mut url) = Url::parse(base_url) else {
        return base_url.to_string();
    };
    if url.host_str() == Some("localhost") {
        let _ = url.set_host(Some("host.openshell.internal"));
    }
    url.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_localhost_replaces_host() {
        assert_eq!(
            rewrite_localhost("http://localhost:11434/v1"),
            "http://host.openshell.internal:11434/v1"
        );
    }

    #[test]
    fn rewrite_localhost_preserves_port_and_path() {
        assert_eq!(
            rewrite_localhost("http://localhost:9999/some/path"),
            "http://host.openshell.internal:9999/some/path"
        );
    }

    #[test]
    fn rewrite_localhost_leaves_other_hosts_unchanged() {
        assert_eq!(
            rewrite_localhost("http://notlocalhost:11434/v1"),
            "http://notlocalhost:11434/v1"
        );
    }

    #[test]
    fn rewrite_localhost_leaves_custom_host_unchanged() {
        assert_eq!(
            rewrite_localhost("http://custom-host:11434/v1"),
            "http://custom-host:11434/v1"
        );
    }

    #[test]
    fn rewrite_localhost_leaves_host_openshell_internal_unchanged() {
        assert_eq!(
            rewrite_localhost("http://host.openshell.internal:11434/v1"),
            "http://host.openshell.internal:11434/v1"
        );
    }
}
