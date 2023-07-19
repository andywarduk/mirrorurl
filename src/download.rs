use std::error::Error;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use reqwest::header::ETAG;
use tokio::fs::{create_dir_all, remove_file, rename, File};
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

    // Build temp file name
    let mut tmp_file_name = match path.file_name() {
        Some(name) => OsString::from(name),
        None => OsString::from("tmp"),
    };
    tmp_file_name.push(OsString::from(".mirrorurl"));

    // Build temp path
    let tmp_path = path.with_file_name(tmp_file_name);

    // Download to temp file
    let bytes = match download_to_path(state, final_url, &mut response, &path, &tmp_path).await {
        Ok(bytes) => {
            // Try and rename the file
            match rename(&tmp_path, path).await {
                Ok(_) => bytes,
                Err(e) => {
                    // Failed - try and remove temp file
                    let _ = remove_file(&tmp_path).await;
                    Err(e)?
                }
            }
        }
        Err(e) => {
            // Failed - try and remove temp file
            let _ = remove_file(&tmp_path).await;
            Err(e)?
        }
    };

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
    final_path: &Path,
    tmp_path: &PathBuf,
) -> Result<usize, Box<dyn Error + Send + Sync>> {
    // Create directories if necessary
    if let Some(parent) = tmp_path.parent() {
        if !parent.is_dir() {
            create_dir_all(parent)
                .await
                .map_err(|e| format!("Unable to create directory {}: {e}", parent.display()))?;
        }
    }

    // Calculate size string
    let size = response
        .content_length()
        .map(|s| format!("{s}"))
        .unwrap_or(String::from("unknown"));

    output!(
        "Downloading {final_url} to {} (size {size})",
        final_path.display()
    );

    // Open the file
    let mut file = File::create(&tmp_path)
        .await
        .map_err(|e| format!("Unable to create file {}: {e}", tmp_path.display()))?;

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
            .map_err(|e| format!("Error writing to {}: {e}", tmp_path.display()))?;

        // Debug delay
        state.debug_delay().await;
    }

    Ok(bytes)
}
