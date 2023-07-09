use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use mime::Mime;
use once_cell::sync::Lazy;
use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Response};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::spawn;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use url::Url;

mod output;
use output::*;

mod args;
use args::*;

mod html;
use html::*;

mod dir;
use dir::*;

fn main() {
    let exit_code = {
        // Parse command line arguments
        match get_args() {
            Ok(args) => {
                // Start tokio runtime and call the main function
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads(args.threads)
                    .build()
                    .unwrap()
                    .block_on(async {
                        match main_process(args).await {
                            Ok(_) => 0,
                            Err(e) => {
                                error!("{}", e.to_string());
                                1
                            }
                        }
                    })
            }
            Err(e) => {
                // Failed to parse arguments
                error!("{}", e.to_string());
                2
            }
        }
    };

    process::exit(exit_code);
}

pub struct State {
    processed_urls: Mutex<HashSet<String>>,
    conc_sem: Semaphore,
    client: Client,
    args: Args,
}

impl State {
    fn new(args: Args, client: Client) -> Self {
        Self {
            processed_urls: Mutex::new(HashSet::new()),
            conc_sem: Semaphore::new(args.concurrent_fetch),
            client,
            args,
        }
    }
}

type ArcState = Arc<State>;

async fn main_process(args: Args) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Make sure the URL parses first
    let parsed_url = Url::parse(&args.url)?;

    // Check the URL
    check_url(&parsed_url)?;

    // Create HTTP client
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(args.connect_timeout))
        .timeout(Duration::from_secs(args.fetch_timeout))
        .build()?;

    // Create state
    let state = Arc::new(State::new(args, client));

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
        debug!(state, 1, "Skipping {url} as it's already been processed");
        return Ok(());
    };

    // Acquire a slot
    let sem = state.conc_sem.acquire().await?;

    // Fetch the URL
    output!("Fetching {url}");
    let response = state.client.get(url.clone()).send().await?;

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
            debug!(state, 2, "{final_url} is {mime_type}, parsing...");

            // Get HTML body
            let html = response.text().await?;

            // Process HTML
            let join_handles = process_html(&state, final_url, html, &target);

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
        debug!(state, 1, "No content type for {url}");
        download(&state, &final_url, &target, response).await?;
    };

    Ok(())
}

fn process_href(
    state: &ArcState,
    base_url: &str,
    href: &str,
    target: &str,
) -> Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>> {
    // Parse the base URL
    let parsed_base_url = Url::parse(base_url).expect("URL should parse");

    // Join href to the base URL if necessary
    match parsed_base_url.join(href) {
        Ok(parsed_href_url) => {
            // Convert to string
            let href_url = parsed_href_url.to_string();

            if href != href_url {
                debug!(state, 2, "href {href} -> {href_url}");
            }

            if let Err(e) = check_url(&parsed_href_url) {
                debug!(state, 1, "Skipping: {e}");
            }

            // Check it's not a fragment
            if parsed_href_url.fragment().is_some() {
                debug!(state, 1, "Skipping: {href_url} is a fragment");
                return None;
            }

            // Check is doesn't have a query string
            if parsed_href_url.query().is_some() {
                debug!(state, 1, "Skipping: {href_url} has a query string");
                return None;
            }

            if href_url.len() < base_url.len() || !href_url.starts_with(base_url) {
                debug!(
                    state,
                    1, "Skipping: {href_url} is not relative to the base {base_url}"
                );
                return None;
            }

            // Clome state
            let state = state.clone();

            // Build sub directory
            let subdir = format!("{}{}", target, &href_url[base_url.len()..]);
            debug!(state, 2, "{href_url} - {base_url} = {subdir}");

            Some(spawn(
                async move { process_url(state, href_url, subdir).await },
            ))
        }
        Err(e) => {
            debug!(state, 1, "href {href} is not valid ({e})");
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
    debug!(state, 2, "Download: url={url}, file={file}");

    // Build path
    let mut path = PathBuf::from(&state.args.target);

    if file.is_empty() {
        path.push(&state.args.unnamed);
    } else {
        path.push(file);

        if url.ends_with('/') {
            path.push(&state.args.unnamed)
        }
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

    output!("Downloading {url} to {} (size {size})", path.display());

    // Open the file
    let mut file = File::create(path).await?;

    if state.args.debug_delay != 0 {
        sleep(Duration::from_millis(state.args.debug_delay)).await;
    }

    // Read next chunk
    while let Some(chunk) = response.chunk().await? {
        debug!(state, 2, "Read {} bytes", chunk.len());

        // Write chunk to the file
        file.write_all(&chunk).await?;

        if state.args.debug_delay != 0 {
            sleep(Duration::from_millis(state.args.debug_delay)).await;
        }
    }

    Ok(())
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
