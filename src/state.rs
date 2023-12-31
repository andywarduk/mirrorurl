use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;

use reqwest::redirect::Policy;
use reqwest::Client;
use tokio::sync::{Mutex, MutexGuard, OwnedSemaphorePermit, Semaphore};
use tokio::time::{sleep, Duration};

use crate::args::Args;
use crate::etags::ETags;
use crate::output::debug;
use crate::skip::SkipList;
use crate::skipreason::{SkipReason, SkipReasonErr};
use crate::stats::Stats;
use crate::url::{Url, UrlExt};

/// Program state shared between all threads
pub struct State {
    /// Base URL
    url: Url,
    /// Set of processed URLs
    processed_urls: Mutex<HashSet<Url>>,
    /// Etags file path as a string
    etags_file: String,
    /// Old etags collection (loaded at startup)
    old_etags: ETags,
    /// New etags collection (added to whilst running)
    new_etags: Mutex<ETags>,
    /// File skip list
    skip_list: SkipList,
    /// Concurrect fetch semaphore
    conc_sem: Arc<Semaphore>,
    /// HTTP client
    client: Client,
    /// Command line arguments
    args: Args,
    /// Statistics
    stats: Mutex<Stats>,
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
            conc_sem: Arc::new(Semaphore::new(args.concurrent_fetch)),
            client,
            args,
            stats: Mutex::new(Stats::default()),
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
    pub async fn acquire_slot(&self) -> Result<OwnedSemaphorePermit, Box<dyn Error + Send + Sync>> {
        Ok(self.conc_sem.clone().acquire_owned().await?)
    }

    /// Build file relative path for a given URL
    pub async fn path_for_url(&self, url: &Url) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
        // Start with download directory
        let mut path = PathBuf::from(&self.args.target);

        // Get relative path of the URL from the base
        let rel = match url.relative_path(&self.url) {
            Some(rel) => rel,
            None => Err(SkipReasonErr::new(url.to_string(), SkipReason::NotRelative))?,
        };

        if rel.is_empty() {
            // Not relative - use the unnamed file name
            path.push(&self.args.unnamed);
        } else {
            // Is it in the skip list?
            if self.skip_list.find(rel) {
                Err(SkipReasonErr::new(url.to_string(), SkipReason::SkipList))?
            }

            // Use relative path
            path.push(rel);
        }

        debug!(self, 2, "URL {url} maps to file {}", path.display());

        Ok(path)
    }

    /// Update stats
    pub async fn update_stats<'a, F>(&'a self, update_fn: F)
    where
        F: FnOnce(MutexGuard<'a, Stats>),
    {
        let stats_lock = self.stats.lock().await;

        update_fn(stats_lock);
    }

    /// Gets a copy of the stats
    pub async fn get_stats(&self) -> Stats {
        self.stats.lock().await.clone()
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
            let new_etags = &mut self.new_etags.lock().await;

            if !new_etags.is_empty() {
                // Merge old etags in to new etags and save to file
                new_etags
                    .extend(&self.old_etags)
                    .save_to_file(&self.etags_file)?
            }
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

    /// Creates the HTTP client
    fn create_http_client(args: &Args, url: Url) -> Result<Client, Box<dyn Error + Send + Sync>> {
        // Create redirect policy
        let max_redirects = args.max_redirects;

        let redirect_policy = Policy::custom(move |attempt| {
            // Check no more that 10 redirects and that path is relative to the base URL
            if attempt.previous().len() > max_redirects {
                let initial = attempt.previous()[0].clone();

                attempt.error(SkipReasonErr::new(
                    initial.to_string(),
                    SkipReason::TooManyRedirects,
                ))
            } else {
                let attempt_url = attempt.url();

                if !attempt_url.is_relative_to(&url) {
                    let initial = attempt.previous()[0].clone();
                    let attempt_url = attempt.url().clone();

                    attempt.error(SkipReasonErr::new(
                        initial.to_string(),
                        SkipReason::RedirectNotRel(attempt_url.to_string()),
                    ))
                } else {
                    attempt.follow()
                }
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
