mod agent;
mod cli;
mod config;
mod model;
mod search;
mod session;
mod tool;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let config = config::AppConfig::load()?;
    let mut repl = cli::Repl::new(config)?;
    repl.run().await
}
