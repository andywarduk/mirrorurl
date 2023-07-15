use std::error::Error;

use once_cell::sync::Lazy;
use scraper::{Html, Selector};
use tokio::spawn;
use tokio::task::JoinHandle;

use crate::output::{debug, output};
use crate::skipreason::{SkipReason, SkipReasonErr};
use crate::state::ArcState;
use crate::url::{Url, UrlExt};
use crate::walk::walk;

/// Process all of the links in an HTML document returning a list of join handles for spawned download tasks
pub fn process_html(
    state: &ArcState,
    url: &Url,
    html: String,
) -> Vec<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>> {
    // Process all of the links
    let mut join_handles = Vec::new();

    // Get hrefs out of the document
    let hrefs = parse_html(state, html);

    // Process each href
    for href in hrefs {
        match process_href(state, url, &href) {
            Err(e) => output!("{e}"),
            Ok(join) => join_handles.push(join),
        }
    }

    join_handles
}

/// Anchor selector
static ANCHOR_SEL: Lazy<Selector> = Lazy::new(|| Selector::parse("a").unwrap());

/// Parse an HTML document and return a list of href links to process
fn parse_html(state: &ArcState, html: String) -> Vec<String> {
    // Parse the document
    let document = Html::parse_document(&html);

    // Select all anchors
    let anchors = document.select(&ANCHOR_SEL);

    // Get all hrefs
    anchors
        .into_iter()
        .filter_map(|a| {
            let r = a.value().attr("href");

            if r.is_none() {
                debug!(state, 1, "Skipping anchor as it has no href ({})", a.html());
            }

            r
        })
        .map(|a| a.to_string())
        .collect::<Vec<_>>()
}

type ThreadResult = Result<(), Box<dyn Error + Send + Sync>>;

/// Process a href on a base URL
fn process_href(
    state: &ArcState,
    base_url: &Url,
    href: &str,
) -> Result<JoinHandle<ThreadResult>, SkipReasonErr> {
    // Join href to the base URL if necessary
    match base_url.join(href) {
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

            // Clone state
            let state = state.clone();

            // Spawn a task to process the url
            Ok(spawn(async move { walk(&state, &href_url).await }))
        }
        Err(e) => Err(SkipReasonErr::new(
            href.to_string(),
            SkipReason::NotValid(e),
        )),
    }
}
