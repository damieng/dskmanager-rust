/// DSK file writer

use crate::error::Result;
use crate::format::constants::*;
use crate::format::DskFormat;
use crate::image::DskImage;
use std::fs::File;
use std::io::{Write};
use std::path::Path;

/// Write a DSK file to disk
pub fn write_dsk<P: AsRef<Path>>(image: &DskImage, path: P) -> Result<()> {
    let mut file = File::create(path)?;

    match image.format {
        DskFormat::Standard => write_standard_dsk(&mut file, image),
        DskFormat::Extended => write_extended_dsk(&mut file, image),
    }
}

/// Write a Standard DSK file
fn write_standard_dsk(file: &mut File, image: &DskImage) -> Result<()> {
    // Calculate track size (assume all tracks are the same size)
    let track_size = calculate_track_size(image);

    // Write disk info block
    let mut disk_info = vec![0u8; DISK_INFO_BLOCK_SIZE];

    // Copy magic bytes
    disk_info[..STANDARD_DSK_SIGNATURE.len()].copy_from_slice(STANDARD_DSK_SIGNATURE);

    // Copy creator signature
    let creator_len = CREATOR_SIGNATURE.len().min(14);
    disk_info[DISK_INFO_CREATOR_OFFSET..DISK_INFO_CREATOR_OFFSET + creator_len]
        .copy_from_slice(&CREATOR_SIGNATURE[..creator_len]);

    // Set track and side counts
    disk_info[DISK_INFO_TRACK_COUNT_OFFSET] = image.spec.num_tracks;
    disk_info[DISK_INFO_SIDE_COUNT_OFFSET] = image.spec.num_sides;

    // Set track size (in bytes, little-endian)
    let track_size_bytes = (track_size as u16).to_le_bytes();
    disk_info[DISK_INFO_TRACK_SIZE_OFFSET] = track_size_bytes[0];
    disk_info[DISK_INFO_TRACK_SIZE_OFFSET + 1] = track_size_bytes[1];

    file.write_all(&disk_info)?;

    // Write tracks for each side
    for disk in &image.disks {
        for track in disk.tracks() {
            write_track(file, track, track_size)?;
        }
    }

    Ok(())
}

/// Write an Extended DSK file
fn write_extended_dsk(file: &mut File, image: &DskImage) -> Result<()> {
    // Write disk info block
    let mut disk_info = vec![0u8; DISK_INFO_BLOCK_SIZE];

    // Copy magic bytes
    disk_info[..EXTENDED_DSK_SIGNATURE.len()].copy_from_slice(EXTENDED_DSK_SIGNATURE);

    // Copy creator signature
    let creator_len = CREATOR_SIGNATURE.len().min(14);
    disk_info[DISK_INFO_CREATOR_OFFSET..DISK_INFO_CREATOR_OFFSET + creator_len]
        .copy_from_slice(&CREATOR_SIGNATURE[..creator_len]);

    // Set track and side counts
    disk_info[DISK_INFO_TRACK_COUNT_OFFSET] = image.spec.num_tracks;
    disk_info[DISK_INFO_SIDE_COUNT_OFFSET] = image.spec.num_sides;

    // Set per-track sizes (in 256-byte units)
    let mut track_index = 0;
    for disk in &image.disks {
        for track in disk.tracks() {
            let track_size = calculate_single_track_size(track);
            let size_units = ((track_size + 255) / 256) as u8; // Round up
            let offset = DISK_INFO_EXT_TRACK_SIZE_OFFSET + track_index;
            if offset < disk_info.len() {
                disk_info[offset] = size_units;
            }
            track_index += 1;
        }
    }

    file.write_all(&disk_info)?;

    // Write tracks for each side
    for disk in &image.disks {
        for track in disk.tracks() {
            let track_size = calculate_single_track_size(track);
            write_track(file, track, track_size)?;
        }
    }

    Ok(())
}

/// Write a single track to the file
fn write_track(file: &mut File, track: &crate::image::Track, track_size: usize) -> Result<()> {
    let mut track_data = vec![0u8; track_size];

    // Write track info block (256 bytes)
    track_data[..TRACK_INFO_MARKER.len()].copy_from_slice(TRACK_INFO_MARKER);

    track_data[0x10] = track.track_number;
    track_data[0x11] = track.side_number;
    track_data[0x12] = track.data_rate.into();
    track_data[0x13] = track.recording_mode.into();

    // Use first sector's size code if available, otherwise default to 2 (512 bytes)
    let sector_size_code = track.sectors().first().map(|s| s.id.size_code).unwrap_or(2);
    track_data[0x14] = sector_size_code;
    track_data[0x15] = track.sector_count() as u8;
    track_data[0x16] = track.gap3_length;
    track_data[0x17] = track.filler_byte;

    // Write sector info list (starts at offset 0x18, 8 bytes per sector)
    let mut sector_offset = TRACK_INFO_BLOCK_SIZE;

    for (i, sector) in track.sectors().iter().enumerate() {
        let sib_offset = 0x18 + (i * SECTOR_INFO_SIZE);
        if sib_offset + SECTOR_INFO_SIZE > track_data.len() {
            break;
        }

        let sib = &mut track_data[sib_offset..sib_offset + SECTOR_INFO_SIZE];

        sib[0] = sector.id.track;
        sib[1] = sector.id.side;
        sib[2] = sector.id.sector;
        sib[3] = sector.id.size_code;
        sib[4] = sector.fdc_status1.0;
        sib[5] = sector.fdc_status2.0;

        // Write data length for extended format
        let data_len_bytes = sector.data_length.to_le_bytes();
        sib[6] = data_len_bytes[0];
        sib[7] = data_len_bytes[1];

        // Copy sector data
        let sector_data = sector.data();
        let copy_len = sector_data.len().min(track_data.len() - sector_offset);
        if copy_len > 0 {
            track_data[sector_offset..sector_offset + copy_len]
                .copy_from_slice(&sector_data[..copy_len]);
        }
        sector_offset += sector_data.len();
    }

    file.write_all(&track_data)?;
    Ok(())
}

/// Calculate the size of a track in bytes (including track info block)
fn calculate_single_track_size(track: &crate::image::Track) -> usize {
    let mut size = TRACK_INFO_BLOCK_SIZE;
    for sector in track.sectors() {
        size += sector.actual_size();
    }
    size
}

/// Calculate a uniform track size for standard format
fn calculate_track_size(image: &DskImage) -> usize {
    // Find the largest track size
    let mut max_size = TRACK_INFO_BLOCK_SIZE;

    for disk in &image.disks {
        for track in disk.tracks() {
            let track_size = calculate_single_track_size(track);
            if track_size > max_size {
                max_size = track_size;
            }
        }
    }

    max_size
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{Sector, SectorId, Track};

    #[test]
    fn test_calculate_single_track_size() {
        let mut track = Track::new(0, 0);

        for i in 0..9 {
            let id = SectorId::new(0, 0, 0xC1 + i, 2);
            track.add_sector(Sector::new(id));
        }

        let size = calculate_single_track_size(&track);
        assert_eq!(size, 256 + 9 * 512); // Track info + 9 * 512-byte sectors
    }
}
