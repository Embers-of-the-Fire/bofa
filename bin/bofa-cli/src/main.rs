use bofa_lib::config::load_config;
use bofa_lib::git::AccountType;
use bofa_lib::git::context::GitContext;
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
        Commands::Config => match load_config(&config_path) {
            Ok(config) => println!("{config:#?}"),
            Err(err) => {
                eprintln!("Error loading config from {}: {err}", config_path.display());
                std::process::exit(1);
            }
        },
        Commands::Login => {
            let config = load_config(&config_path).unwrap_or_else(|err| {
                eprintln!("Error loading config from {}: {err}", config_path.display());
                std::process::exit(1);
            });
            let context = GitContext::from_credentials(&config.credentials, config.provider)
                .await
                .unwrap_or_else(|err| {
                    eprintln!("Authentication failed: {err}");
                    std::process::exit(1);
                });
            let metadata = context.account_metadata().await.unwrap_or_else(|err| {
                eprintln!("Failed to fetch account metadata: {err}");
                std::process::exit(1);
            });
            match metadata.account_type {
                AccountType::GitHubApp => {
                    let installation = metadata
                        .installation
                        .as_ref()
                        .expect("installation metadata missing for GitHub App");
                    println!(
                        "Logged in as {} (GitHub App) installed on {} ({})",
                        metadata.login, installation.login, installation.account_type
                    );
                }
                _ => {
                    println!(
                        "Logged in as {} ({}), id: {}",
                        metadata.login, metadata.account_type, metadata.id
                    );
                }
            }
        }
    }
}
