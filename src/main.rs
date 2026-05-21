use clap::Parser;

#[derive(Parser)]
#[command(
    name = "openshell-image-builder",
    version,
    about = "OpenShell image builder"
)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
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
