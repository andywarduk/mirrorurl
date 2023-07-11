use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;

use reqwest::redirect::Policy;
use reqwest::Client;
use tokio::sync::{Mutex, Semaphore, SemaphorePermit};
use tokio::time::{sleep, Duration};

use crate::args::Args;
use crate::etags::ETags;
use crate::output::debug;
use crate::skip::SkipList;
use crate::url::{Url, UrlExt};

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
        url.is_handled()?;

        // Create HTTP client
        let client = Self::create_http_client(&args, url.clone())?;

        // Build etags file path
        let mut etags_file = PathBuf::from(&args.target);
        etags_file.push(".etags.json");
        let etags_file = etags_file
            .to_str()
            .ok_or("Unable to build path to .etags")?;

        let etags = if args.no_etags {
            ETags::default()
        } else {
            // Load etags if present
            ETags::new_from_file(etags_file)?
        };

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

    /// Returns a reference to the starting URL
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Returns a reference to the HTTP client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Adds a URL to the processed list. Returns false if URL alredy seen
    pub async fn add_processed_url(&self, url: Url) -> bool {
        self.processed_urls.lock().await.insert(url)
    }

    /// Acquire a download slot
    pub async fn acquire_slot(&self) -> Result<SemaphorePermit<'_>, Box<dyn Error + Send + Sync>> {
        Ok(self.conc_sem.acquire().await?)
    }

    /// Build file relative path for a given URL
    pub fn path_for_url(&self, url: &Url) -> Result<PathBuf, String> {
        // Start with download directory
        let mut path = PathBuf::from(&self.args.target);

        // Get relative path of the URL from the base
        let rel = url
            .relative_path(&self.url)
            .ok_or("URL is not relative to the base URL".to_string())?;

        if rel.is_empty() {
            // Not relative - use the unnamed file name
            path.push(&self.args.unnamed);
        } else {
            // Is it in the skip list?
            if self.skip_list.find(rel) {
                Err("Path is in the skip list")?
            }

            // Use relative path
            path.push(rel);
        }

        debug!(self, 2, "URL {url} maps to file {}", path.display());

        Ok(path)
    }

    /// Looks for an etag in the etag list for a given URL
    pub fn find_etag(&self, url: &Url) -> Option<&String> {
        self.old_etags.find(url.as_ref())
    }

    /// Add an etag for a list of URLs to the new etags collection
    pub async fn add_etags(&self, urls: Vec<&Url>, etag: &str) {
        let mut new_etags = self.new_etags.lock().await;

        for url in urls {
            new_etags.add(url.to_string(), etag.to_string());
            debug!(self, 2, "Set etag for {url} to {etag}")
        }

        drop(new_etags);
    }

    /// Save the etags file
    pub async fn save_etags(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if !self.args.no_etags {
            // Save etags
            self.new_etags
                .lock()
                .await
                .extend(&self.old_etags)
                .save_to_file(&self.etags_file)?
        }

        Ok(())
    }

    /// Returns the debug level
    #[inline]
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

            // Check no more that 10 redirects and that path is relative to the base URL
            if attempt.previous().len() <= 10 && attempt_url.is_relative_to(&url) {
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
