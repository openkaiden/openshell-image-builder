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

use super::Agent;

pub struct OpencodeAgent;

impl Agent for OpencodeAgent {
    fn install(&self) -> String {
        // The installer places the binary in ~/.opencode/bin/ which is not in PATH,
        // so we symlink it into ~/.local/bin/.
        "RUN cd /tmp && curl -fsSL https://opencode.ai/install | bash && \\\n    \
             mkdir -p /sandbox/.local/bin && \\\n    \
             ln -sf /sandbox/.opencode/bin/opencode /sandbox/.local/bin/opencode && \\\n    \
             mkdir -p /sandbox/.config/opencode\n\
         ENV PATH=/sandbox/.local/bin:$PATH"
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_is_nonempty() {
        assert!(!OpencodeAgent.install().is_empty());
    }

    #[test]
    fn install_contains_opencode_installer() {
        assert!(
            OpencodeAgent
                .install()
                .contains("https://opencode.ai/install")
        );
    }

    #[test]
    fn install_symlinks_binary_to_local_bin() {
        assert!(
            OpencodeAgent
                .install()
                .contains("/sandbox/.local/bin/opencode")
        );
    }

    #[test]
    fn install_adds_local_bin_to_path() {
        assert!(
            OpencodeAgent
                .install()
                .contains("ENV PATH=/sandbox/.local/bin:$PATH")
        );
    }
}
