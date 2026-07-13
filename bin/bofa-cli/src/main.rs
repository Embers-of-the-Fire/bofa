use bofa_lib::action::Bofa;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "bofa", version, about, long_about = None)]
struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,
    #[arg(long, global = true)]
    dry_run: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Config,
    Login,
    Check {
        #[command(subcommand)]
        command: CheckCommands,
    },
}

#[derive(Subcommand)]
enum CheckCommands {
    Pr { id: u64 },
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();
    let config_path = cli.config.unwrap_or_else(|| PathBuf::from("bofa.toml"));
    let bofa = load_config(&config_path).with_dry_run(cli.dry_run);

    bofa_lib::logging::init(&bofa.config().log, true);

    match cli.command {
        Commands::Config => {
            println!("{:#?}", bofa.config());
        }
        Commands::Login => {
            let bofa = authenticate(bofa).await;
            let message = bofa.login().await.unwrap_or_else(|err| {
                eprintln!("Login failed: {err}");
                std::process::exit(1);
            });
            println!("{message}");
        }
        Commands::Check {
            command: CheckCommands::Pr { id },
        } => {
            let bofa = authenticate(bofa).await;
            let result = bofa.check_pr(id).await.unwrap_or_else(|err| {
                eprintln!("Check failed: {err}");
                std::process::exit(1);
            });
            if result.posted {
                println!("{}", result.comment_url.unwrap());
            } else if let Some(body) = result.body {
                println!("{body}");
            }
        }
    }
}

fn load_config(config_path: &PathBuf) -> bofa_lib::action::Bofa {
    Bofa::load_config(config_path).unwrap_or_else(|err| {
        eprintln!("Error loading config from {}: {err}", config_path.display());
        std::process::exit(1);
    })
}

async fn authenticate(bofa: bofa_lib::action::Bofa) -> bofa_lib::action::AuthenticatedBofa {
    bofa.ensure_authenticated().await.unwrap_or_else(|err| {
        eprintln!("Authentication failed: {err}");
        std::process::exit(1);
    })
}
