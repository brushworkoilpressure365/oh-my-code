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
    // Load `.env` from the current working directory if it exists. Missing
    // file is not an error — in production the variables come from the real
    // process environment. See `.env.example` for the documented set.
    let _ = dotenvy::dotenv();

    let config = config::AppConfig::load()?;
    let mut repl = cli::Repl::new(config)?;
    repl.run().await
}
