pub use mime::Mime;

/// Extension trait for Mime structure
pub trait MimeExt {
    /// Returns true if MIME types are equal
    fn equal(&self, other: &Mime) -> bool;
}

impl MimeExt for Mime {
    /// Tests if this MIME type has equal type and subtype to another MIME type
    fn equal(&self, other: &Mime) -> bool {
        self.type_() == other.type_() && self.subtype() == other.subtype()
    }
}
