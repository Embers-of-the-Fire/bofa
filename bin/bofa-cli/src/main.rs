use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bofa", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Hello,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Hello => {
            println!("Hello, world!");
        }
    }
}
