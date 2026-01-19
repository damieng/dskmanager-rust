/// DISCiPLE/+D filesystem implementation
///
/// ZX Spectrum specific filesystem using MGT format.
/// Used by:
/// - Miles Gordon Technology DISCiPLE
/// - MGT +D
///
/// Directory entry extensions at offset 0xD2 (210):
/// - 10 bytes for Disciple/+D header info

use crate::error::Result;
use crate::filesystem::mgt::{MgtDirEntry, MgtFileSystem, MgtFileType};
use crate::filesystem::{ExtendedDirEntry, FileHeader, HeaderType};
use crate::image::DiskImage;

/// Disciple/+D specific file header info
#[derive(Debug, Clone)]
pub struct DiscipleHeader {
    /// File type description
    pub file_type: DiscipleFileType,
    /// Load address (for CODE files)
    pub load_address: u16,
    /// Length in bytes
    pub length: u16,
    /// Start address/line (for BASIC/CODE)
    pub start: u16,
}

/// Disciple/+D file types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscipleFileType {
    /// BASIC program
    Basic,
    /// Numeric array
    NumericArray,
    /// String array
    StringArray,
    /// CODE (binary)
    Code,
    /// 48K Snapshot
    Snapshot48k,
    /// Microdrive file
    Microdrive,
    /// SCREEN$
    Screen,
    /// Special file
    Special,
    /// 128K Snapshot
    Snapshot128k,
    /// Opentype file
    Opentype,
    /// Execute file
    Execute,
    /// Unknown type
    Unknown(u8),
}

impl DiscipleFileType {
    /// Parse from MGT file type
    pub fn from_mgt_type(mgt_type: &MgtFileType, raw_data: &[u8]) -> Self {
        match mgt_type {
            MgtFileType::ZxSnapshot => {
                // Check if it's 48K or 128K based on size or header
                if raw_data.len() >= 215 && raw_data[214] != 0 {
                    DiscipleFileType::Snapshot128k
                } else {
                    DiscipleFileType::Snapshot48k
                }
            }
            MgtFileType::Code => DiscipleFileType::Code,
            MgtFileType::Screen => DiscipleFileType::Screen,
            MgtFileType::SamBasic => DiscipleFileType::Basic,
            MgtFileType::NumericArray => DiscipleFileType::NumericArray,
            MgtFileType::StringArray => DiscipleFileType::StringArray,
            MgtFileType::Other(code) => match code {
                1 => DiscipleFileType::Basic,
                2 => DiscipleFileType::NumericArray,
                3 => DiscipleFileType::StringArray,
                4 => DiscipleFileType::Code,
                5 => DiscipleFileType::Snapshot48k,
                6 => DiscipleFileType::Microdrive,
                7 => DiscipleFileType::Screen,
                8 => DiscipleFileType::Special,
                9 => DiscipleFileType::Snapshot128k,
                10 => DiscipleFileType::Opentype,
                11 => DiscipleFileType::Execute,
                _ => DiscipleFileType::Unknown(*code),
            },
            _ => DiscipleFileType::Unknown(mgt_type.type_code()),
        }
    }
}

impl std::fmt::Display for DiscipleFileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscipleFileType::Basic => write!(f, "BASIC"),
            DiscipleFileType::NumericArray => write!(f, "Number Array"),
            DiscipleFileType::StringArray => write!(f, "String Array"),
            DiscipleFileType::Code => write!(f, "CODE"),
            DiscipleFileType::Snapshot48k => write!(f, "48K Snapshot"),
            DiscipleFileType::Microdrive => write!(f, "Microdrive"),
            DiscipleFileType::Screen => write!(f, "SCREEN$"),
            DiscipleFileType::Special => write!(f, "Special"),
            DiscipleFileType::Snapshot128k => write!(f, "128K Snapshot"),
            DiscipleFileType::Opentype => write!(f, "Opentype"),
            DiscipleFileType::Execute => write!(f, "Execute"),
            DiscipleFileType::Unknown(code) => write!(f, "Type {}", code),
        }
    }
}

/// DISCiPLE/+D filesystem
pub struct DiscipleFileSystem<'a> {
    mgt: MgtFileSystem<'a>,
}

impl<'a> DiscipleFileSystem<'a> {
    /// Create a new Disciple filesystem from an image
    pub fn new(image: &'a DiskImage) -> Result<Self> {
        let mgt = MgtFileSystem::new(image)?;
        Ok(Self { mgt })
    }

    /// Get the underlying MGT filesystem
    pub fn mgt(&self) -> &MgtFileSystem<'a> {
        &self.mgt
    }

    /// Read directory with Disciple-specific information
    pub fn read_dir_extended(&self) -> Result<Vec<ExtendedDirEntry>> {
        let mut entries = Vec::new();

        for dir_entry in self.mgt.directory() {
            let header = self.parse_disciple_header(dir_entry);

            entries.push(ExtendedDirEntry {
                name: dir_entry.filename.clone(),
                user: 0,
                index: dir_entry.index,
                blocks: dir_entry.sectors_used as usize,
                allocated: dir_entry.file_size(),
                size: dir_entry.file_size(),
                attributes: dir_entry.attributes(),
                header,
            });
        }

        Ok(entries)
    }

    /// Parse Disciple-specific header from directory entry
    fn parse_disciple_header(&self, entry: &MgtDirEntry) -> FileHeader {
        let raw = &entry.raw_data;
        let file_type = DiscipleFileType::from_mgt_type(&entry.file_type, raw);

        // Extract header info from offset 0xD2 (210)
        let meta = if raw.len() >= 220 {
            let header_offset = 210;

            match file_type {
                DiscipleFileType::Code | DiscipleFileType::Screen => {
                    // Load address at offset 211-212 (little endian)
                    let load_addr = if raw.len() >= header_offset + 3 {
                        u16::from_le_bytes([raw[header_offset + 1], raw[header_offset + 2]])
                    } else {
                        0
                    };
                    // Length at offset 213-214
                    let length = if raw.len() >= header_offset + 5 {
                        u16::from_le_bytes([raw[header_offset + 3], raw[header_offset + 4]])
                    } else {
                        0
                    };
                    format!("{} {},{}",file_type, load_addr, length)
                }
                DiscipleFileType::Basic => {
                    // Auto-start line at offset 217-218
                    let auto_line = if raw.len() >= header_offset + 9 {
                        u16::from_le_bytes([raw[header_offset + 7], raw[header_offset + 8]])
                    } else {
                        0
                    };
                    if auto_line > 0 && auto_line < 10000 {
                        format!("{} LINE {}", file_type, auto_line)
                    } else {
                        format!("{}", file_type)
                    }
                }
                DiscipleFileType::Snapshot48k | DiscipleFileType::Snapshot128k => {
                    format!("{}", file_type)
                }
                _ => format!("{}", file_type),
            }
        } else {
            format!("{}", file_type)
        };

        FileHeader {
            header_type: HeaderType::None, // Disciple has its own header format
            checksum_valid: true,
            file_size: entry.file_size(),
            header_size: 0,
            meta,
        }
    }

    /// Read a file by name
    pub fn read_file(&self, name: &str) -> Result<Vec<u8>> {
        let entry = self
            .mgt
            .find_file(name)
            .ok_or_else(|| crate::error::DskError::FileNotFound(name.to_string()))?;
        self.mgt.read_file(entry)
    }

    /// List all files
    pub fn list_files(&self) -> Vec<&MgtDirEntry> {
        self.mgt.directory().iter().collect()
    }

    /// Get filesystem info
    pub fn info(&self) -> String {
        let mgt_info = self.mgt.info();
        format!(
            "DISCiPLE/+D Filesystem\n  Files: {}\n  Used: {} KB\n  Free: {} KB",
            mgt_info.file_count,
            mgt_info.used_sectors / 2,
            mgt_info.free_sectors / 2
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disciple_file_type_display() {
        assert_eq!(format!("{}", DiscipleFileType::Basic), "BASIC");
        assert_eq!(format!("{}", DiscipleFileType::Code), "CODE");
        assert_eq!(format!("{}", DiscipleFileType::Snapshot48k), "48K Snapshot");
        assert_eq!(format!("{}", DiscipleFileType::Screen), "SCREEN$");
    }
}
