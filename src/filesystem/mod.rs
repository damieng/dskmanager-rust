/// Filesystem implementations

pub mod cpm;

pub use cpm::CpmFileSystem;

use crate::error::Result;
use crate::image::DskImage;

/// File attributes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAttributes {
    /// Read-only flag
    pub read_only: bool,
    /// System file flag
    pub system: bool,
    /// Archive flag
    pub archive: bool,
}

impl Default for FileAttributes {
    fn default() -> Self {
        Self {
            read_only: false,
            system: false,
            archive: false,
        }
    }
}

/// Directory entry
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Filename (8.3 format, e.g., "FILENAME.TXT")
    pub name: String,
    /// User number (0-15)
    pub user: u8,
    /// Extent number
    pub extent: u8,
    /// File size in bytes
    pub size: usize,
    /// File attributes
    pub attributes: FileAttributes,
}

/// Filesystem information
#[derive(Debug)]
pub struct FileSystemInfo {
    /// Filesystem type name
    pub fs_type: String,
    /// Total blocks on disk
    pub total_blocks: usize,
    /// Free blocks
    pub free_blocks: usize,
    /// Block size in bytes
    pub block_size: usize,
}

/// Filesystem trait for accessing files on DSK images
pub trait FileSystem {
    /// Attempt to mount a filesystem from a DSK image (read-only)
    fn from_image<'a>(image: &'a DskImage) -> Result<Self> where Self: Sized;

    /// Attempt to mount a filesystem from a DSK image (read-write)
    fn from_image_mut<'a>(image: &'a mut DskImage) -> Result<Self> where Self: Sized;

    /// List directory entries
    fn read_dir(&self) -> Result<Vec<DirEntry>>;

    /// Read a file's contents
    fn read_file(&self, name: &str) -> Result<Vec<u8>>;

    /// Write a file (requires mutable filesystem)
    fn write_file(&mut self, name: &str, data: &[u8]) -> Result<()>;

    /// Delete a file (requires mutable filesystem)
    fn delete_file(&mut self, name: &str) -> Result<()>;

    /// Get filesystem information
    fn info(&self) -> FileSystemInfo;
}
