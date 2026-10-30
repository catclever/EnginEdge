use clap::{Parser, Subcommand};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use std::io::stderr;

mod ast;
mod bridge;
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

    /// URL of the Descartes/Dalembert Cloud Bridge
    #[arg(long, env = "DESCARTES_WS_URL", alias = "ENGINEDGE_HUB_URL")]
    hub_url: Option<String>,
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

    let mut hub_url = cli.hub_url;
    if hub_url.is_none() {
        hub_url = std::env::var("DESCARTES_WS_URL")
            .or_else(|_| std::env::var("ENGINEDGE_HUB_URL"))
            .ok();
    }

    let bridge = bridge::CloudBridge::new(hub_url);
    bridge.start();

    match cli.command {
        Some(Commands::Mcp) | None => {
            tracing::info!("Starting EnginEdge MCP stdio server...");
            server::run_stdio_server(bridge).await?;
        }
    }

    Ok(())
}
