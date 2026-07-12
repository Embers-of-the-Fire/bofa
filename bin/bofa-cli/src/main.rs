use bofa_lib::action::Bofa;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    Config,
    Login,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    let config_path = cli.config.unwrap_or_else(|| PathBuf::from("bofa.toml"));

    match cli.command {
        Commands::Config => {
            let bofa = Bofa::load_config(&config_path).unwrap_or_else(|err| {
                eprintln!("Error loading config from {}: {err}", config_path.display());
                std::process::exit(1);
            });
            println!("{:#?}", bofa.config());
        }
        Commands::Login => {
            let bofa = Bofa::load_config(&config_path).unwrap_or_else(|err| {
                eprintln!("Error loading config from {}: {err}", config_path.display());
                std::process::exit(1);
            });
            let bofa = bofa.ensure_authenticated().await.unwrap_or_else(|err| {
                eprintln!("Authentication failed: {err}");
                std::process::exit(1);
            });
            let message = bofa.login().await.unwrap_or_else(|err| {
                eprintln!("Login failed: {err}");
                std::process::exit(1);
            });
            println!("{message}");
        }
    }
}
