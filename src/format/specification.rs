/// Disk specification for CP/M and similar file systems
///
/// This module provides detection and representation of disk specifications
/// used by CP/M and compatible systems (Amstrad PCW, CPC, Spectrum +3, etc.)

use crate::image::DiskImage;
use std::fmt;


/// Side configuration for double-sided disks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskSpecSide {
    /// Single-sided disk
    Single,
    /// Double-sided with alternating tracks (T0S0, T0S1, T1S0, T1S1...)
    DoubleAlternate,
    /// Double-sided with successive tracks (all of side 0, then all of side 1)
    DoubleSuccessive,
    /// Double-sided with reverse order on side 1
    DoubleReverse,
    /// Invalid or unrecognized
    Invalid,
}

impl fmt::Display for DiskSpecSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiskSpecSide::Single => write!(f, "Single"),
            DiskSpecSide::DoubleAlternate => write!(f, "Double (Alternate)"),
            DiskSpecSide::DoubleSuccessive => write!(f, "Double (Successive)"),
            DiskSpecSide::DoubleReverse => write!(f, "Double (Reverse)"),
            DiskSpecSide::Invalid => write!(f, "Invalid"),
        }
    }
}

/// Track density
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskSpecTrack {
    /// Single density (40 tracks)
    Single,
    /// Double density (80 tracks)
    Double,
    /// Invalid or unrecognized
    Invalid,
}

impl fmt::Display for DiskSpecTrack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiskSpecTrack::Single => write!(f, "Single"),
            DiskSpecTrack::Double => write!(f, "Double"),
            DiskSpecTrack::Invalid => write!(f, "Invalid"),
        }
    }
}

/// Allocation block size type (for block allocation map)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationSize {
    /// 8-bit block numbers (max 255 blocks)
    Byte,
    /// 16-bit block numbers (max 65535 blocks)
    Word,
}

impl fmt::Display for AllocationSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AllocationSize::Byte => write!(f, "8-bit/byte"),
            AllocationSize::Word => write!(f, "16-bit/word"),
        }
    }
}

/// Disk specification containing all parameters needed to read a CP/M filesystem
#[derive(Debug, Clone)]
pub struct DiskSpecification {
    /// How this specification was determined
    pub source: String,
    /// Disk format name
    pub format: String,
    /// Side configuration
    pub side: DiskSpecSide,
    /// Track density
    pub track: DiskSpecTrack,
    /// Number of tracks per side
    pub tracks_per_side: u8,
    /// Number of sectors per track
    pub sectors_per_track: u8,
    /// Sector size in bytes
    pub sector_size: u16,
    /// FDC sector size code (N value)
    pub fdc_sector_size: u8,
    /// Number of reserved tracks (for boot sector, etc.)
    pub reserved_tracks: u8,
    /// Block shift value (block size = 128 << block_shift)
    pub block_shift: u8,
    /// Number of directory blocks
    pub directory_blocks: u8,
    /// Gap length for read/write operations
    pub gap_read_write: u8,
    /// Gap length for formatting
    pub gap_format: u8,
    /// Checksum byte (from spec block)
    pub checksum: u8,
    /// Allocation block size type
    pub allocation_size: AllocationSize,
}

impl Default for DiskSpecification {
    fn default() -> Self {
        Self {
            source: String::new(),
            format: "Amstrad PCW/+3 DD/SS/ST (Assumed)".to_string(),
            side: DiskSpecSide::Single,
            track: DiskSpecTrack::Single,
            tracks_per_side: 40,
            sectors_per_track: 9,
            sector_size: 512,
            fdc_sector_size: 2,
            reserved_tracks: 1,
            block_shift: 3,
            directory_blocks: 2,
            gap_read_write: 42,
            gap_format: 82,
            checksum: 0,
            allocation_size: AllocationSize::Byte,
        }
    }
}

/// Trait for format detectors that can identify disk specifications
pub trait FormatDetector {
    /// Attempt to detect and return a disk specification for the given image.
    /// Returns `Some(DiskSpecification)` if this detector can handle the disk,
    /// or `None` if it cannot.
    fn detect(&self, image: &DiskImage) -> Option<DiskSpecification>;
}

/// Amstrad PCW format detector
pub struct AmstradPCW;

impl FormatDetector for AmstradPCW {
    fn detect(&self, image: &DiskImage) -> Option<DiskSpecification> {
        let first_sector = get_first_logical_sector(image)?;
        let (_, sector_data) = first_sector;

        if sector_data.len() < 10 {
            return None;
        }

        // Check if first 10 bytes are all the same value (blank spec block)
        let check_byte = sector_data[0];
        let all_same = sector_data[..10].iter().all(|&b| b == check_byte);
        if all_same {
            let mut spec = DiskSpecification::new();
            spec.set_defaults();
            spec.source = format!("Sector 0 spec block is all 0x{:02X}", check_byte);
            spec.update_allocation_size();
            return Some(spec);
        }

        // Check format byte
        match sector_data[0] {
            0 => {
                // PCW Single Sided
                let mut spec = DiskSpecification::new();
                spec.format = "Amstrad PCW/+3 DD/SS/ST".to_string();
                spec.source = "Sector 0 spec block (format byte 0)".to_string();
                parse_spec_block(&mut spec, &sector_data);
                spec.update_allocation_size();
                Some(spec)
            }
            3 => {
                // PCW Double Sided
                let mut spec = DiskSpecification::new();
                spec.format = "Amstrad PCW DD/DS/DT".to_string();
                spec.source = "Sector 0 spec block (format byte 3)".to_string();
                parse_spec_block(&mut spec, &sector_data);
                spec.update_allocation_size();
                Some(spec)
            }
            _ => None,
        }
    }
}

/// Amstrad CPC System format detector
pub struct AmstradCPCSystem;

impl FormatDetector for AmstradCPCSystem {
    fn detect(&self, image: &DiskImage) -> Option<DiskSpecification> {
        let first_sector = get_first_logical_sector(image)?;
        let (sector_id, sector_data) = first_sector;

        // Check first sector ID for CPC System format
        if sector_id == 0x41 {
            let mut spec = DiskSpecification::new();
            spec.set_defaults();
            spec.source = "First logical sector has ID of 65 (0x41)".to_string();
            spec.format = "Amstrad CPC DD/SS/ST system".to_string();
            spec.reserved_tracks = 2;
            spec.update_allocation_size();
            return Some(spec);
        }

        // Check spec block format byte
        if sector_data.len() >= 10 && sector_data[0] == 1 {
            let mut spec = DiskSpecification::new();
            spec.format = "Amstrad CPC DD/SS/ST system".to_string();
            spec.source = "Sector 0 spec block (format byte 1)".to_string();
            parse_spec_block(&mut spec, &sector_data);
            spec.update_allocation_size();
            Some(spec)
        } else {
            None
        }
    }
}

/// Amstrad CPC Data format detector
pub struct AmstradCPCData;

impl FormatDetector for AmstradCPCData {
    fn detect(&self, image: &DiskImage) -> Option<DiskSpecification> {
        let first_sector = get_first_logical_sector(image)?;
        let (sector_id, sector_data) = first_sector;

        // Check first sector ID for CPC Data format
        if sector_id == 0xC1 {
            let mut spec = DiskSpecification::new();
            spec.set_defaults();
            spec.source = "First logical sector has ID of 193 (0xC1)".to_string();
            spec.format = "Amstrad CPC DD/SS/ST data".to_string();
            spec.reserved_tracks = 0;
            spec.update_allocation_size();
            return Some(spec);
        }

        // Check spec block format byte
        if sector_data.len() >= 10 && sector_data[0] == 2 {
            let mut spec = DiskSpecification::new();
            spec.format = "Amstrad CPC DD/SS/ST data".to_string();
            spec.source = "Sector 0 spec block (format byte 2)".to_string();
            parse_spec_block(&mut spec, &sector_data);
            spec.update_allocation_size();
            Some(spec)
        } else {
            None
        }
    }
}

/// Tatung Einstein format detector
pub struct Einstein;

impl FormatDetector for Einstein {
    fn detect(&self, image: &DiskImage) -> Option<DiskSpecification> {
        let first_sector = get_first_logical_sector(image)?;
        let (_, data) = first_sector;

        if data.len() >= 6 {
            // Einstein boot sector signature: 00 E1 00 FB 00 FA
            if data[0] == 0x00
                && data[1] == 0xE1
                && data[2] == 0x00
                && data[3] == 0xFB
                && data[4] == 0x00
                && data[5] == 0xFA
            {
                let mut spec = DiskSpecification::new();
                spec.format = "Tatung Einstein".to_string();
                spec.source = "Signature 00 E1 00 FB 00 FA on first logical sector".to_string();
                spec.sector_size = 512;
                spec.sectors_per_track = 10;
                spec.tracks_per_side = 40;
                spec.block_shift = 4;
                spec.reserved_tracks = 2;
                spec.directory_blocks = 1;
                spec.allocation_size = AllocationSize::Word;
                spec.fdc_sector_size = 2;
                return Some(spec);
            }
        }

        None
    }
}

/// MGT Sam Coupe format detector
pub struct Mgt;

impl FormatDetector for Mgt {
    fn detect(&self, image: &DiskImage) -> Option<DiskSpecification> {
        let disk = image.get_disk(0)?;
        let track = disk.get_track(0)?;

        // Check for MGT format: double-sided, 80 tracks, 10 sectors of 512 bytes
        let total_tracks: usize = image.disks().iter().map(|d| d.track_count()).sum();
        let is_double_sided = image.disk_count() == 2;
        if is_double_sided && total_tracks >= 160 {
            let sectors = track.sectors();
            if sectors.len() == 10 {
                let all_512 = sectors.iter().all(|s| s.advertised_size() == 512);
                if all_512 {
                    let mut spec = DiskSpecification::new();
                    spec.format = "MGT Sam Coupe".to_string();
                    spec.source = "Double sided 80 track 10 sectors of 512 bytes".to_string();
                    spec.sector_size = 512;
                    spec.sectors_per_track = 10;
                    spec.tracks_per_side = 80;
                    spec.side = DiskSpecSide::DoubleSuccessive;
                    spec.track = DiskSpecTrack::Double;
                    spec.reserved_tracks = 0;
                    spec.directory_blocks = 4;
                    spec.fdc_sector_size = 2;
                    spec.update_allocation_size();
                    return Some(spec);
                }
            }
        }

        None
    }
}

/// Timex/Sinclair TS2068 format detector
pub struct Ts2068;

impl FormatDetector for Ts2068 {
    fn detect(&self, image: &DiskImage) -> Option<DiskSpecification> {
        let disk = image.get_disk(0)?;
        let track = disk.get_track(0)?;

        // Check for TS2068 format: 16 sectors of 256 bytes, starting at ID 0
        let sectors = track.sectors();
        if sectors.len() == 16 {
            let all_256 = sectors.iter().all(|s| s.advertised_size() == 256);
            let starts_at_0 = sectors.iter().any(|s| s.id.sector == 0);
            if all_256 && starts_at_0 {
                let mut spec = DiskSpecification::new();
                spec.format = "Timex/Sinclair TS2068".to_string();
                spec.source = "16x 256 byte sectors per track, starting ID 0".to_string();
                spec.sector_size = 256;
                spec.sectors_per_track = 16;
                spec.tracks_per_side = 40;
                spec.gap_read_write = 12;
                spec.gap_format = 23;
                spec.reserved_tracks = 2;
                spec.directory_blocks = 1;
                spec.fdc_sector_size = 1;
                spec.update_allocation_size();
                return Some(spec);
            }
        }

        None
    }
}

/// Assumed PCW Single Sided format detector (fallback for blank spec blocks)
pub struct AssumedPcwSingleSided;

impl FormatDetector for AssumedPcwSingleSided {
    fn detect(&self, image: &DiskImage) -> Option<DiskSpecification> {
        // This detects blank spec blocks (all same byte)
        let first_sector = get_first_logical_sector(image)?;
        let (_, sector_data) = first_sector;

        if sector_data.len() < 10 {
            return None;
        }

        // Check if first 10 bytes are all the same value (blank spec block)
        let check_byte = sector_data[0];
        let all_same = sector_data[..10].iter().all(|&b| b == check_byte);
        if all_same {
            let mut spec = DiskSpecification::new();
            spec.set_defaults();
            spec.source = format!("Sector 0 spec block is all 0x{:02X}", check_byte);
            spec.update_allocation_size();
            Some(spec)
        } else {
            None
        }
    }
}

/// Invalid format detector (for unrecognized format bytes)
pub struct InvalidFormat;

impl FormatDetector for InvalidFormat {
    fn detect(&self, image: &DiskImage) -> Option<DiskSpecification> {
        let first_sector = get_first_logical_sector(image)?;
        let (_, sector_data) = first_sector;

        if sector_data.len() < 10 {
            return None;
        }

        // Check if we have a spec block with an invalid format byte
        // Valid format bytes are 0, 1, 2, 3
        let format_byte = sector_data[0];
        if format_byte > 3 {
            // Check if it's not a blank spec block (all same byte)
            let check_byte = sector_data[0];
            let all_same = sector_data[..10].iter().all(|&b| b == check_byte);
            if !all_same {
                let mut spec = DiskSpecification::new();
                spec.format = "Invalid".to_string();
                spec.source = format!("Unknown format byte: 0x{:02X}", format_byte);
                return Some(spec);
            }
        }

        None
    }
}

/// Default fallback detector (always matches if image has sectors)
pub struct DefaultFallback;

impl FormatDetector for DefaultFallback {
    fn detect(&self, image: &DiskImage) -> Option<DiskSpecification> {
        // This is the final fallback - if we have sectors but nothing else matched,
        // return a default assumed PCW spec
        if get_first_logical_sector(image).is_some() {
            let mut spec = DiskSpecification::new();
            spec.set_defaults();
            spec.source = "Default fallback (no specific format detected)".to_string();
            spec.update_allocation_size();
            Some(spec)
        } else {
            None
        }
    }
}

/// Parse a spec block from sector data
fn parse_spec_block(spec: &mut DiskSpecification, sector_data: &[u8]) {
    if sector_data.len() < 10 {
        return;
    }

    // Parse side configuration
    spec.side = match sector_data[1] & 0x03 {
        0 => DiskSpecSide::Single,
        1 => DiskSpecSide::DoubleAlternate,
        2 => DiskSpecSide::DoubleSuccessive,
        _ => DiskSpecSide::Invalid,
    };

    // Parse track density
    spec.track = if (sector_data[1] & 0x80) == 0x80 {
        DiskSpecTrack::Double
    } else {
        DiskSpecTrack::Single
    };

    spec.tracks_per_side = sector_data[2];
    spec.sectors_per_track = sector_data[3];

    // Parse sector size (stored as log2(size) - 7)
    let size_code = sector_data[4];
    let calculated_size = 1u16 << (size_code + 7);
    if calculated_size <= 8192 {
        spec.sector_size = calculated_size;
        spec.fdc_sector_size = size_code;
    } else {
        spec.sector_size = 0;
    }

    spec.reserved_tracks = sector_data[5];
    spec.block_shift = sector_data[6];
    spec.directory_blocks = sector_data[7];
    spec.gap_read_write = sector_data[8];
    spec.gap_format = sector_data[9];

    if sector_data.len() > 15 {
        spec.checksum = sector_data[15];
    }
}

/// Identify the disk specification by trying all format detectors in order
pub fn identify_specification(image: &DiskImage) -> Option<DiskSpecification> {
    // Try detectors in order of specificity (most specific first)
    let detectors: Vec<Box<dyn FormatDetector>> = vec![
        Box::new(Einstein),
        Box::new(Ts2068),
        Box::new(Mgt),
        Box::new(AmstradCPCSystem),
        Box::new(AmstradCPCData),
        Box::new(AmstradPCW),
        Box::new(AssumedPcwSingleSided),
        Box::new(InvalidFormat),
        Box::new(DefaultFallback),
    ];

    for detector in detectors {
        if let Some(spec) = detector.detect(image) {
            return Some(spec);
        }
    }

    None
}

impl DiskSpecification {
    /// Create a new disk specification with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate the block size in bytes
    pub fn block_size(&self) -> usize {
        128 << self.block_shift
    }

    /// Calculate the number of blocks on the disk
    pub fn block_count(&self) -> u16 {
        let usable = self.usable_capacity();
        let block_size = self.block_size();
        if block_size == 0 {
            0
        } else {
            (usable / block_size) as u16
        }
    }

    /// Calculate the usable capacity in bytes (excluding reserved tracks)
    pub fn usable_capacity(&self) -> usize {
        let mut usable_tracks = self.tracks_per_side as usize;
        if self.side != DiskSpecSide::Single {
            usable_tracks *= 2;
        }
        usable_tracks = usable_tracks.saturating_sub(self.reserved_tracks as usize);
        usable_tracks * self.sectors_per_track as usize * self.sector_size as usize
    }

    /// Calculate the number of 128-byte records per track
    pub fn records_per_track(&self) -> usize {
        (self.sector_size as usize * self.sectors_per_track as usize) / 128
    }

    /// Calculate the number of directory entries
    pub fn directory_entries(&self) -> usize {
        (self.directory_blocks as usize * self.block_size()) / 32
    }

    /// Get the number of sides
    pub fn side_count(&self) -> u8 {
        if self.side == DiskSpecSide::Single {
            1
        } else {
            2
        }
    }

    /// Calculate total disk capacity in bytes
    pub fn total_capacity(&self) -> usize {
        let tracks = self.tracks_per_side as usize * self.side_count() as usize;
        tracks * self.sectors_per_track as usize * self.sector_size as usize
    }

    /// Update allocation size based on block count
    fn update_allocation_size(&mut self) {
        if self.block_count() > 255 {
            self.allocation_size = AllocationSize::Word;
        } else {
            self.allocation_size = AllocationSize::Byte;
        }
    }

    /// Identify the disk specification from a disk image
    /// 
    /// This is a convenience method that calls `identify_specification`.
    /// For more control, use `identify_specification` directly.
    pub fn identify(image: &DiskImage) -> Self {
        // Check if we have any sectors first
        if get_first_logical_sector(image).is_none() {
            let mut spec = Self::new();
            spec.format = "Invalid".to_string();
            spec.source = "No sectors found".to_string();
            return spec;
        }

        identify_specification(image).unwrap_or_else(|| {
            let mut spec = Self::new();
            spec.format = "Invalid".to_string();
            spec.source = "No matching format detector found".to_string();
            spec
        })
    }

    /// Set default PCW/+3 values
    fn set_defaults(&mut self) {
        self.format = "Amstrad PCW/+3 DD/SS/ST (Assumed)".to_string();
        self.side = DiskSpecSide::Single;
        self.track = DiskSpecTrack::Single;
        self.tracks_per_side = 40;
        self.sectors_per_track = 9;
        self.sector_size = 512;
        self.fdc_sector_size = 2;
        self.reserved_tracks = 1;
        self.block_shift = 3;
        self.directory_blocks = 2;
        self.gap_read_write = 42;
        self.gap_format = 82;
    }
}

/// Get the first logical sector (lowest sector ID on track 0)
fn get_first_logical_sector(image: &DiskImage) -> Option<(u8, Vec<u8>)> {
    let disk = image.get_disk(0)?;
    let track = disk.get_track(0)?;

    let sectors = track.sectors();
    if sectors.is_empty() {
        return None;
    }

    // Find the sector with the lowest ID
    let min_sector = sectors.iter().min_by_key(|s| s.id.sector)?;
    Some((min_sector.id.sector, min_sector.data().to_vec()))
}

impl fmt::Display for DiskSpecification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Format: {}", self.format)?;
        writeln!(f, "Source: {}", self.source)?;
        writeln!(f, "Density: {}", self.track)?;
        writeln!(f, "Sides: {}", self.side)?;
        writeln!(f, "Tracks: {} ({} per side)", self.tracks_per_side * self.side_count(), self.tracks_per_side)?;
        writeln!(f, "Sectors per track: {}", self.sectors_per_track)?;
        writeln!(f, "Sector size: {} bytes (FDC N={})", self.sector_size, self.fdc_sector_size)?;
        writeln!(f, "Reserved tracks: {}", self.reserved_tracks)?;
        writeln!(f, "Block shift: {}", self.block_shift)?;
        writeln!(f, "Directory: {} blocks ({} entries)", self.directory_blocks, self.directory_entries())?;
        writeln!(f, "Gap: R/W {}, format {}", self.gap_read_write, self.gap_format)?;
        writeln!(f, "Block size: {} bytes ({} blocks)", self.block_size(), self.block_count())?;
        writeln!(f, "Allocation size: {}", self.allocation_size)?;
        writeln!(f, "Total capacity: {} KB", self.total_capacity() / 1024)?;
        writeln!(f, "Usable capacity: {} KB", self.usable_capacity() / 1024)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_spec() {
        let spec = DiskSpecification::new();
        assert_eq!(spec.format, "Amstrad PCW/+3 DD/SS/ST (Assumed)");
        assert_eq!(spec.side, DiskSpecSide::Single);
        assert_eq!(spec.tracks_per_side, 40);
        assert_eq!(spec.sectors_per_track, 9);
        assert_eq!(spec.sector_size, 512);
    }

    #[test]
    fn test_block_size() {
        let mut spec = DiskSpecification::new();
        spec.block_shift = 3;
        assert_eq!(spec.block_size(), 1024);

        spec.block_shift = 4;
        assert_eq!(spec.block_size(), 2048);
    }

    #[test]
    fn test_usable_capacity() {
        let spec = DiskSpecification::new();
        // 40 tracks - 1 reserved = 39 tracks
        // 39 * 9 sectors * 512 bytes = 179712 bytes
        assert_eq!(spec.usable_capacity(), 179712);
    }

    #[test]
    fn test_block_count() {
        let spec = DiskSpecification::new();
        // 179712 bytes / 1024 bytes per block = 175 blocks
        assert_eq!(spec.block_count(), 175);
    }

    #[test]
    fn test_directory_entries() {
        let spec = DiskSpecification::new();
        // 2 blocks * 1024 bytes / 32 bytes per entry = 64 entries
        assert_eq!(spec.directory_entries(), 64);
    }

    #[test]
    fn test_records_per_track() {
        let spec = DiskSpecification::new();
        // 9 sectors * 512 bytes / 128 bytes per record = 36 records
        assert_eq!(spec.records_per_track(), 36);
    }

    #[test]
    fn test_format_string() {
        let mut spec = DiskSpecification::new();
        spec.format = "Amstrad PCW/+3 DD/SS/ST".to_string();
        assert_eq!(spec.format, "Amstrad PCW/+3 DD/SS/ST");
        
        spec.format = "Amstrad CPC DD/SS/ST system".to_string();
        assert_eq!(spec.format, "Amstrad CPC DD/SS/ST system");
    }

    #[test]
    fn test_side_display() {
        assert_eq!(format!("{}", DiskSpecSide::Single), "Single");
        assert_eq!(
            format!("{}", DiskSpecSide::DoubleAlternate),
            "Double (Alternate)"
        );
    }
}
