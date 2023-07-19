use std::cmp::{max, min};
use std::error::Error;

use clap::Parser;

use crate::output::output;

#[derive(Parser, Clone, Debug)]
#[clap(author, version, about)]
pub struct Args {
    /// URL to mirror
    pub url: String,

    /// Target directory
    pub target: String,

    /// Maximum number of concurrent requests to the web server
    #[clap(short = 'c', long = "concurrent", default_value_t = default_concurrent_requests(), value_parser = clamp_concurrent)]
    pub concurrent_fetch: usize,

    /// Maximum number of worker threads to run
    #[clap(short = 't', long = "threads", default_value_t = default_threads(), value_parser = clamp_threads)]
    pub threads: usize,

    /// File name to use for unnamed files
    #[clap(short = 'u', long = "unnamed", default_value_t = default_unnamed())]
    pub unnamed: String,

    /// Connection timout in seconds
    #[clap(long = "connect-timeout", default_value_t = default_connect_timeout())]
    pub connect_timeout: u64,

    /// Fetch timout in minutes
    #[clap(long = "fetch-timeout", default_value_t = default_fetch_timeout())]
    pub fetch_timeout: u64,

    /// Skip list file (JSON array file containing URLs or relative file paths to skip)
    #[clap(short = 's', long = "skip-file")]
    pub skip_file: Option<String>,

    /// Don't use etags to detect out of date files
    #[clap(short = 'e', long = "no-etags")]
    pub no_etags: bool,

    /// Increase debug message level
    #[clap(short = 'd', long = "debug", action = clap::ArgAction::Count)]
    pub debug: u8,

    /// Insert an artificial delay in the data fetch for debugging
    #[clap(long = "debug-delay", default_value_t = 0)]
    pub debug_delay: u64,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            url: Default::default(),
            target: Default::default(),
            concurrent_fetch: default_concurrent_requests(),
            threads: default_threads(),
            unnamed: default_unnamed(),
            connect_timeout: default_connect_timeout(),
            fetch_timeout: default_fetch_timeout(),
            skip_file: Default::default(),
            no_etags: Default::default(),
            debug: Default::default(),
            debug_delay: Default::default(),
        }
    }
}

impl Args {
    pub fn parse() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let args = Args::try_parse()?;

        Ok(args)
    }
}

fn default_concurrent_requests() -> usize {
    10
}

fn default_threads() -> usize {
    min(default_concurrent_requests(), num_cpus::get())
}

fn default_unnamed() -> String {
    String::from("__file.dat")
}

fn default_connect_timeout() -> u64 {
    60
}

fn default_fetch_timeout() -> u64 {
    5
}

fn clamp_concurrent(s: &str) -> Result<usize, String> {
    Ok(max(
        1,
        s.parse().map_err(|_| format!("'{s}' is not a number"))?,
    ))
}

fn clamp_threads(s: &str) -> Result<usize, String> {
    let rq_threads: usize = s.parse().map_err(|_| format!("'{s}' is not a number"))?;
    let mut act_threads = rq_threads;
    let cpus = num_cpus::get();

    if rq_threads < 1 {
        act_threads = 1;
    } else if rq_threads > cpus {
        act_threads = cpus;
        output!("Warning: Clamping number of threads to {cpus} due to cpu count")
    }

    Ok(act_threads)
}
