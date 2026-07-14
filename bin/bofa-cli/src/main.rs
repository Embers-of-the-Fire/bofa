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
    Triage {
        #[command(subcommand)]
        command: TriageCommands,
    },
}

#[derive(Subcommand)]
enum CheckCommands {
    Pr { id: u64 },
}

#[derive(Subcommand)]
enum TriageCommands {
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
            use bofa_lib::action::check::pr::CommentStatus;
            let bofa = authenticate(bofa).await;
            let result = bofa.check_pr(id).await.unwrap_or_else(|err| {
                eprintln!("Check failed: {err}");
                std::process::exit(1);
            });
            match result.status {
                CommentStatus::Created => {
                    println!("Comment: created {}", result.comment_url.unwrap());
                }
                CommentStatus::Updated => {
                    println!("Comment: updated {}", result.comment_url.unwrap());
                }
                CommentStatus::Unchanged => {
                    println!("Comment: up to date {}", result.comment_url.unwrap());
                }
                CommentStatus::Skipped => {
                    if result.body.is_some() {
                        println!("Comment: not posted");
                    } else {
                        println!("Comment: none (nothing to report)");
                    }
                }
            }
            if !result.labels_applied.is_empty() {
                println!("Applied labels: {}", result.labels_applied.join(", "));
            }
            if !result.labels_missing.is_empty() {
                println!(
                    "Missing labels (skipped): {}",
                    result.labels_missing.join(", ")
                );
            }
            if matches!(result.status, CommentStatus::Skipped)
                && let Some(body) = result.body
            {
                println!();
                println!("--- Rendered comment ---");
                println!("{body}");
                println!("--- End of comment ---");
            }
        }
        Commands::Triage {
            command: TriageCommands::Pr { id },
        } => {
            use bofa_lib::action::triage::pr::CommentStatus;
            let bofa = authenticate(bofa).await;
            let result = bofa.triage_pr(id).await.unwrap_or_else(|err| {
                eprintln!("Triage failed: {err}");
                std::process::exit(1);
            });
            match result.status {
                CommentStatus::Created => {
                    println!("Comment: created {}", result.comment_url.unwrap());
                }
                CommentStatus::Updated => {
                    println!("Comment: updated {}", result.comment_url.unwrap());
                }
                CommentStatus::Unchanged => {
                    println!("Comment: up to date {}", result.comment_url.unwrap());
                }
                CommentStatus::Skipped => {
                    if result.body.is_some() {
                        println!("Comment: not posted");
                    } else {
                        println!("Comment: none (no triage groups matched)");
                    }
                }
            }
            if !result.labels_applied.is_empty() {
                println!("Applied labels: {}", result.labels_applied.join(", "));
            }
            if !result.labels_missing.is_empty() {
                println!(
                    "Missing labels (skipped): {}",
                    result.labels_missing.join(", ")
                );
            }
            if matches!(result.status, CommentStatus::Skipped)
                && let Some(body) = result.body
            {
                println!();
                println!("--- Rendered comment ---");
                println!("{body}");
                println!("--- End of comment ---");
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
