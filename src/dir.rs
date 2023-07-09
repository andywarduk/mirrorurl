use std::error::Error;
use std::path::Path;

use futures::future::{BoxFuture, FutureExt};
use once_cell::sync::Lazy;
use tokio::fs::{create_dir, metadata};

use crate::output::debug;
use crate::ArcState;

static EMPTY_PATH: Lazy<&Path> = Lazy::new(|| Path::new(""));

pub fn create_directories<'a>(
    state: &'a ArcState,
    path: &'a Path,
) -> BoxFuture<'a, Result<(), Box<dyn Error + Send + Sync>>> {
    async move {
        if path != *EMPTY_PATH {
            // Create parents first
            if let Some(parent) = path.parent() {
                create_directories(state, parent).await?;
            }

            let mut tried = false;

            loop {
                // Get path metadata
                match metadata(path).await {
                    Err(e) => match e.kind() {
                        // Failed - if not found try and create
                        std::io::ErrorKind::NotFound => match create_dir(path).await {
                            Ok(()) => {
                                // Created
                                debug!(state, 2, "Created directory {}", path.display());
                            }
                            Err(e) => {
                                // Failed to create - try two times to avoid races
                                if tried {
                                    Err(e)?;
                                }

                                tried = true;
                                continue;
                            }
                        },
                        _ => Err(e)?,
                    },
                    Ok(meta) => {
                        // Got metadata. Is it a directory?
                        if !meta.is_dir() {
                            // Something exists but it's not a directory
                            Err(format!(
                                "{} already exists and is not a directory",
                                path.display()
                            ))?
                        }
                    }
                }

                break;
            }
        }

        Ok(())
    }
    .boxed()
}
