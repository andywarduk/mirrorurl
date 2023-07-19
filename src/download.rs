use std::error::Error;
use std::path::PathBuf;

use reqwest::header::ETAG;
use tokio::fs::{create_dir_all, File};
use tokio::io::AsyncWriteExt;

use crate::output::{debug, error, output};
use crate::response::Response;
use crate::url::Url;
use crate::ArcState;

/// Downloads a URL to a file
pub async fn download(
    state: &ArcState,
    url: &Url,
    final_url: &Url,
    mut response: Response,
) -> Result<usize, Box<dyn Error + Send + Sync>> {
    // Build full download path
    let path = state.path_for_url(final_url).await?;

    // Download to file
    let bytes = download_to_path(state, final_url, &mut response, path).await?;

    // Get response etag
    match response.headers().get(ETAG).map(|value| value.to_str()) {
        Some(Ok(etag)) => {
            // Add etag for original and final url (if different)
            debug!(state, 1, "etag for {url} (final {final_url}): {etag}");
            state.add_etags(vec![url, final_url], etag).await;
        }
        Some(_) => {
            // Etag is invalid
            error!("Invalid etag header received from {url}");
        }
        None => {
            // No etag received
            debug!(state, 1, "No etag header received");
        }
    }

    Ok(bytes)
}

pub async fn download_to_path(
    state: &ArcState,
    final_url: &Url,
    response: &mut Response,
    path: PathBuf,
) -> Result<usize, Box<dyn Error + Send + Sync>> {
    // Create directories if necessary
    if let Some(parent) = path.parent() {
        if !parent.is_dir() {
            create_dir_all(parent)
                .await
                .map_err(|e| format!("Unable to create directory {}: {e}", path.display()))?;
        }
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
    let mut file = File::create(&path)
        .await
        .map_err(|e| format!("Unable to create file {}: {e}", path.display()))?;

    // Debug delay
    state.debug_delay().await;

    // Read next chunk
    let mut bytes = 0;

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Error downloading chunk: {e}"))?
    {
        bytes += chunk.len();
        debug!(state, 2, "Read {} bytes", chunk.len());

        // Write chunk to the file
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Error writing to {}: {e}", path.display()))?;

        // Debug delay
        state.debug_delay().await;
    }

    Ok(bytes)
}
