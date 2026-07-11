use std::path::PathBuf;

use bofa_lib::config::load_config;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bofa", version, about, long_about = None)]
struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Hello,
    Config,
}

fn main() {
    let cli = Cli::parse();

    let config_path = cli.config.unwrap_or_else(|| PathBuf::from("bofa.toml"));

    match cli.command {
        Commands::Hello => {
            println!("Hello, world!");
        }
        Commands::Config => match load_config(&config_path) {
            Ok(config) => println!("{config:#?}"),
            Err(err) => {
                eprintln!("Error loading config from {}: {err}", config_path.display());
                std::process::exit(1);
            }
        },
    }
}
