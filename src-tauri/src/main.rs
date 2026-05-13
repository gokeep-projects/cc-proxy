#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cc-proxy", about = "API Proxy for Claude/Codex to OpenAI-compatible providers")]
struct Cli {
    /// Path to config file
    #[arg(long, short, default_value = "config.toml")]
    config: PathBuf,

    /// Run in headless mode (no GUI)
    #[arg(long)]
    headless: bool,

    /// Override proxy port
    #[arg(long, short)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.headless || std::env::var("CC_PROXY_HEADLESS").is_ok() {
        cc_proxy_lib::run_headless(cli.config, cli.port).await;
    } else {
        cc_proxy_lib::run();
    }
}
