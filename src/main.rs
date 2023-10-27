use clap::{Parser, Subcommand};

mod cli;

#[derive(Parser)]
#[command(author, version, about)]
struct ArgumentParser {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install an asset from a GitHub repository
    Install {
        /// The repository to install from (ex. DISTREAT/projavu)
        repository: String,
    },
}

fn main() {
    let arguments = ArgumentParser::parse();

    match &arguments.command {
        Commands::Install { repository } => cli::install::install_package(repository),
    }
}
