/// DSK format specifications and constants

/// Format constants
pub mod constants;
/// Format specification types
pub mod spec;

pub use constants::*;
pub use spec::{FormatSpec, SideMode};

/// DSK format type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DskFormat {
    /// Standard DSK format with fixed track sizes
    Standard,
    /// Extended DSK format with variable track sizes
    Extended,
}

impl DskFormat {
    /// Get the magic bytes for this format
    pub fn magic_bytes(&self) -> &'static [u8] {
        match self {
            DskFormat::Standard => STANDARD_DSK_SIGNATURE,
            DskFormat::Extended => EXTENDED_DSK_SIGNATURE,
        }
    }

    /// Get a human-readable name for this format
    pub fn name(&self) -> &'static str {
        match self {
            DskFormat::Standard => "Standard DSK",
            DskFormat::Extended => "Extended DSK",
        }
    }
}

/// Detect DSK format from magic bytes
pub fn detect_format(magic: &[u8]) -> Option<DskFormat> {
    if magic.len() < 8 {
        return None;
    }

    if magic.starts_with(b"EXTENDED") {
        Some(DskFormat::Extended)
    } else if magic.starts_with(b"MV - CPC") {
        Some(DskFormat::Standard)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_standard_format() {
        let result = detect_format(STANDARD_DSK_SIGNATURE);
        assert_eq!(result, Some(DskFormat::Standard));
    }

    #[test]
    fn test_detect_extended_format() {
        let result = detect_format(EXTENDED_DSK_SIGNATURE);
        assert_eq!(result, Some(DskFormat::Extended));
    }

    #[test]
    fn test_detect_invalid_format() {
        let result = detect_format(b"INVALID DATA");
        assert_eq!(result, None);
    }

    #[test]
    fn test_format_magic_bytes() {
        assert_eq!(
            DskFormat::Standard.magic_bytes(),
            STANDARD_DSK_SIGNATURE
        );
        assert_eq!(
            DskFormat::Extended.magic_bytes(),
            EXTENDED_DSK_SIGNATURE
        );
    }
}
