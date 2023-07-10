use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter};

#[derive(Default)]
pub struct ETags {
    etags: HashMap<String, String>,
}

impl ETags {
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

    pub fn save_to_file(&self, file: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let fh = File::create(file)?;

        let writer = BufWriter::new(fh);

        serde_json::to_writer_pretty(writer, &self.etags)?;

        Ok(())
    }

    pub fn find(&self, key: &str) -> Option<&String> {
        self.etags.get(key)
    }

    pub fn add(&mut self, key: String, value: String) {
        self.etags.insert(key, value);
    }

    pub fn extend(&mut self, other: &ETags) -> &Self {
        self.etags
            .extend(other.etags.iter().map(|(k, v)| (k.clone(), v.clone())));

        self
    }
}
