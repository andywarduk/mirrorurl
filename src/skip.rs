use std::error::Error;
use std::fs::File;
use std::io::BufReader;

/// Holds a list for partial file paths to skip downloading
#[derive(Default)]
pub struct SkipList {
    list: Vec<String>,
}

impl SkipList {
    /// Creates a new empty list
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads a skip list from a JSON file
    pub fn new_from_file(file: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let fh =
            File::open(file).map_err(|e| format!("Failed to open skip list file {file}: {e}"))?;

        let reader = BufReader::new(fh);

        let list = serde_json::from_reader(reader)
            .map_err(|e| format!("Failed to load skip list file {file}: {e}"))?;

        Ok(Self { list })
    }

    /// Returns true if the relative file path matches an item in the skip lists
    pub fn find(&self, rel_path: &str) -> bool {
        for s in &self.list {
            if rel_path.starts_with(s) {
                return true;
            }
        }

        false
    }
}
