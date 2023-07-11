use url::Position;
pub use url::Url;

pub trait UrlExt {
    /// Returns true if test URL is relative to a base URL
    fn is_relative_to(&self, base_url: &Url) -> bool;

    /// Returns the relative path for a URL from a base URL
    fn relative_path<'a>(&'a self, base_url: &Url) -> Option<&'a str>;

    /// Returns the full path of the URL including query and hash strings
    fn full_path(&self) -> &str;
}

impl UrlExt for Url {
    fn is_relative_to(&self, base_url: &Url) -> bool {
        self.host_str() == base_url.host_str() && self.full_path().starts_with(base_url.full_path())
    }

    fn relative_path<'a>(&'a self, base_url: &Url) -> Option<&'a str> {
        if self.is_relative_to(base_url) {
            let chop_pos = base_url[Position::BeforePath..].len();
            Some(&self.full_path()[chop_pos..])
        } else {
            None
        }
    }

    fn full_path(&self) -> &str {
        &self[Position::BeforePath..]
    }
}
