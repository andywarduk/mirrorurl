use std::error::Error;
use std::process::ExitCode;
use std::sync::Arc;

use lazy_static::lazy_static;

mod output;

mod args;
use args::Args;

mod state;
use log::LevelFilter;
use output::Logger;
use state::{ArcState, State};

mod walk;
use stats::Stats;
use walk::walk;

use crate::output::error;

mod download;
mod etags;
mod html;
mod mime;
mod response;
mod skip;
mod skipreason;
mod stats;
mod url;

lazy_static! {
    /// Global logger
    static ref LOGGER: Logger = Logger::new();
}

fn main() -> ExitCode {
    // Set up logger
    log::set_logger(&*LOGGER).expect("Failed to set logger");
    log::set_max_level(LevelFilter::Info);

    match start() {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            error!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn start() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse command line arguments
    let args = Args::parse()?;

    if args.debug > 0 {
        // Set max log level to Debug if debugging required
        log::set_max_level(LevelFilter::Debug);

        if args.debug > 2 {
            // Log debug messages from all modules
            LOGGER.set_all_targets(true);
        }
    }

    // Create tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(args.threads)
        .build()?;

    // Start tokio runtime and call the main function
    runtime.block_on(async { process(args).await })?;

    Ok(())
}

async fn process(args: Args) -> Result<Stats, Box<dyn Error + Send + Sync>> {
    // Create shared state
    let state = Arc::new(State::new(args)?);

    // Process main url
    walk(&state, state.url()).await;

    // Get and print stats
    let stats = state.get_stats().await;
    stats.print();

    // Save the new etags list
    state.save_etags().await?;

    Ok(stats)
}

#[cfg(test)]
mod tests;
