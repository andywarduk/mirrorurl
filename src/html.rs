use std::error::Error;

use once_cell::sync::Lazy;
use scraper::{Html, Selector};
use tokio::task::JoinHandle;

use crate::output::{debug, output};
use crate::skipreason::{SkipReason, SkipReasonErr};
use crate::state::ArcState;
use crate::url::{Url, UrlExt};
use crate::walk::walk_recurse;

/// Process all of the links in an HTML document returning a list of join handles for spawned download tasks
pub async fn process_html(state: &ArcState, url: &Url, html: String) -> Vec<JoinHandle<()>> {
    // Process all of the links
    let mut join_handles = Vec::new();

    // Get hrefs out of the document
    let hrefs = parse_html(html);

    // Process each href
    for href in hrefs {
        match process_href(state, url, &href).await {
            // TODO just stats.add_errored(e) to consolidate?
            Err(e) if e.is::<SkipReasonErr>() => {
                state.update_stats(|mut stats| stats.add_skipped()).await;
                output!("{e}")
            }
            Err(e) => {
                state.update_stats(|mut stats| stats.add_errored()).await;
                output!("{e}")
            }
            Ok(join) => join_handles.push(join),
        }
    }

    join_handles
}

/// Anchor selector
static ANCHOR_SEL: Lazy<Selector> = Lazy::new(|| Selector::parse("a[href]").unwrap());

/// Parse an HTML document and return a list of href links to process
fn parse_html(html: String) -> Vec<String> {
    // Parse the document
    let document = Html::parse_document(&html);

    // Select all anchors
    let anchors = document.select(&ANCHOR_SEL);

    // Get all hrefs
    anchors
        .into_iter()
        .filter_map(|a| a.value().attr("href"))
        .map(|a| a.to_string())
        .collect()
}

/// Process a href on a base URL
async fn process_href<'a>(
    state: &'a ArcState,
    base_url: &'a Url,
    href: &'a str,
) -> Result<JoinHandle<()>, Box<dyn Error + Send + Sync>> {
    // Join href to the base URL if necessary
    let join = match base_url.join(href) {
        Ok(href_url) => {
            debug!(state, 2, "href {href} of {base_url} -> {href_url}");

            href_url.is_handled()?;

            // Check it's not a fragment
            if href_url.fragment().is_some() {
                Err(SkipReasonErr::new(
                    href_url.to_string(),
                    SkipReason::Fragment,
                ))?;
            }

            // Check is doesn't have a query string
            if href_url.query().is_some() {
                Err(SkipReasonErr::new(href_url.to_string(), SkipReason::Query))?;
            }

            // Check the URL is relative to the base URL
            if !href_url.is_relative_to(state.url()) {
                Err(SkipReasonErr::new(
                    href_url.to_string(),
                    SkipReason::NotRelative,
                ))?;
            }

            // Recurse in to this URL
            walk_recurse(state, href_url).await?
        }
        Err(e) => Err(SkipReasonErr::new(
            href.to_string(),
            SkipReason::NotValid(e),
        ))?,
    };

    Ok(join)
}
