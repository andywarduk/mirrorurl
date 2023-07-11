use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter};

/// Map of URLs to etags
#[derive(Default)]
pub struct ETags {
    etags: HashMap<String, String>,
}

impl ETags {
    /// Load mapping from a JSON file. If the file does not exist, create an empty list
    pub fn new_from_file(file: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let etags = match File::open(file) {
            Ok(fh) => {
                let reader = BufReader::new(fh);

                let map = serde_json::from_reader(reader)
                    .map_err(|e| format!("Failed to load etags file {file}: {e}"))?;

                Self { etags: map }
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => ETags::default(),
                _ => Err(format!("Failed to open etags {file}: {e}"))?,
            },
        };

        Ok(etags)
    }

    /// Save mapping to a JSON file
    pub fn save_to_file(&self, file: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let fh = File::create(file).map_err(|e| format!("Error creating {file}: {e}"))?;

        let writer = BufWriter::new(fh);

        serde_json::to_writer_pretty(writer, &self.etags)
            .map_err(|e| format!("Error writing {file}: {e}"))?;

        Ok(())
    }

    /// Looks for a URL in the mapping and returns the etag if present
    pub fn find(&self, key: &str) -> Option<&String> {
        self.etags.get(key)
    }

    /// Adds a URL to etag mapping
    pub fn add(&mut self, url: String, etag: String) {
        self.etags.insert(url, etag);
    }

    /// Extends the map with another map
    pub fn extend(&mut self, other: &ETags) -> &Self {
        self.etags.extend(
            other
                .etags
                .iter()
                .map(|(url, etag)| (url.clone(), etag.clone())),
        );

        self
    }
}
