use std::{io::BufRead, path::PathBuf};

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use porch_server::{auth, serve, store::Store, validate, voice::RealtimeConfig, Config, Limits};
use tokio::net::TcpListener;

#[derive(Parser)]
#[command(name = "porch-server", version, about = "The Porch chat server")]
struct Cli {
    /// SQLite database file.
    #[arg(long, env = "PORCH_DB", default_value = "porch.sqlite3", global = true)]
    db: String,

    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    serve: ServeArgs,
}

#[derive(clap::Args)]
struct ServeArgs {
    /// Address to listen on. Put a TLS proxy such as Caddy in front of it.
    #[arg(long, env = "PORCH_BIND", default_value = "127.0.0.1:7878")]
    bind: String,

    /// Require this code to create an account.
    #[arg(long, env = "PORCH_INVITE", hide_env_values = true)]
    invite: Option<String>,

    /// Serve the built web client from this folder.
    #[arg(long, env = "PORCH_WEB_DIR")]
    web_dir: Option<PathBuf>,

    /// Cloudflare Realtime SFU app ID. Voice is off unless this and the token are set.
    #[arg(long, env = "PORCH_CF_APP_ID")]
    cf_app_id: Option<String>,

    /// Cloudflare Realtime SFU app token. Prefer the environment variable.
    #[arg(long, env = "PORCH_CF_APP_TOKEN", hide_env_values = true)]
    cf_app_token: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the server (the default).
    Serve,
    /// Set a user's password from one line on stdin and sign them out everywhere.
    Passwd { username: String },
    /// List accounts.
    Users,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "porch_server=info".into()),
        )
        .init();
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => {
            let ServeArgs {
                bind,
                invite,
                web_dir,
                cf_app_id,
                cf_app_token,
            } = cli.serve;
            let realtime = match (cf_app_id, cf_app_token) {
                (Some(app_id), Some(app_token)) => Some(RealtimeConfig {
                    base_url: RealtimeConfig::BASE_URL.into(),
                    app_id,
                    app_token,
                }),
                (None, None) => None,
                _ => bail!("set both PORCH_CF_APP_ID and PORCH_CF_APP_TOKEN for voice, or neither"),
            };
            if realtime.is_none() {
                tracing::info!("voice is off: no Cloudflare Realtime app configured");
            }
            if invite.is_none() {
                tracing::warn!("no invite code set: anyone who can reach this server can register");
            }
            let listener = TcpListener::bind(&bind)
                .await
                .with_context(|| format!("couldn't listen on {bind}"))?;
            tracing::info!(
                "porch-server {} listening on {bind}",
                env!("CARGO_PKG_VERSION")
            );
            let cfg = Config {
                db_path: cli.db,
                invite,
                web_dir,
                realtime,
                limits: Limits::default(),
            };
            serve(listener, cfg, async {
                let _ = tokio::signal::ctrl_c().await;
                tracing::info!("shutting down");
            })
            .await
        }
        Command::Passwd { username } => {
            let user = validate::username(&username).map_err(|e| anyhow::anyhow!("{e}"))?;
            let mut line = String::new();
            std::io::stdin().lock().read_line(&mut line)?;
            let password = line.trim_end_matches(['\r', '\n']);
            validate::password(password).map_err(|e| anyhow::anyhow!("{e}"))?;
            let store = Store::open(&cli.db)?;
            let hash = auth::hash_password(password)?;
            if !store
                .set_password(&user, &hash)
                .map_err(|e| anyhow::anyhow!("{e}"))?
            {
                bail!("no user called {user}");
            }
            println!("Password changed for {user}; their sessions were signed out.");
            Ok(())
        }
        Command::Users => {
            let store = Store::open(&cli.db)?;
            for user in store.users().map_err(|e| anyhow::anyhow!("{e}"))? {
                println!("{user}");
            }
            Ok(())
        }
    }
}
