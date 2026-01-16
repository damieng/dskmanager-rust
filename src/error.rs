use thiserror::Error;

/// Result type alias for DSK operations
pub type Result<T> = std::result::Result<T, DskError>;

/// Errors that can occur when working with DSK files
#[derive(Debug, Error)]
pub enum DskError {
    /// I/O error occurred while reading or writing
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid or unrecognized DSK file format
    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    /// Invalid track number specified
    #[error("Invalid track {track} on side {side} (max: {max})")]
    InvalidTrack {
        /// Side number
        side: u8,
        /// Track number
        track: u8,
        /// Maximum allowed track number
        max: u8,
    },

    /// Invalid sector ID specified
    #[error("Invalid sector: id={id} on track {track}, side {side}")]
    InvalidSector {
        /// Side number
        side: u8,
        /// Track number
        track: u8,
        /// Sector ID
        id: u8,
    },

    /// Unsupported format variant
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    /// Parse error at specific offset
    #[error("Parse error at offset {offset}: {message}")]
    ParseError {
        /// Byte offset where error occurred
        offset: usize,
        /// Error message
        message: String,
    },

    /// Filesystem-related error
    #[error("Filesystem error: {0}")]
    FileSystemError(String),

    /// Data integrity error
    #[error("Data integrity error: {0}")]
    IntegrityError(String),

    /// File not found in filesystem
    #[error("File not found: {0}")]
    FileNotFound(String),

    /// Disk is full, no free space
    #[error("Disk full: no free space available")]
    DiskFull,

    /// Invalid filename
    #[error("Invalid filename: {0}")]
    InvalidFilename(String),
}

impl DskError {
    /// Create a parse error with context
    pub fn parse<S: Into<String>>(offset: usize, message: S) -> Self {
        DskError::ParseError {
            offset,
            message: message.into(),
        }
    }

    /// Create an invalid format error
    pub fn invalid_format<S: Into<String>>(message: S) -> Self {
        DskError::InvalidFormat(message.into())
    }

    /// Create a filesystem error
    pub fn filesystem<S: Into<String>>(message: S) -> Self {
        DskError::FileSystemError(message.into())
    }

    /// Create an integrity error
    pub fn integrity<S: Into<String>>(message: S) -> Self {
        DskError::IntegrityError(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = DskError::InvalidTrack {
            side: 0,
            track: 50,
            max: 39,
        };
        assert_eq!(
            err.to_string(),
            "Invalid track 50 on side 0 (max: 39)"
        );
    }

    #[test]
    fn test_parse_error() {
        let err = DskError::parse(256, "Invalid magic bytes");
        assert_eq!(
            err.to_string(),
            "Parse error at offset 256: Invalid magic bytes"
        );
    }
}
