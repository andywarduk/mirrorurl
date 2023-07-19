use std::error::Error;

use reqwest::header::{HeaderMap, HeaderValue};

use crate::download::download;
use crate::html::process_html;
use crate::output::{debug, error, output};
use crate::response::ResponseExt;
use crate::skipreason::SkipReasonErr;
use crate::state::ArcState;
use crate::url::Url;

/// Loads data from a URL. If the data is HTML, parse the document and follow links.
/// Otherwise download the file.
/// Use loaded etags to determine if the resource has already been downloaded and skip if so.
pub async fn walk(state: &ArcState, url: &Url) {
    match walk_internal(state, url).await {
        Ok(()) => {}
        Err(e) if e.is::<SkipReasonErr>() => {
            output!("{e}");
            state.update_stats(|mut stats| stats.add_skipped()).await;
        }
        Err(e) if matches!(e.source(), Some(e) if e.is::<SkipReasonErr>()) => {
            // Error from the redirect policy
            output!("{}", e.source().unwrap());
            state.update_stats(|mut stats| stats.add_skipped()).await;
        }
        Err(e) => {
            error!("{e}");
            state.update_stats(|mut stats| stats.add_errored()).await;
        }
    }
}

pub async fn walk_internal(
    state: &ArcState,
    url: &Url,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Already seen this URL?
    if !state.add_processed_url(url.clone()).await {
        debug!(state, 1, "URL {url} has already been processed");
        return Ok(());
    };

    // Check path
    let _ = state.path_for_url(url).await?;

    // Create additional HTTP headers
    let mut headers = HeaderMap::new();

    // Is there an etag for this URL?
    let old_etag = state.find_etag(url);

    if let Some(old_etag) = old_etag {
        debug!(state, 2, "Previous etag value: {old_etag}");

        // Set the If-None-Match request header to the old etag
        if let Ok(value) = HeaderValue::from_str(old_etag) {
            headers.insert("If-None-Match", value);
        } else {
            error!("Previous etag value {old_etag} is not valid");
        }
    }

    // Acquire a download slot
    let sem = state.acquire_slot().await?;

    // Fetch the URL
    output!("Fetching {url}");

    let response = state
        .client()
        .get(url.clone())
        .headers(headers)
        .send()
        .await?;

    // Get final URL after any redirects
    let final_url = response.url().clone();

    // Get status code
    let status = response.status();

    // Check status code
    if !status.is_success() {
        // Not OK - check status
        match status.as_u16() {
            304 if old_etag.is_some() => {
                state
                    .update_stats(|mut stats| stats.add_not_modified())
                    .await;
                output!("{url} is not modified");
            }
            _ => Err(format!("Status {status} fetching {final_url}"))?,
        }

        return Ok(());
    } else {
        debug!(state, 2, "Status {status}");
    }

    // Is the document HTML?
    if response.is_html(state) {
        // Get HTML body
        let html = response.text().await?;

        // Release the download slot
        drop(sem);

        // Add html stats
        let html_bytes = html.len();
        state
            .update_stats(|mut stats| stats.add_html(html_bytes))
            .await;

        // Process HTML
        let join_handles = process_html(state, &final_url, html).await;

        // Join the threads
        for j in join_handles {
            match j.await {
                Ok(()) => {}
                Err(e) => {
                    error!("Failed to join thread: {e}");
                }
            }
        }
    } else {
        // Download the resource
        let bytes = download(state, url, &final_url, response).await?;

        // Release the download slot
        drop(sem);

        // Add download stats
        state
            .update_stats(|mut stats| stats.add_download(bytes))
            .await;
    }

    Ok(())
}
