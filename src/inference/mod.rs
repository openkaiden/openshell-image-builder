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

mod anthropic;
mod ollama;
mod openai;
mod vertexai;

pub(crate) use ollama::DEFAULT_BASE_URL as OLLAMA_DEFAULT_BASE_URL;

#[cfg(test)]
pub use anthropic::AnthropicInference;
#[cfg(test)]
pub use ollama::OllamaInference;
#[cfg(test)]
pub use openai::OpenAiInference;
#[cfg(test)]
pub use vertexai::VertexAiInference;

use clap::ValueEnum;

pub trait Inference {
    /// Returns a network policy YAML fragment scoped to the given agent binary.
    /// `base_url` replaces the provider's default endpoint in the policy when `Some`.
    fn policy_yaml(&self, agent_binary: &str, base_url: Option<&str>) -> String;
}

/// Parses `host` and `port` from a URL string using the `url` crate.
/// Returns `None` if the URL cannot be parsed or has no host.
fn parse_host_port(url: &str) -> Option<(String, u16)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_string();
    let port = parsed
        .port()
        .unwrap_or_else(|| if parsed.scheme() == "https" { 443 } else { 80 });
    Some((host, port))
}

#[derive(Clone, PartialEq, ValueEnum)]
pub enum InferenceKind {
    Anthropic,
    Ollama,
    #[value(name = "openai")]
    OpenAi,
    #[value(name = "vertexai")]
    VertexAi,
}

pub fn from_kind(kind: InferenceKind) -> Box<dyn Inference> {
    match kind {
        InferenceKind::Anthropic => Box::new(anthropic::AnthropicInference),
        InferenceKind::Ollama => Box::new(ollama::OllamaInference),
        InferenceKind::OpenAi => Box::new(openai::OpenAiInference),
        InferenceKind::VertexAi => Box::new(vertexai::VertexAiInference),
    }
}
