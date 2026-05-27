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

mod agent;
mod build;
mod config;
mod containerfile;
mod feature;
mod workspace;

use std::path::PathBuf;

use clap::Parser;
use log::LevelFilter;

#[derive(Parser)]
#[command(
    name = "openshell-image-builder",
    version,
    about = "OpenShell image builder"
)]
struct Cli {
    #[arg(help = "Tag for the built image (e.g. myimage:latest)")]
    tag: String,
    #[arg(
        long,
        env = "OPENSHELL_IMAGE_BUILDER_CONFIG",
        help = "Path to config file"
    )]
    config: Option<PathBuf>,
    #[arg(
        short = 'v',
        action = clap::ArgAction::Count,
        help = "Increase log verbosity (-v info, -vv debug)"
    )]
    verbose: u8,
    #[arg(long, value_enum, help = "Agent to install in the image")]
    agent: Option<agent::AgentKind>,
}

fn main() {
    let cli = Cli::parse();
    // TODO: when JSON output is added, logs written to stderr may interfere with
    // structured output — revisit whether logs should be suppressed or embedded in the JSON.
    let log_level = match cli.verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    };
    env_logger::Builder::new().filter_level(log_level).init();
    let config = config::load(cli.config).unwrap_or_else(|e| {
        eprintln!("Error reading config file: {e}");
        std::process::exit(1);
    });
    let workspace = workspace::load().unwrap_or_else(|e| {
        eprintln!("Error reading workspace file: {e}");
        std::process::exit(1);
    });
    let agent = cli.agent.map(agent::from_kind);
    let context_dir = tempfile::Builder::new()
        .prefix("openshell-image-builder")
        .tempdir()
        .unwrap_or_else(|e| {
            eprintln!("Error creating build context directory: {e}");
            std::process::exit(1);
        });
    let features = feature::stage_all(workspace.as_ref(), context_dir.path()).unwrap_or_else(|e| {
        eprintln!("Error staging features: {e}");
        std::process::exit(1);
    });
    let output =
        containerfile::generate(&config, agent.as_deref(), &features).unwrap_or_else(|e| {
            eprintln!("Error generating Containerfile: {e}");
            std::process::exit(1);
        });
    build::build(&output, &cli.tag, &build::PodmanRunner, context_dir.path()).unwrap_or_else(|e| {
        eprintln!("Error building image: {e}");
        std::process::exit(1);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn version_matches_cargo_toml() {
        let cmd = Cli::command();
        assert_eq!(cmd.get_version(), Some(env!("CARGO_PKG_VERSION")));
    }
}
