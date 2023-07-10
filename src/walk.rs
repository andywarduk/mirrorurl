use std::error::Error;

use reqwest::header::{HeaderMap, HeaderValue};
use url::Url;

use crate::download::download;
use crate::html::{is_html, process_html};
use crate::output::{debug, error, output};
use crate::state::ArcState;

pub async fn walk(state: &ArcState, url: &Url) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Already seen this URL?
    if !state.add_processed_url(url.clone()).await {
        debug!(state, 1, "Skipping {url} as it's already been processed");
        return Ok(());
    };

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
        // Not OK - check for not modified
        if status == 304 && old_etag.is_some() {
            output!("{url} is not modified");
            return Ok(());
        }

        Err(format!("Status {status} fetching {final_url}"))?;
    } else {
        debug!(state, 2, "Status {status}");
    }

    // Is the document HTML?
    if is_html(state, &response) {
        // Get HTML body
        let html = response.text().await?;

        // Release the download slot
        drop(sem);

        // Process HTML
        let join_handles = process_html(state, &final_url, html);

        // Join the threads
        for j in join_handles {
            match j.await {
                Ok(res) => {
                    if let Err(e) = res {
                        error!("{}", e.to_string());
                    }
                }
                Err(e) => {
                    error!("{}", e.to_string());
                }
            }
        }
    } else {
        // Download the resource
        download(state, url, &final_url, response).await?;

        // Release the download slot
        drop(sem);
    }

    Ok(())
}
