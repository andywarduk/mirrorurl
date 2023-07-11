use std::error::Error;

use reqwest::header::ETAG;
use reqwest::Response;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::dir::*;
use crate::output::{debug, error, output};
use crate::url::Url;
use crate::ArcState;

/// Downloads a URL to a file
pub async fn download(
    state: &ArcState,
    url: &Url,
    final_url: &Url,
    mut response: Response,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Build full download path
    let path = state.path_for_url(final_url);

    // Get download relative path
    let rel_path = state.download_relative_path(&path)?;

    // Check skip list
    if state.path_in_skip_list(rel_path) {
        debug!(state, 1, "Skipping: Relative path is in the skip list");
    } else {
        // Create directory if necessary
        if let Some(parent) = path.parent() {
            create_directories(state, parent).await?;
        }

        // Calculate size string
        let size = response
            .content_length()
            .map(|s| format!("{s}"))
            .unwrap_or(String::from("unknown"));

        output!(
            "Downloading {final_url} to {} (size {size})",
            path.display()
        );

        // Open the file
        let mut file = File::create(path).await?;

        // Debug delay
        state.debug_delay().await;

        // Read next chunk
        while let Some(chunk) = response.chunk().await? {
            debug!(state, 2, "Read {} bytes", chunk.len());

            // Write chunk to the file
            file.write_all(&chunk).await?;

            // Debug delay
            state.debug_delay().await;
        }
    }

    // Get response etag
    match response.headers().get(ETAG).map(|value| value.to_str()) {
        Some(Ok(etag)) => {
            // Add etag for original and final url (if different)
            let mut urls = Vec::with_capacity(2);

            if url != final_url {
                urls.push(url);
            }

            urls.push(final_url);

            state.add_etags(urls, etag).await;
        }
        Some(_) => {
            error!("Invalid etag header received from {url}");
        }
        None => {
            debug!(state, 1, "No etag header received");
        }
    }

    Ok(())
}
