/// CP/M filesystem implementation

use crate::error::{DskError, Result};
use crate::filesystem::{DirEntry, FileAttributes, FileSystem, FileSystemInfo};
use crate::image::DskImage;
use std::collections::HashMap;

/// CP/M Disk Parameter Block
#[derive(Debug, Clone)]
pub struct DiskParameterBlock {
    /// Sectors per track
    pub sectors_per_track: u16,
    /// Block shift (3=1024, 4=2048, 5=4096)
    pub block_shift: u8,
    /// Block mask
    pub block_mask: u8,
    /// Extent mask
    pub extent_mask: u8,
    /// Maximum allocation blocks (DSM)
    pub max_blocks: u16,
    /// Maximum directory entries (DRM)
    pub max_dir_entries: u16,
    /// Number of reserved tracks
    pub reserved_tracks: u8,
}

impl DiskParameterBlock {
    /// Get the block size in bytes
    pub fn block_size(&self) -> usize {
        128 << self.block_shift
    }

    /// CP/M format for Amstrad CPC Data format
    pub fn amstrad_data() -> Self {
        Self {
            sectors_per_track: 9,
            block_shift: 3,      // 1024 bytes
            block_mask: 7,
            extent_mask: 0,
            max_blocks: 179,     // 180 KB / 1024
            max_dir_entries: 63,
            reserved_tracks: 0,
        }
    }

    /// CP/M format for Spectrum +3
    pub fn spectrum_plus3() -> Self {
        Self {
            sectors_per_track: 9,
            block_shift: 3,      // 1024 bytes
            block_mask: 7,
            extent_mask: 0,
            max_blocks: 179,
            max_dir_entries: 63,
            reserved_tracks: 1,  // Boot track
        }
    }
}

/// CP/M directory entry (32 bytes)
#[derive(Debug, Clone)]
struct CpmDirEntry {
    user: u8,
    filename: [u8; 8],
    extension: [u8; 3],
    extent_low: u8,
    #[allow(dead_code)]
    reserved1: u8,
    extent_high: u8,
    record_count: u8,
    allocation: Vec<u8>,
}

impl CpmDirEntry {
    /// Parse a directory entry from 32 bytes
    fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 32 {
            return None;
        }

        let user = data[0];

        // Skip deleted entries (0xE5)
        if user == 0xE5 {
            return None;
        }

        let mut filename = [0u8; 8];
        let mut extension = [0u8; 3];
        filename.copy_from_slice(&data[1..9]);
        extension.copy_from_slice(&data[9..12]);

        let extent_low = data[12];
        let reserved1 = data[13];
        let extent_high = data[14];
        let record_count = data[15];

        let allocation = data[16..32].to_vec();

        Some(Self {
            user,
            filename,
            extension,
            extent_low,
            reserved1,
            extent_high,
            record_count,
            allocation,
        })
    }

    /// Get the full filename as a string
    fn filename_str(&self) -> String {
        let name = String::from_utf8_lossy(&self.filename)
            .trim_end()
            .replace('\0', " ")
            .trim()
            .to_string();
        let ext = String::from_utf8_lossy(&self.extension)
            .trim_end()
            .replace('\0', " ")
            .trim()
            .to_string();

        if ext.is_empty() {
            name
        } else {
            format!("{}.{}", name, ext)
        }
    }

    /// Check if read-only attribute is set
    fn is_read_only(&self) -> bool {
        (self.filename[0] & 0x80) != 0
    }

    /// Check if system attribute is set
    fn is_system(&self) -> bool {
        (self.filename[1] & 0x80) != 0
    }

    /// Check if archive attribute is set
    fn is_archive(&self) -> bool {
        (self.extension[2] & 0x80) != 0
    }

    /// Get file attributes
    fn attributes(&self) -> FileAttributes {
        FileAttributes {
            read_only: self.is_read_only(),
            system: self.is_system(),
            archive: self.is_archive(),
        }
    }

    /// Get the extent number
    fn extent_number(&self) -> u16 {
        ((self.extent_high as u16) << 8) | (self.extent_low as u16)
    }
}

/// CP/M filesystem implementation
pub struct CpmFileSystem<'a> {
    image: &'a DskImage,
    dpb: DiskParameterBlock,
    directory_entries: Vec<CpmDirEntry>,
}

impl<'a> CpmFileSystem<'a> {
    /// Create a new CP/M filesystem from an image
    pub fn new(image: &'a DskImage, dpb: DiskParameterBlock) -> Result<Self> {
        let directory_entries = Self::read_directory(image, &dpb)?;

        Ok(Self {
            image,
            dpb,
            directory_entries,
        })
    }

    /// Read the directory entries from the disk
    fn read_directory(image: &DskImage, dpb: &DiskParameterBlock) -> Result<Vec<CpmDirEntry>> {
        let mut entries = Vec::new();

        // Directory starts after reserved tracks
        let dir_track_start = dpb.reserved_tracks;

        // Calculate how many sectors are needed for directory
        let dir_size_bytes = (dpb.max_dir_entries as usize + 1) * 32;
        let sector_size = image.spec().sector_size as usize;
        let dir_sectors = (dir_size_bytes + sector_size - 1) / sector_size;

        let mut dir_data = Vec::new();

        // Read directory sectors
        let mut sectors_read = 0;
        'outer: for track_num in dir_track_start..image.spec().num_tracks {
            if let Some(disk) = image.get_disk(0) {
                if let Some(track) = disk.get_track(track_num) {
                    for sector in track.sectors() {
                        dir_data.extend_from_slice(sector.data());
                        sectors_read += 1;

                        if sectors_read >= dir_sectors {
                            break 'outer;
                        }
                    }
                }
            }
        }

        // Parse directory entries (32 bytes each)
        for chunk in dir_data.chunks(32) {
            if let Some(entry) = CpmDirEntry::parse(chunk) {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Merge extents for files that span multiple directory entries
    fn merge_extents(&self) -> HashMap<String, Vec<&CpmDirEntry>> {
        let mut files: HashMap<String, Vec<&CpmDirEntry>> = HashMap::new();

        for entry in &self.directory_entries {
            let filename = entry.filename_str();
            files.entry(filename).or_insert_with(Vec::new).push(entry);
        }

        // Sort extents by extent number
        for extents in files.values_mut() {
            extents.sort_by_key(|e| e.extent_number());
        }

        files
    }

    /// Read data from allocation blocks
    fn read_blocks(&self, blocks: &[u16]) -> Result<Vec<u8>> {
        let block_size = self.dpb.block_size();
        let sector_size = self.image.spec().sector_size as usize;
        let sectors_per_block = block_size / sector_size;

        let mut data = Vec::new();

        for &block_num in blocks {
            if block_num == 0 || block_num > self.dpb.max_blocks {
                continue;
            }

            // Calculate which track and sector this block starts at
            let block_sector = (block_num as usize) * sectors_per_block;
            let reserved_sectors = self.dpb.reserved_tracks as usize * self.dpb.sectors_per_track as usize;
            let absolute_sector = block_sector + reserved_sectors;

            // Read sectors for this block
            for i in 0..sectors_per_block {
                let sector_num = absolute_sector + i;
                let track = sector_num / self.dpb.sectors_per_track as usize;
                let sector_in_track = sector_num % self.dpb.sectors_per_track as usize;

                if let Some(disk) = self.image.get_disk(0) {
                    if let Some(track_obj) = disk.get_track(track as u8) {
                        // Find sector by index
                        if let Some(sector) = track_obj.get_sector_by_index(sector_in_track) {
                            data.extend_from_slice(sector.data());
                        }
                    }
                }
            }
        }

        Ok(data)
    }

    /// Extract allocation blocks from directory entry
    fn extract_blocks(&self, entry: &CpmDirEntry) -> Vec<u16> {
        let mut blocks = Vec::new();

        // CP/M uses either 8-bit or 16-bit allocation numbers depending on disk size
        if self.dpb.max_blocks < 256 {
            // 8-bit allocation numbers
            for &block in &entry.allocation {
                if block != 0 {
                    blocks.push(block as u16);
                }
            }
        } else {
            // 16-bit allocation numbers
            for i in (0..entry.allocation.len()).step_by(2) {
                if i + 1 < entry.allocation.len() {
                    let block = u16::from_le_bytes([entry.allocation[i], entry.allocation[i + 1]]);
                    if block != 0 {
                        blocks.push(block);
                    }
                }
            }
        }

        blocks
    }
}

impl CpmFileSystem<'_> {
    /// Create a CP/M filesystem from an image
    pub fn from_image<'a>(image: &'a DskImage) -> Result<CpmFileSystem<'a>> {
        // Try to detect format from disk spec
        let dpb = if image.spec().sector_size == 512 && image.spec().sectors_per_track == 9 {
            DiskParameterBlock::spectrum_plus3()
        } else {
            DiskParameterBlock::amstrad_data()
        };

        CpmFileSystem::new(image, dpb)
    }
}

impl<'a> FileSystem for CpmFileSystem<'a> {
    fn from_image<'b>(_image: &'b DskImage) -> Result<Self> where Self: Sized {
        Err(DskError::filesystem("Use CpmFileSystem::from_image() directly"))
    }

    fn from_image_mut<'b>(_image: &'b mut DskImage) -> Result<Self> where Self: Sized {
        Err(DskError::filesystem("Mutable CP/M filesystem not yet implemented"))
    }

    fn read_dir(&self) -> Result<Vec<DirEntry>> {
        let files = self.merge_extents();
        let mut entries = Vec::new();

        for (filename, extents) in files {
            if extents.is_empty() {
                continue;
            }

            let first_extent = extents[0];

            // Calculate total file size
            let mut total_size = 0;
            for extent in &extents {
                // Each record is 128 bytes
                total_size += extent.record_count as usize * 128;
            }

            entries.push(DirEntry {
                name: filename,
                user: first_extent.user,
                extent: first_extent.extent_low,
                size: total_size,
                attributes: first_extent.attributes(),
            });
        }

        // Sort by filename
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(entries)
    }

    fn read_file(&self, name: &str) -> Result<Vec<u8>> {
        let files = self.merge_extents();

        let extents = files
            .get(name)
            .ok_or_else(|| DskError::FileNotFound(name.to_string()))?;

        if extents.is_empty() {
            return Ok(Vec::new());
        }

        // Read all allocation blocks from all extents
        let mut file_data = Vec::new();

        for extent in extents {
            let blocks = self.extract_blocks(extent);
            let block_data = self.read_blocks(&blocks)?;
            file_data.extend_from_slice(&block_data);
        }

        // Trim to actual file size
        let mut actual_size = 0;
        for extent in extents {
            actual_size += extent.record_count as usize * 128;
        }

        if file_data.len() > actual_size {
            file_data.truncate(actual_size);
        }

        Ok(file_data)
    }

    fn write_file(&mut self, _name: &str, _data: &[u8]) -> Result<()> {
        Err(DskError::filesystem("Writing files not yet implemented"))
    }

    fn delete_file(&mut self, _name: &str) -> Result<()> {
        Err(DskError::filesystem("Deleting files not yet implemented"))
    }

    fn info(&self) -> FileSystemInfo {
        let total_blocks = self.dpb.max_blocks as usize + 1;

        // Calculate used blocks
        let mut used_blocks = 0;
        for entry in &self.directory_entries {
            let blocks = self.extract_blocks(entry);
            used_blocks += blocks.len();
        }

        FileSystemInfo {
            fs_type: "CP/M".to_string(),
            total_blocks,
            free_blocks: total_blocks.saturating_sub(used_blocks),
            block_size: self.dpb.block_size(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpb_block_size() {
        let dpb = DiskParameterBlock::amstrad_data();
        assert_eq!(dpb.block_size(), 1024);
    }

    #[test]
    fn test_parse_dir_entry() {
        let mut data = [0u8; 32];
        data[0] = 0; // User 0
        data[1..9].copy_from_slice(b"TESTFILE");
        data[9..12].copy_from_slice(b"TXT");
        data[12] = 0; // Extent low
        data[15] = 10; // Record count

        let entry = CpmDirEntry::parse(&data).unwrap();
        assert_eq!(entry.user, 0);
        assert_eq!(entry.filename_str(), "TESTFILE.TXT");
        assert_eq!(entry.record_count, 10);
    }

    #[test]
    fn test_parse_deleted_entry() {
        let mut data = [0u8; 32];
        data[0] = 0xE5; // Deleted

        let entry = CpmDirEntry::parse(&data);
        assert!(entry.is_none());
    }
}
