/// DSK file reader

use crate::error::{DskError, Result};
use crate::fdc::{FdcStatus1, FdcStatus2};
use crate::format::constants::*;
use crate::format::{detect_format, DskFormat, FormatSpec};
use crate::image::{DataRate, Disk, DskImage, RecordingMode, Sector, SectorId, Track};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Read a DSK file from disk
pub fn read_dsk<P: AsRef<Path>>(path: P) -> Result<DskImage> {
    let mut file = File::open(path)?;

    // Read disk info block (256 bytes)
    let mut disk_info = vec![0u8; DISK_INFO_BLOCK_SIZE];
    file.read_exact(&mut disk_info)?;

    // Detect format
    let format = detect_format(&disk_info)
        .ok_or_else(|| DskError::invalid_format("Unknown DSK format"))?;

    match format {
        DskFormat::Standard => read_standard_dsk(file, &disk_info),
        DskFormat::Extended => read_extended_dsk(file, &disk_info),
    }
}

/// Read a Standard DSK file
fn read_standard_dsk(mut file: File, disk_info: &[u8]) -> Result<DskImage> {
    // Parse disk info block
    let num_tracks = disk_info[DISK_INFO_TRACK_COUNT_OFFSET];
    let num_sides = disk_info[DISK_INFO_SIDE_COUNT_OFFSET];
    let track_size = u16::from_le_bytes([
        disk_info[DISK_INFO_TRACK_SIZE_OFFSET],
        disk_info[DISK_INFO_TRACK_SIZE_OFFSET + 1],
    ]) as usize;

    let mut disks = Vec::with_capacity(num_sides as usize);

    // Read tracks for each side
    for side in 0..num_sides {
        let mut disk = Disk::new(side);

        for track_num in 0..num_tracks {
            let track = read_track(&mut file, track_num, side, track_size)?;
            disk.add_track(track);
        }

        disks.push(disk);
    }

    // Create format spec based on first track
    let spec = build_format_spec(&disks, num_sides, num_tracks);

    Ok(DskImage {
        format: DskFormat::Standard,
        spec,
        disks,
        changed: false,
    })
}

/// Read an Extended DSK file
fn read_extended_dsk(mut file: File, disk_info: &[u8]) -> Result<DskImage> {
    // Parse disk info block
    let num_tracks = disk_info[DISK_INFO_TRACK_COUNT_OFFSET];
    let num_sides = disk_info[DISK_INFO_SIDE_COUNT_OFFSET];

    // Extended format has per-track sizes (in 256-byte units)
    let mut track_sizes = Vec::new();
    for i in 0..(num_tracks as usize * num_sides as usize) {
        let offset = DISK_INFO_EXT_TRACK_SIZE_OFFSET + i;
        if offset < disk_info.len() {
            let size = disk_info[offset] as usize * 256;
            track_sizes.push(size);
        } else {
            track_sizes.push(0);
        }
    }

    let mut disks = Vec::with_capacity(num_sides as usize);

    // Read tracks for each side
    let mut track_index = 0;
    for side in 0..num_sides {
        let mut disk = Disk::new(side);

        for track_num in 0..num_tracks {
            let track_size = track_sizes[track_index];
            track_index += 1;

            if track_size == 0 {
                // Unformatted track - create empty track
                disk.add_track(Track::new(track_num, side));
            } else {
                let track = read_track(&mut file, track_num, side, track_size)?;
                disk.add_track(track);
            }
        }

        disks.push(disk);
    }

    // Create format spec
    let spec = build_format_spec(&disks, num_sides, num_tracks);

    Ok(DskImage {
        format: DskFormat::Extended,
        spec,
        disks,
        changed: false,
    })
}

/// Read a single track from the file
fn read_track(file: &mut File, track_num: u8, side: u8, track_size: usize) -> Result<Track> {
    let mut track_data = vec![0u8; track_size];
    file.read_exact(&mut track_data)?;

    // Parse track info block (256 bytes)
    if track_data.len() < TRACK_INFO_BLOCK_SIZE {
        return Err(DskError::parse(0, "Track too small"));
    }

    // Verify track marker
    if !track_data.starts_with(b"Track-Info") {
        return Err(DskError::parse(0, "Invalid track marker"));
    }

    let _track_number = track_data[0x10];
    let _side_number = track_data[0x11];
    let data_rate = DataRate::from(track_data[0x12]);
    let recording_mode = RecordingMode::from(track_data[0x13]);
    let _sector_size_code = track_data[0x14];
    let num_sectors = track_data[0x15];
    let gap3_length = track_data[0x16];
    let filler_byte = track_data[0x17];

    let mut track = Track::new(track_num, side);
    track.gap3_length = gap3_length;
    track.filler_byte = filler_byte;
    track.data_rate = data_rate;
    track.recording_mode = recording_mode;

    // Parse sector info list (starts at offset 0x18, 8 bytes per sector)
    let mut sector_offset = TRACK_INFO_BLOCK_SIZE;

    for i in 0..num_sectors as usize {
        let sib_offset = 0x18 + (i * SECTOR_INFO_SIZE);
        if sib_offset + SECTOR_INFO_SIZE > track_data.len() {
            break;
        }

        let sib = &track_data[sib_offset..sib_offset + SECTOR_INFO_SIZE];

        let sector_track = sib[0];
        let sector_side = sib[1];
        let sector_id = sib[2];
        let sector_size_code = sib[3];
        let fdc_st1 = sib[4];
        let fdc_st2 = sib[5];
        let data_length = if sib.len() >= 8 {
            u16::from_le_bytes([sib[6], sib[7]])
        } else {
            fdc_size_to_stored_bytes(sector_size_code) as u16
        };

        // Calculate actual sector data size
        // Use stored size rules when data_length is 0 (standard format fallback)
        let actual_size = if data_length > 0 {
            data_length as usize
        } else {
            fdc_size_to_stored_bytes(sector_size_code)
        };

        // Extract sector data
        let sector_data = if sector_offset + actual_size <= track_data.len() {
            track_data[sector_offset..sector_offset + actual_size].to_vec()
        } else if sector_offset < track_data.len() {
            // Partial data available, pad with filler
            let mut data = track_data[sector_offset..].to_vec();
            data.resize(actual_size, filler_byte);
            data
        } else {
            // No data available, fill entirely with filler byte
            vec![filler_byte; actual_size]
        };

        sector_offset += actual_size;

        let id = SectorId::new(sector_track, sector_side, sector_id, sector_size_code);
        let sector = Sector::with_status(
            id,
            FdcStatus1::new(fdc_st1),
            FdcStatus2::new(fdc_st2),
            sector_data,
        );

        track.add_sector(sector);
    }

    Ok(track)
}

/// Build a format specification from the disk structure
fn build_format_spec(disks: &[Disk], num_sides: u8, num_tracks: u8) -> FormatSpec {
    // Try to detect format from first non-empty track
    let mut sectors_per_track = 9;
    let mut sector_size = 512;
    let mut first_sector_id = 0xC1;
    let mut gap3_length = 0x4E;
    let mut filler_byte = 0xE5;

    if let Some(disk) = disks.first() {
        if let Some(track) = disk.tracks().iter().find(|t| !t.is_empty()) {
            sectors_per_track = track.sector_count() as u8;
            gap3_length = track.gap3_length;
            filler_byte = track.filler_byte;

            if let Some(sector) = track.sectors().first() {
                sector_size = sector.advertised_size() as u16;
                first_sector_id = sector.id.sector;
            }
        }
    }

    FormatSpec {
        num_sides,
        num_tracks,
        sectors_per_track,
        sector_size,
        first_sector_id,
        gap3_length,
        filler_byte,
        interleave: 1,
        side_mode: if num_sides == 1 {
            crate::format::spec::SideMode::SingleSide
        } else {
            crate::format::spec::SideMode::Alternate
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_format_spec() {
        let mut disks = Vec::new();
        let mut disk = Disk::new(0);
        let mut track = Track::new(0, 0);

        for i in 0..9 {
            let id = SectorId::new(0, 0, 0xC1 + i, 2);
            track.add_sector(Sector::new(id));
        }

        disk.add_track(track);
        disks.push(disk);

        let spec = build_format_spec(&disks, 1, 40);

        assert_eq!(spec.num_sides, 1);
        assert_eq!(spec.num_tracks, 40);
        assert_eq!(spec.sectors_per_track, 9);
        assert_eq!(spec.sector_size, 512);
        assert_eq!(spec.first_sector_id, 0xC1);
    }
}
