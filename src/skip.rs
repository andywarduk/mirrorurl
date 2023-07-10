use std::error::Error;
use std::fs::File;
use std::io::BufReader;

#[derive(Default)]
pub struct SkipList {
    list: Vec<String>,
}

impl SkipList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_from_file(file: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let fh =
            File::open(file).map_err(|e| format!("Failed to open skip list file {file}: {e}"))?;

        let reader = BufReader::new(fh);

        let list = serde_json::from_reader(reader)
            .map_err(|e| format!("Failed to load skip list file {file}: {e}"))?;

        Ok(Self { list })
    }

    pub fn find(&self, string: &str) -> bool {
        for s in &self.list {
            if string.starts_with(s) {
                return true;
            }
        }

        false
    }
}
