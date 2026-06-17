use crate::proto::TokenType;
use clap::{Parser, Subcommand};

mod agent;
#[cfg(test)]
mod integration_test;
mod proto;
mod relay;
mod web;

#[derive(Parser)]
#[command(name = "shell-remote", about = "Collaborative remote shell tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run in relay (server) mode
    Relay {
        /// Address to bind the relay server
        #[arg(long, default_value = "0.0.0.0:3000")]
        bind: String,

        /// Path to TLS certificate file (omit for dev mode)
        #[arg(long)]
        tls_cert: Option<String>,

        /// Path to TLS private key file (omit for dev mode)
        #[arg(long)]
        tls_key: Option<String>,

        /// Run in development mode (no TLS, plaintext WebSocket)
        #[arg(long, default_value_t = false)]
        dev: bool,

        /// Server access password (required unless --dev)
        #[arg(long)]
        auth: Option<String>,

        /// Directory containing pre-built binaries for /download page
        #[arg(long)]
        bin_dir: Option<String>,
    },

    /// Run in agent mode (connects to a relay)
    Agent {
        /// WebSocket URL of the relay server
        #[arg(long, default_value = "ws://localhost:3000")]
        relay_url: String,

        /// Fixed authentication key (optional, random token used if omitted)
        #[arg(long)]
        key: Option<String>,

        /// Default directory for file manager (defaults to $HOME)
        #[arg(long, env = "HOME")]
        root: String,

        /// Token type: rw, ro, or both
        #[arg(long, default_value = "rw")]
        token_type: TokenType,

        /// Shell path (e.g., /bin/bash, /usr/bin/zsh)
        #[arg(long, env = "SHELL", default_value = "/bin/bash")]
        shell: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    let version = env!("CARGO_PKG_VERSION");
    eprintln!("shell-remote v{}", version);

    match cli.command {
        Command::Relay {
            bind,
            tls_cert,
            tls_key,
            dev,
            auth,
            bin_dir,
        } => {
            relay::start(bind, tls_cert, tls_key, dev, auth, bin_dir).await?;
        }
        Command::Agent {
            relay_url,
            key,
            root,
            token_type,
            shell,
        } => {
            agent::start(relay_url, key, root, token_type.as_str().to_string(), shell).await?;
        }
    }

    Ok(())
}
