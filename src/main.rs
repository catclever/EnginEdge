use clap::{Parser, Subcommand};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use std::io::stderr;
use std::time::Duration;
use serde_json::json;

mod ast;
mod bridge;
mod protocol;
mod server;
mod tools;
mod security;
pub mod policy;
pub mod bench;
pub mod knowledge;

const DEFAULT_HUB_URL: &str = "wss://dalembert-production.up.railway.app";

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

    /// Security policy mode: 'dev' (default, max weight 40) or 'eval' (benchmark, max weight 75)
    #[arg(long, env = "ENGINEDGE_MODE", default_value = "dev")]
    mode: String,

    /// Custom max risk weight threshold (0-99). Overrides mode default.
    #[arg(long, env = "ENGINEDGE_MAX_WEIGHT")]
    max_weight: Option<u32>,

    /// Tools preset for MCP server: "full" (all 15 tools), "light" / "plugin" (AST + Cloud + Telemetry, 9 tools), "ast" / "ast-only" (3 tools)
    #[arg(long, env = "ENGINEDGE_TOOLS_PRESET")]
    preset: Option<String>,

    /// Comma-separated list of tools to forcefully include in MCP
    #[arg(long)]
    include: Option<String>,

    /// Comma-separated list of tools to forcefully exclude from MCP
    #[arg(long)]
    exclude: Option<String>,

    /// Enable the knowledge engine
    #[arg(long, env = "ENGINEDGE_ENABLE_KB", default_value_t = false)]
    enable_kb: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP stdio server (default)
    Mcp {
        /// Tools preset: "full", "light" / "plugin", "ast" / "ast-only", "kb"
        #[arg(long)]
        preset: Option<String>,

        /// Comma-separated list of tools to include
        #[arg(long)]
        include: Option<String>,

        /// Comma-separated list of tools to exclude
        #[arg(long)]
        exclude: Option<String>,

        /// Enable the knowledge engine
        #[arg(long, env = "ENGINEDGE_ENABLE_KB", default_value_t = false)]
        enable_kb: bool,
    },
    /// Run headless autonomous coding agent
    Run {
        /// Task prompt / description for autonomous coding
        #[arg(long, short)]
        task: String,

        /// Custom system prompt override
        #[arg(long)]
        system_prompt: Option<String>,

        /// LLM profile name on cloud hub
        #[arg(long)]
        profile: Option<String>,
    },
    /// Run automated coding benchmark evaluations against datasets
    Bench {
        /// Path to dataset directory or manifest (JSONL / directory of problem cases). Default: built-in suite.
        #[arg(long, short)]
        dataset: Option<String>,

        /// Comma-separated languages to evaluate (e.g. "rust,go,python,javascript")
        #[arg(long, default_value = "rust,go,python,javascript")]
        langs: String,

        /// Maximum number of tasks to evaluate (sampling limit)
        #[arg(long)]
        limit: Option<usize>,

        /// Output report path (Markdown)
        #[arg(long, short, default_value = "bench_report.md")]
        output: String,

        /// Directory to store per-task execution trace logs
        #[arg(long, default_value = "bench_logs")]
        log_dir: String,

        /// LLM profile name on cloud hub
        #[arg(long)]
        profile: Option<String>,

        /// Dry-run mode: verify sandbox setup and test runner without cloud LLM inference
        #[arg(long)]
        dry_run: bool,

        /// Timeout in seconds per task
        #[arg(long, default_value = "180")]
        timeout: u64,

        /// Maximum reasoning turns allowed on cloud hub per benchmark task
        #[arg(long, default_value = "50")]
        max_turns: usize,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize execution security policy (dev mode default weight 40, eval mode weight 75)
    policy::init_policy(&cli.mode, cli.max_weight);

    let is_headless = matches!(cli.command, Some(Commands::Run { .. }));

    // CRITICAL HYGIENE RULE: Configure tracing to write EXCLUSIVELY to stderr.
    // stdout MUST only contain valid JSON-RPC 2.0 protocol lines in MCP mode.
    let log_level = if is_headless { Level::INFO } else { Level::DEBUG };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_writer(stderr)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let hub_url = cli.hub_url
        .or_else(|| std::env::var("DESCARTES_WS_URL").ok())
        .or_else(|| std::env::var("ENGINEDGE_HUB_URL").ok())
        .or_else(|| Some(DEFAULT_HUB_URL.to_string()));

    let bridge = bridge::CloudBridge::new(hub_url.clone());
    bridge.start();

    match cli.command {
        Some(Commands::Mcp { preset, include, exclude, enable_kb }) => {
            let effective_preset = preset.or(cli.preset).unwrap_or_else(|| "full".to_string());
            let effective_include = include.or(cli.include);
            let effective_exclude = exclude.or(cli.exclude);
            let effective_enable_kb = enable_kb || cli.enable_kb;

            tracing::info!(
                "Starting EnginEdge MCP stdio server (preset: {}, include: {:?}, exclude: {:?}, kb: {})...",
                effective_preset,
                effective_include,
                effective_exclude,
                effective_enable_kb
            );
            let tools = tools::get_filtered_mcp_tools(
                Some(&effective_preset),
                effective_include.as_deref(),
                effective_exclude.as_deref(),
                effective_enable_kb,
            );
            server::run_stdio_server(bridge, tools).await?;
        }
        None => {
            let effective_preset = cli.preset.unwrap_or_else(|| "full".to_string());
            tracing::info!(
                "Starting EnginEdge MCP stdio server (default mode, preset: {}, include: {:?}, exclude: {:?}, kb: {})...",
                effective_preset,
                cli.include,
                cli.exclude,
                cli.enable_kb
            );
            let tools = tools::get_filtered_mcp_tools(
                Some(&effective_preset),
                cli.include.as_deref(),
                cli.exclude.as_deref(),
                cli.enable_kb,
            );
            server::run_stdio_server(bridge, tools).await?;
        }
        Some(Commands::Run { task, system_prompt, profile }) => {
            run_headless_agent(bridge, task, system_prompt, profile).await?;
        }
        Some(Commands::Bench {
            dataset,
            langs,
            limit,
            output,
            log_dir,
            profile,
            dry_run,
            timeout,
            max_turns,
        }) => {
            let lang_list = langs
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            let options = bench::BenchOptions {
                dataset_path: dataset,
                langs: lang_list,
                limit,
                output_path: output,
                log_dir,
                hub_url,
                profile,
                dry_run,
                timeout_secs: timeout,
                max_turns,
            };

            bench::run_benchmark(options).await?;
        }
    }

    Ok(())
}

async fn run_headless_agent(
    bridge: bridge::CloudBridge,
    task: String,
    system_prompt: Option<String>,
    profile: Option<String>,
) -> anyhow::Result<()> {
    eprintln!("🚀 [EnginEdge] Initializing headless autonomous agent...");
    eprintln!("📡 [EnginEdge] Connecting to Cloud Bridge at Railway...");

    if !bridge.wait_for_connection(Duration::from_secs(15)).await {
        anyhow::bail!("Failed to connect to Cloud Bridge within 15 seconds. Please verify network or hub status.");
    }
    eprintln!("✅ [EnginEdge] Connected to Cloud Bridge!");

    let mut default_system_prompt = "You are EnginEdge Autonomous Agent, an ultra-fast edge coding assistant. \
        You have local workspace tools (list_files, read_file, grep_search, write_file, run_verification, get_outline, read_symbol, replace_symbol) \
        and cloud intelligence tools (github_repo_survey, github_code_search, doc_fetcher, spec_inspector, knowledge_store). \
        Inspect the codebase, think step-by-step, make necessary edits, verify your work using tests or builds, and report final results.".to_string();

    let kb_summaries = knowledge::get_kb_summaries();
    let mut kb_enabled = false;

    if kb_summaries.active_summary.is_some() || kb_summaries.grave_summary.is_some() {
        kb_enabled = true;
        default_system_prompt.push_str("\n\nPROJECT KNOWLEDGE BASE:\n");
        if let Some(active) = kb_summaries.active_summary {
            default_system_prompt.push_str(&format!("\nActive Knowledge:\n{}\n", active));
        }
        if let Some(grave) = kb_summaries.grave_summary {
            default_system_prompt.push_str(&format!("\nGraveyard (Anti-patterns):\n{}\n", grave));
        }
    }

    let init_payload = json!({
        "type": "init",
        "task_name": "enginedge_headless_task",
        "prompt": task,
        "system_prompt": system_prompt.unwrap_or(default_system_prompt),
        "llm_profile": profile.unwrap_or_else(|| "default".to_string()),
        "tools": tools::all_local_tool_schemas(kb_enabled),
    });

    let mut event_rx = bridge.subscribe_events();
    bridge.send_raw(init_payload.to_string()).await
        .map_err(|e| anyhow::anyhow!("Failed to send init payload: {}", e))?;

    eprintln!("✨ [EnginEdge] Task dispatched. Awaiting agent execution events...\n");

    while let Ok(event) = event_rx.recv().await {
        match event {
            bridge::BridgeEvent::StreamEvent { event, data } => {
                match event.as_str() {
                    "thought" => {
                        let text = data.as_str().unwrap_or("");
                        eprintln!("💭 [Thought] {}", text);
                    }
                    "tool_call_start" => {
                        eprintln!("🛠️ [Tool Call] {}", data);
                    }
                    "tool_call_end" => {
                        eprintln!("✔️ [Tool Result] {}", data);
                    }
                    _ => {
                        eprintln!("ℹ️ [Event] {} : {}", event, data);
                    }
                }
            }
            bridge::BridgeEvent::Finished { result, status } => {
                eprintln!("\n🏁 [EnginEdge] Task completed with status: {}", status);
                if let Some(res_str) = result.as_str() {
                    println!("{}", res_str);
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                return Ok(());
            }
            bridge::BridgeEvent::Error { message } => {
                eprintln!("\n❌ [EnginEdge] Cloud Agent error: {}", message);
                anyhow::bail!("Agent execution failed: {}", message);
            }
        }
    }

    Ok(())
}
