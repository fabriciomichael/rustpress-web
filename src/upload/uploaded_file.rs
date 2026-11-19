use std::path::{Path, PathBuf};

/// Represents a file received through a multipart request.
#[derive(Debug, Clone)]
pub struct UploadedFile {
    field_name: String,
    original_name: Option<String>,
    stored_name: String,
    path: PathBuf,
    content_type: Option<String>,
    size: u64,
}

impl UploadedFile {
    /// Creates a representation of a received file.
    pub(crate) fn new(
        field_name: String,
        original_name: Option<String>,
        stored_name: String,
        path: PathBuf,
        content_type: Option<String>,
        size: u64,
    ) -> Self {
        Self {
            field_name,
            original_name,
            stored_name,
            path,
            content_type,
            size,
        }
    }

    /// Returns the name of the form field that sent the file.
    pub fn field_name(&self) -> &str {
        &self.field_name
    }

    /// Returns the original name provided by the client.
    pub fn original_name(&self) -> Option<&str> {
        self.original_name.as_deref()
    }

    /// Returns the name used for storage.
    pub fn stored_name(&self) -> &str {
        &self.stored_name
    }

    /// Returns the path of the stored file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the MIME type declared by the client.
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Returns the file size in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }
}

#[cfg(test)]
mod tests {
    use super::UploadedFile;
    use std::path::Path;

    #[test]
    fn exposes_file_metadata() {
        let file = UploadedFile::new(
            "avatar".to_owned(),
            Some("foto.png".to_owned()),
            "550e8400-e29b-41d4-a716-446655440000.png".to_owned(),
            Path::new("storage/uploads/550e8400-e29b-41d4-a716-446655440000.png").to_path_buf(),
            Some("image/png".to_owned()),
            4096,
        );

        assert_eq!(file.field_name(), "avatar");
        assert_eq!(file.original_name(), Some("foto.png"));
        assert_eq!(
            file.stored_name(),
            "550e8400-e29b-41d4-a716-446655440000.png"
        );
        assert_eq!(
            file.path(),
            Path::new("storage/uploads/550e8400-e29b-41d4-a716-446655440000.png")
        );
        assert_eq!(file.content_type(), Some("image/png"));
        assert_eq!(file.size(), 4096);
    }

    #[test]
    fn supports_missing_optional_metadata() {
        let file = UploadedFile::new(
            "documento".to_owned(),
            None,
            "arquivo".to_owned(),
            Path::new("storage/uploads/arquivo").to_path_buf(),
            None,
            0,
        );

        assert_eq!(file.field_name(), "documento");
        assert_eq!(file.original_name(), None);
        assert_eq!(file.content_type(), None);
        assert_eq!(file.size(), 0);
    }
}
