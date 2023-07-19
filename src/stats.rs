use num::PrimInt;

use crate::output::output;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Stats {
    downloads: u64,
    download_bytes: usize,
    html_docs: u64,
    html_bytes: usize,
    not_modified: u64,
    skipped: u64,
    errored: u64,
}

impl Stats {
    pub fn add_download(&mut self, bytes: usize) {
        self.downloads += 1;
        self.download_bytes += bytes;
    }

    pub fn add_html(&mut self, bytes: usize) {
        self.html_docs += 1;
        self.html_bytes += bytes;
    }

    pub fn add_skipped(&mut self) {
        self.skipped += 1;
    }

    pub fn add_not_modified(&mut self) {
        self.not_modified += 1;
    }

    pub fn add_errored(&mut self) {
        self.errored += 1;
    }

    pub fn print(&self) {
        output!(
            "{} parsed ({})",
            Self::format_qty(self.html_docs, "document", "documents"),
            Self::format_qty(self.html_bytes, "byte", "bytes"),
        );
        output!(
            "{} downloaded ({}), {} not modified, {} skipped, {} errored",
            Self::format_qty(self.downloads, "file", "files"),
            Self::format_qty(self.download_bytes, "byte", "bytes"),
            self.not_modified,
            self.skipped,
            self.errored
        );
    }

    fn format_qty<T>(qty: T, single: &str, plural: &str) -> String
    where
        T: PrimInt + std::fmt::Display,
    {
        if qty.is_one() {
            format!("{} {}", qty, single)
        } else {
            format!("{} {}", qty, plural)
        }
    }
}
