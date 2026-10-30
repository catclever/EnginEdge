use clap::{Parser, Subcommand};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use std::io::stderr;

mod ast;
mod protocol;
mod server;
mod tools;

#[derive(Parser)]
#[command(name = "enginedge")]
#[command(version)]
#[command(about = "EnginEdge - Ultra-fast, native edge agent and MCP server")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP stdio server (default)
    Mcp,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // CRITICAL HYGIENE RULE: Configure tracing to write EXCLUSIVELY to stderr.
    // stdout MUST only contain valid JSON-RPC 2.0 protocol lines.
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_writer(stderr)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Mcp) | None => {
            tracing::info!("Starting EnginEdge MCP stdio server...");
            server::run_stdio_server().await?;
        }
    }

    Ok(())
}
