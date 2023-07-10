use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use reqwest::redirect::Policy;
use reqwest::Client;
use tokio::sync::{Mutex, Semaphore, SemaphorePermit};
use tokio::time::{sleep, Duration};
use url::Url;

use crate::args::Args;
use crate::etags::ETags;
use crate::output::debug;
use crate::skip::SkipList;
use crate::url::{url_relative_path, url_relative_to};

/// Program state shared between all threads
pub struct State {
    url: Url,
    processed_urls: Mutex<HashSet<Url>>,
    etags_file: String,
    old_etags: ETags,
    new_etags: Mutex<ETags>,
    skip_list: SkipList,
    conc_sem: Semaphore,
    client: Client,
    args: Args,
}

impl State {
    /// Creates the state
    pub fn new(args: Args) -> Result<Self, Box<dyn Error + Send + Sync>> {
        // Make sure the URL parses first
        let url = Url::parse(&args.url)?;

        // Check the URL is processable
        Self::check_url(&url)?;

        // Create HTTP client
        let client = Self::create_http_client(&args, url.clone())?;

        // Build etags file path
        let mut etags_file = PathBuf::from(&args.target);
        etags_file.push(".etags.json");
        let etags_file = etags_file
            .to_str()
            .ok_or("Unable to build path to .etags")?;

        // Load etags if present
        let etags = ETags::new_from_file(etags_file)?;

        // Load skip list
        let skip_list = if let Some(skip_file) = &args.skip_file {
            SkipList::new_from_file(skip_file)?
        } else {
            SkipList::new()
        };

        Ok(Self {
            url,
            processed_urls: Mutex::new(HashSet::new()),
            etags_file: etags_file.to_string(),
            old_etags: etags,
            new_etags: Mutex::new(ETags::default()),
            skip_list,
            conc_sem: Semaphore::new(args.concurrent_fetch),
            client,
            args,
        })
    }

    /// Returns a reference to the HTTP client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Save the etags file
    pub async fn save_etags(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Save etags
        self.new_etags
            .lock()
            .await
            .extend(&self.old_etags)
            .save_to_file(&self.etags_file)
    }

    /// Adds a URL to the processed list. Returns false if URL alredy seen
    pub async fn add_processed_url(&self, url: Url) -> bool {
        self.processed_urls.lock().await.insert(url)
    }

    /// Looks for an etag in the etag list for a given URL
    pub fn find_etag(&self, url: &Url) -> Option<&String> {
        self.old_etags.find(url.as_ref())
    }

    /// Add an etag for a list of URLs to the new etags collection
    pub async fn add_etags(&self, urls: Vec<&Url>, etag: &str) {
        let mut etags = self.new_etags.lock().await;

        for url in urls {
            etags.add(url.to_string(), etag.to_string());
            debug!(self, 2, "Set etag for {url} to {etag}")
        }

        drop(etags);
    }

    /// Acquire a download slot
    pub async fn acquire_slot(&self) -> Result<SemaphorePermit<'_>, Box<dyn Error + Send + Sync>> {
        Ok(self.conc_sem.acquire().await?)
    }

    /// Returns the starting URL
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Tests if a URL is relative to the base URL
    pub fn url_is_relative(&self, url: &Url) -> bool {
        url_relative_to(url, &self.url)
    }

    /// Build file relative path for a given URL
    pub fn path_for_url(&self, url: &Url) -> PathBuf {
        // Start with download directory
        let mut path = PathBuf::from(&self.args.target);

        // Get relative path of the URL from the base
        let rel = url_relative_path(url, &self.url).expect("URL should be relative");

        // Trim leading slashes from the relative path
        let rel = rel.trim_start_matches('/');

        if rel.is_empty() {
            // Not relative - use the unnamed file name
            path.push(&self.args.unnamed);
        } else {
            // Use relative path
            path.push(rel);
        }

        debug!(self, 2, "URL {url} maps to file {}", path.display());

        path
    }

    /// Get relative path to download directory
    pub fn download_relative_path<'a>(
        &self,
        path: &'a Path,
    ) -> Result<&'a Path, Box<dyn Error + Send + Sync>> {
        Ok(path.strip_prefix(&self.args.target)?)
    }

    /// Checks the passed URL can be handled
    pub fn check_url(url: &Url) -> Result<(), String> {
        // Check scheme
        match url.scheme() {
            "http" | "https" => (),
            _ => {
                Err(format!("{url} is not an http or https scheme"))?;
            }
        }

        Ok(())
    }

    /// Checks is a given path is in the skip list
    pub fn path_in_skip_list(&self, path: &Path) -> bool {
        if let Some(path) = path.to_str() {
            self.skip_list.find(path)
        } else {
            false
        }
    }

    /// Returns the debug level
    pub fn debug_level(&self) -> u8 {
        self.args.debug
    }

    /// Performs a debug delay
    pub async fn debug_delay(&self) {
        let delay = self.args.debug_delay;

        if delay > 0 {
            sleep(Duration::from_millis(delay)).await;
        }
    }

    fn create_http_client(args: &Args, url: Url) -> Result<Client, Box<dyn Error + Send + Sync>> {
        // Create redirect policy
        let redirect_policy = Policy::custom(move |attempt| {
            let attempt_url = attempt.url();

            // Check no more that 10 redirects and that path partially matches
            if attempt.previous().len() <= 10 && url_relative_to(attempt_url, &url) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        });

        // Create HTTP client
        Ok(Client::builder()
            .redirect(redirect_policy)
            .connect_timeout(Duration::from_secs(args.connect_timeout))
            .timeout(Duration::from_secs(args.fetch_timeout))
            .build()?)
    }
}

pub type ArcState = Arc<State>;
