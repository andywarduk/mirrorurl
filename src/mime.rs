pub use mime::Mime;

pub trait MimeExt {
    /// Returns true if MIME types are equal
    fn equal(&self, other: &Mime) -> bool;
}

impl MimeExt for Mime {
    fn equal(&self, other: &Mime) -> bool {
        self.type_() == other.type_() && self.subtype() == other.subtype()
    }
}
