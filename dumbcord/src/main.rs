mod client;
mod cli;
mod desktop;
mod protocol;
mod server;
mod store;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dumbcord", version, about = "Native Rust chat for friends")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Host a self-contained chat server and SQLite database.
    Server {
        #[arg(long, default_value = "127.0.0.1:7777")]
        bind: String,
        #[arg(long, default_value = "dumbcord.sqlite3")]
        db: String,
    },
    /// Run the terminal chat client.
    Cli {
        #[arg(long, default_value = "127.0.0.1:7777")]
        server: String,
        #[arg(long)]
        user: Option<String>,
    },
    /// Open the native desktop client.
    Desktop {
        #[arg(long, default_value = "127.0.0.1:7777")]
        server: String,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Server { bind, db } => server::run(&bind, &db),
        Command::Cli { server, user } => cli::run(&server, user),
        Command::Desktop { server } => desktop::run(server),
    }
}
