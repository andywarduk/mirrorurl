use std::error::Error;
use std::process::ExitCode;
use std::sync::Arc;

use args::Args;
use log::LevelFilter;
use once_cell::sync::Lazy;
use output::{error, output, Logger};
use simple_process_stats::ProcessStats;
use state::{ArcState, State};
use stats::Stats;
use tokio::time::Instant;
use walk::walk;

mod args;
mod download;
mod etags;
mod html;
mod mime;
mod output;
mod response;
mod skip;
mod skipreason;
mod state;
mod stats;
mod url;
mod walk;

#[cfg(test)]
mod tests;

static LOGGER: Lazy<Logger> = Lazy::new(Logger::new);

/// Program entry point
fn main() -> ExitCode {
    // Set up logger
    log::set_logger(&*LOGGER).expect("Failed to set logger");
    log::set_max_level(LevelFilter::Info);

    match start_async() {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            error!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Parse command line args, start tokio and run
fn start_async() -> Result<(), Box<dyn Error + Send + Sync>> {
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
    runtime.block_on(async {
        let start = Instant::now();
        let result = async_main(args).await;
        print_process_stats(start).await;
        result
    })?;

    Ok(())
}

/// Async entry point
async fn async_main(args: Args) -> Result<Stats, Box<dyn Error + Send + Sync>> {
    // Create shared state
    let state = Arc::new(State::new(args)?);

    // Acquire a download slot
    let sem = state.acquire_slot().await?;

    // Process main url
    walk(&state, state.url(), sem).await;

    // Get and print stats
    let stats = state.get_stats().await;
    stats.print();

    // Save the new etags list
    state.save_etags().await?;

    Ok(stats)
}

async fn print_process_stats(start: Instant) {
    let end = Instant::now();

    // Print run time
    output!(
        "Run time: {:.2} seconds",
        end.duration_since(start).as_secs_f64()
    );

    // Print cpu stats
    if let Ok(cpu_stats) = ProcessStats::get().await {
        output!(
            "CPU time: user {:.2} seconds, kernel {:.2} seconds",
            cpu_stats.cpu_time_user.as_secs_f64(),
            cpu_stats.cpu_time_kernel.as_secs_f64(),
        );
    } else {
        error!("Unable to get CPU usage stats")
    }
}
