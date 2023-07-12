use std::error::Error;
use std::process;
use std::sync::Arc;

mod output;
use output::{error, output};

mod args;
use args::Args;

mod state;
use state::{ArcState, State};

mod walk;
use walk::walk;

mod download;
mod etags;
mod html;
mod mime;
mod response;
mod skip;
mod url;

fn main() {
    let exit_code = {
        match start() {
            Ok(_) => 0,
            Err(e) => {
                error!("{}", e.to_string());
                1
            }
        }
    };

    process::exit(exit_code);
}

fn start() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse command line arguments
    let args = Args::parse()?;

    // Create tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(args.threads)
        .build()?;

    // Start tokio runtime and call the main function
    runtime.block_on(async { process(args).await })?;

    Ok(())
}

async fn process(args: Args) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Create shared state
    let state = Arc::new(State::new(args)?);

    // Process main url
    walk(&state, state.url()).await?;

    // Save the new etags list
    state.save_etags().await?;

    Ok(())
}

#[cfg(test)]
mod tests;
