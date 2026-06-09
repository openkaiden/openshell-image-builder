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

// Serde types that mirror the OpenShell sandbox policy YAML schema.
// Derived from the YAML layer of the openshell-policy crate (Apache-2.0):
//   https://github.com/NVIDIA/OpenShell/blob/c9056bbc5550a58d70cd652ac53bccf6a0f48a0b/crates/openshell-policy/src/lib.rs
// Proto/tonic conversions are omitted; only the pure serde round-trip is kept.
// Rationale: https://github.com/NVIDIA/OpenShell/issues/1608

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Top-level policy document
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxPolicy {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filesystem_policy: Option<FilesystemPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landlock: Option<LandlockPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<ProcessPolicy>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub network_policies: BTreeMap<String, NetworkPolicyRule>,
}

// ---------------------------------------------------------------------------
// Sandbox sections
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilesystemPolicy {
    #[serde(default)]
    pub include_workdir: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_only: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_write: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandlockPolicy {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub compatibility: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessPolicy {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub run_as_user: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub run_as_group: String,
}

// ---------------------------------------------------------------------------
// Network policy
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkPolicyRule {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub endpoints: Vec<NetworkEndpoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binaries: Vec<NetworkBinary>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkEndpoint {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub host: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    /// Single port (backwards compat). Mutually exclusive with `ports`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub port: u16,
    /// Multiple ports. When non-empty, covers all listed ports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<u16>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub protocol: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tls: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub enforcement: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub access: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<L7Rule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_ips: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny_rules: Vec<L7DenyRule>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_encoded_slash: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub websocket_credential_rewrite: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub request_body_credential_rewrite: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub persisted_queries: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub graphql_persisted_queries: BTreeMap<String, GraphqlOperation>,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub graphql_max_body_bytes: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkBinary {
    pub path: String,
    /// Deprecated: ignored. Kept for backward compat with existing YAML files.
    #[serde(default, skip_serializing)]
    #[allow(dead_code)]
    harness: bool,
}

impl NetworkBinary {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            harness: false,
        }
    }
}

// ---------------------------------------------------------------------------
// L7 rules
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct L7Rule {
    pub allow: L7Allow,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct L7Allow {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub method: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query: BTreeMap<String, QueryMatcher>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub operation_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub operation_name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct L7DenyRule {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub method: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query: BTreeMap<String, QueryMatcher>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub operation_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub operation_name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryMatcher {
    Glob(String),
    Any(QueryAny),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryAny {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub any: Vec<String>,
}

// ---------------------------------------------------------------------------
// GraphQL
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphqlOperation {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub operation_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub operation_name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// Signature dictated by serde's `skip_serializing_if`, which requires `&T`.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(v: &u16) -> bool {
    *v == 0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse a sandbox policy from a YAML string.
pub fn parse_sandbox_policy(yaml: &str) -> Result<SandboxPolicy, serde_yml::Error> {
    serde_yml::from_str(yaml)
}

/// Serialize a sandbox policy to a YAML string.
pub fn serialize_sandbox_policy(policy: &SandboxPolicy) -> Result<String, serde_yml::Error> {
    serde_yml::to_string(policy)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_policy() {
        let policy = parse_sandbox_policy("version: 1\n").expect("should parse");
        assert_eq!(policy.version, 1);
        assert!(policy.network_policies.is_empty());
        assert!(policy.filesystem_policy.is_none());
    }

    #[test]
    fn parse_rejects_unknown_fields() {
        assert!(parse_sandbox_policy("version: 1\nbogus_field: true\n").is_err());
    }

    #[test]
    fn round_trip_base_policy() {
        let yaml = include_str!("../../assets/policy.yaml");
        let policy = parse_sandbox_policy(yaml).expect("should parse base policy");
        assert_eq!(policy.version, 1);
        assert!(policy.filesystem_policy.is_some());
        assert!(!policy.network_policies.is_empty());
        let out = serialize_sandbox_policy(&policy).expect("should serialize");
        let reparsed = parse_sandbox_policy(&out).expect("round-trip should parse");
        assert_eq!(reparsed.version, policy.version);
        assert_eq!(
            reparsed.network_policies.len(),
            policy.network_policies.len()
        );
    }

    #[test]
    fn network_policies_are_merged_by_extend() {
        let base_yaml = "version: 1\nnetwork_policies:\n  a:\n    name: a\n";
        let extra_yaml = "version: 1\nnetwork_policies:\n  b:\n    name: b\n";
        let mut base = parse_sandbox_policy(base_yaml).expect("parse");
        let extra = parse_sandbox_policy(extra_yaml).expect("parse");
        base.network_policies.extend(extra.network_policies);
        assert_eq!(base.network_policies.len(), 2);
        assert!(base.network_policies.contains_key("a"));
        assert!(base.network_policies.contains_key("b"));
    }

    #[test]
    fn serialized_yaml_uses_filesystem_policy_key() {
        let yaml = include_str!("../../assets/policy.yaml");
        let policy = parse_sandbox_policy(yaml).expect("parse");
        let out = serialize_sandbox_policy(&policy).expect("serialize");
        assert!(out.contains("filesystem_policy:"));
        assert!(!out.contains("\nfilesystem:"));
    }

    #[test]
    fn parse_port_above_65535_fails() {
        let yaml = "version: 1\nnetwork_policies:\n  t:\n    endpoints:\n      - host: x.com\n        port: 70000\n";
        assert!(parse_sandbox_policy(yaml).is_err());
    }

    #[test]
    fn round_trip_preserves_l7_rules() {
        let yaml = "version: 1\nnetwork_policies:\n  github:\n    name: github\n    endpoints:\n      - host: github.com\n        port: 443\n        protocol: rest\n        tls: terminate\n        enforcement: enforce\n        rules:\n          - allow:\n              method: GET\n              path: \"/**/info/refs*\"\n          - allow:\n              method: POST\n              path: \"/**/git-upload-pack\"\n    binaries:\n      - path: /usr/bin/git\n";
        let p1 = parse_sandbox_policy(yaml).expect("parse");
        let out = serialize_sandbox_policy(&p1).expect("serialize");
        let p2 = parse_sandbox_policy(&out).expect("reparse");
        let ep = &p2.network_policies["github"].endpoints[0];
        assert_eq!(ep.rules.len(), 2);
        assert_eq!(ep.rules[0].allow.method, "GET");
        assert_eq!(ep.rules[1].allow.path, "/**/git-upload-pack");
    }
}
