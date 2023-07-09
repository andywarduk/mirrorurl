use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;

use futures::future::{BoxFuture, FutureExt};
use mime::Mime;
use once_cell::sync::Lazy;
use reqwest::header::CONTENT_TYPE;
use reqwest::{get, Response};
use scraper::{Html, Selector};
use tokio::fs::{create_dir, metadata, File};
use tokio::io::AsyncWriteExt;
use tokio::spawn;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use url::Url;

macro_rules! output {
    ($($arg:tt)*) => {{
        println!("{}", format!($($arg)*));
    }};
}

macro_rules! error {
    ($($arg:tt)*) => {{
        eprintln!("ERROR: {}", format!($($arg)*));
    }};
}

macro_rules! debug {
    ($state:ident, $($arg:tt)*) => (
        {
            if $state.as_ref().args.debug {
                eprintln!("DEBUG: {}", format!($($arg)*));
            }
        }
    )
}

struct State {
    processed_urls: Mutex<HashSet<String>>,
    conc_sem: Semaphore,
    args: Args,
}

impl State {
    fn new(args: Args) -> Self {
        Self {
            processed_urls: Mutex::new(HashSet::new()),
            conc_sem: Semaphore::new(args.concurrent_fetch),
            args,
        }
    }
}

struct Args {
    url: String,
    target: String,
    concurrent_fetch: usize,
    debug: bool,
    debug_delay: u64,
}

type ArcState = Arc<State>;

#[tokio::main]
async fn main() {
    let exit_code = match main_process().await {
        Ok(_) => 0,
        Err(e) => {
            error!("{}", e.to_string());
            1
        }
    };

    process::exit(exit_code);
}

async fn main_process() -> Result<(), Box<dyn Error + Send + Sync>> {
    // TODO replace with clap
    let args = Args {
        url: "http://git.wardy/patch.git/objects/".to_string(),
        target: "download".to_string(),
        concurrent_fetch: 10,
        debug: true,
        debug_delay: 1000,
    };

    // Make sure the URL parses first
    let parsed_url = Url::parse(&args.url)?;

    // Check the URL
    check_url(&parsed_url)?;

    // Create state
    let state = Arc::new(State::new(args));

    // Clone main url
    let main_url = state.args.url.clone();

    // Process main url
    process_url(state, main_url, "".to_string()).await?;

    Ok(())
}

static MIME_HTML: Lazy<Mime> = Lazy::new(|| "text/html".parse::<Mime>().unwrap());
static MIME_XHTML: Lazy<Mime> = Lazy::new(|| "application/xhtml+xml".parse::<Mime>().unwrap());

async fn process_url(
    state: ArcState,
    url: String,
    target: String,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Already seen this URL?
    if !state.processed_urls.lock().await.insert(url.clone()) {
        debug!(state, "Skipping {url} as it's already been processed");
        return Ok(());
    };

    // Acquire a slot
    let sem = state.conc_sem.acquire().await?;

    // Fetch the URL
    output!("Fetching {url}...");
    let response = get(url.clone()).await?;

    // Release the slot
    drop(sem);

    // Were redirects followed?
    let final_url = response.url().to_string();

    if final_url != url {
        output!("{url} was redirected to {final_url}");
    }

    // Check for fatal statuses
    response.error_for_status_ref()?;

    // Get content type
    if let Some(mime_type) = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<Mime>().ok())
    {
        // Is it html or xhtml?
        if (mime_type.type_() == MIME_HTML.type_() && mime_type.subtype() == MIME_HTML.subtype())
            || (mime_type.type_() == MIME_XHTML.type_()
                && mime_type.subtype() == MIME_XHTML.subtype())
        {
            debug!(state, "{final_url} is {mime_type}, parsing...");

            // Get HTML body
            let html = response.text().await?;

            // Process HTML
            let join_handles = process_html(&state, final_url, html);

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
            download(&state, &final_url, &target, response).await?;
        }
    } else {
        debug!(state, "No content type for {url}");
        download(&state, &final_url, &target, response).await?;
    };

    Ok(())
}

fn process_html(
    state: &ArcState,
    url: String,
    html: String,
) -> Vec<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>> {
    // Process all of the links
    let mut join_handles = Vec::new();

    for href in parse_html(state, html) {
        // Look for href on each anchor
        if let Some(join) = process_link(state, &url, &href) {
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
                debug!(state, "Skipping anchor as it has no href ({})", a.html());
            }

            r
        })
        .map(|a| a.to_string())
        .collect::<Vec<_>>()
}

fn process_link(
    state: &ArcState,
    base_url: &str,
    href: &str,
) -> Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>> {
    // Parse the base URL
    let parsed_base_url = Url::parse(base_url).expect("URL should parse");

    // Join href to the base URL if necessary
    match parsed_base_url.join(href) {
        Ok(parsed_href_url) => {
            // Convert to string
            let href_url = parsed_href_url.to_string();

            if href != href_url {
                debug!(state, "href {href} -> {href_url}");
            }

            if let Err(e) = check_url(&parsed_href_url) {
                debug!(state, "Skipping: {e}");
            }

            // Check it's not a fragment
            if parsed_href_url.fragment().is_some() {
                debug!(state, "Skipping: {href_url} is a fragment");
                return None;
            }

            // Check is doesn't have a query string
            if parsed_href_url.query().is_some() {
                debug!(state, "Skipping: {href_url} has a query string");
                return None;
            }

            if href_url.len() < base_url.len() || !href_url.starts_with(base_url) {
                debug!(
                    state,
                    "Skipping: {href_url} is not relative to the base {base_url}"
                );
                return None;
            }

            // Clome state
            let state = state.clone();

            // Build sub directory
            let subdir = href_url[base_url.len()..].to_string();

            Some(spawn(
                async move { process_url(state, href_url, subdir).await },
            ))
        }
        Err(e) => {
            debug!(state, "href {href} is not valid ({e})");
            None
        }
    }
}

async fn download(
    state: &ArcState,
    url: &str,
    file: &str,
    mut response: Response,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Build path
    let mut path = PathBuf::from(&state.args.target);

    path.push(file);

    if url.ends_with('/') {
        path.push("__file.dat")
    }

    // Create directory if necessary
    if let Some(parent) = path.parent() {
        create_directories(state, parent).await?;
    }

    // Calculate size string
    let size = response
        .content_length()
        .map(|s| format!("{s}"))
        .unwrap_or(String::from("unknown"));

    debug!(
        state,
        "Downloading {url} to {} (size {size})...",
        path.display()
    );

    // Open the file
    let mut file = File::create(path).await?;

    if state.args.debug_delay != 0 {
        sleep(Duration::from_millis(state.args.debug_delay)).await;
    }

    // Read next chunk
    while let Some(chunk) = response.chunk().await? {
        debug!(state, "Read {} bytes", chunk.len());

        // Write chunk to the file
        file.write_all(&chunk).await?;

        if state.args.debug_delay != 0 {
            sleep(Duration::from_millis(state.args.debug_delay)).await;
        }
    }

    Ok(())
}

static EMPTY_PATH: Lazy<&Path> = Lazy::new(|| Path::new(""));

fn create_directories<'a>(
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
                                debug!(state, "Created directory {}", path.display());
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

fn check_url(url: &Url) -> Result<(), String> {
    // Check scheme
    match url.scheme() {
        "http" | "https" => (),
        _ => {
            Err(format!("{url} is not an http or https scheme"))?;
        }
    }

    Ok(())
}
