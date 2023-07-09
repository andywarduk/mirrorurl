use std::error::Error;

use scraper::{Html, Selector};
use tokio::task::JoinHandle;

use super::output::*;
use super::{process_href, ArcState};

pub fn process_html(
    state: &ArcState,
    url: String,
    html: String,
    target: &str,
) -> Vec<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>> {
    // Process all of the links
    let mut join_handles = Vec::new();

    for href in parse_html(state, html) {
        // Look for href on each anchor
        if let Some(join) = process_href(state, &url, &href, target) {
            join_handles.push(join);
        }
    }

    join_handles
}

fn parse_html(state: &ArcState, html: String) -> Vec<String> {
    // Parse the document
    let document = Html::parse_document(&html);

    // Create anchor selector
    let anchor_sel = Selector::parse("a").unwrap();

    // Select all anchors
    let anchors = document.select(&anchor_sel);

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
