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

    match &cli.command {
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
        }
        | Commands::Triage {
            command: TriageCommands::Pr { id },
        } => {
            let is_check = matches!(&cli.command, Commands::Check { .. });
            let bofa = authenticate(bofa).await;
            let result = if is_check {
                bofa.check_pr(*id)
                    .await
                    .map(PrResult::from)
                    .unwrap_or_else(|err| {
                        eprintln!("Check failed: {err}");
                        std::process::exit(1);
                    })
            } else {
                bofa.triage_pr(*id)
                    .await
                    .map(PrResult::from)
                    .unwrap_or_else(|err| {
                        eprintln!("Triage failed: {err}");
                        std::process::exit(1);
                    })
            };
            print_pr_result(result, is_check);
        }
    }
}

fn print_pr_result(result: PrResult, is_check: bool) {
    use self::LocalCommentStatus as CommentStatus;
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
            } else if is_check {
                println!("Comment: none (nothing to report)");
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

#[derive(Debug)]
enum LocalCommentStatus {
    Created,
    Updated,
    Unchanged,
    Skipped,
}

#[derive(Debug)]
struct PrResult {
    body: Option<String>,
    status: LocalCommentStatus,
    comment_url: Option<String>,
    labels_applied: Vec<String>,
    labels_missing: Vec<String>,
}

impl From<bofa_lib::action::check::pr::CheckPrOutput> for PrResult {
    fn from(output: bofa_lib::action::check::pr::CheckPrOutput) -> Self {
        Self {
            body: output.body,
            status: output.status.into(),
            comment_url: output.comment_url,
            labels_applied: output.labels_applied,
            labels_missing: output.labels_missing,
        }
    }
}

impl From<bofa_lib::action::triage::pr::TriagePrOutput> for PrResult {
    fn from(output: bofa_lib::action::triage::pr::TriagePrOutput) -> Self {
        Self {
            body: output.body,
            status: output.status.into(),
            comment_url: output.comment_url,
            labels_applied: output.labels_applied,
            labels_missing: output.labels_missing,
        }
    }
}

impl From<bofa_lib::action::check::pr::CommentStatus> for LocalCommentStatus {
    fn from(status: bofa_lib::action::check::pr::CommentStatus) -> Self {
        match status {
            bofa_lib::action::check::pr::CommentStatus::Created => LocalCommentStatus::Created,
            bofa_lib::action::check::pr::CommentStatus::Updated => LocalCommentStatus::Updated,
            bofa_lib::action::check::pr::CommentStatus::Unchanged => LocalCommentStatus::Unchanged,
            bofa_lib::action::check::pr::CommentStatus::Skipped => LocalCommentStatus::Skipped,
        }
    }
}

impl From<bofa_lib::action::triage::pr::CommentStatus> for LocalCommentStatus {
    fn from(status: bofa_lib::action::triage::pr::CommentStatus) -> Self {
        match status {
            bofa_lib::action::triage::pr::CommentStatus::Created => LocalCommentStatus::Created,
            bofa_lib::action::triage::pr::CommentStatus::Updated => LocalCommentStatus::Updated,
            bofa_lib::action::triage::pr::CommentStatus::Unchanged => LocalCommentStatus::Unchanged,
            bofa_lib::action::triage::pr::CommentStatus::Skipped => LocalCommentStatus::Skipped,
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
