/// Copy protection detection for DSK disk images
///
/// Detects various copy protection schemes used on Amstrad CPC, ZX Spectrum +3,
/// and other systems that used the DSK format.

use crate::image::{Disk, Sector, Track};

/// Result of copy protection detection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectionResult {
    /// Name of the detected protection scheme
    pub name: String,
    /// Description of why this protection was detected
    pub reason: String,
}

impl ProtectionResult {
    /// Create a new protection result
    pub fn new(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for ProtectionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.reason)
    }
}

/// Find a byte pattern in a buffer, returning the offset if found
fn find_pattern(data: &[u8], pattern: &[u8]) -> Option<usize> {
    if pattern.is_empty() || data.len() < pattern.len() {
        return None;
    }
    data.windows(pattern.len())
        .position(|window| window == pattern)
}

/// Check if all tracks on the disk have a uniform format
fn is_uniform(disk: &Disk) -> bool {
    if disk.track_count() == 0 {
        return true;
    }

    let first_track = match disk.get_track(0) {
        Some(t) => t,
        None => return true,
    };

    let sector_count = first_track.sector_count();
    let sector_size = first_track.uniform_sector_size();

    for t_idx in 1..disk.track_count() {
        if let Some(track) = disk.get_track(t_idx as u8) {
            if track.sector_count() != sector_count {
                return false;
            }
            if track.uniform_sector_size() != sector_size {
                return false;
            }
        }
    }

    true
}

/// Check if any sector on the disk has FDC errors
fn has_fdc_errors(disk: &Disk) -> bool {
    for t_idx in 0..disk.track_count() {
        let Some(track) = disk.get_track(t_idx as u8) else {
            continue;
        };
        for s_idx in 0..track.sector_count() {
            let Some(sector) = track.get_sector_by_index(s_idx) else {
                continue;
            };
            if sector.has_error() {
                return true;
            }
        }
    }
    false
}

/// Get the largest track size in bytes
fn get_largest_track_size(disk: &Disk) -> usize {
    (0..disk.track_count())
        .filter_map(|t| disk.get_track(t as u8))
        .map(|track| track.total_data_size())
        .max()
        .unwrap_or(0)
}

/// Helper to get track and sector by indices
fn get_sector(disk: &Disk, track_idx: u8, sector_idx: usize) -> Option<&Sector> {
    disk.get_track(track_idx)?.get_sector_by_index(sector_idx)
}

/// Helper to get a track
fn get_track(disk: &Disk, track_idx: u8) -> Option<&Track> {
    disk.get_track(track_idx)
}

/// Search for a signature pattern across all sectors
fn find_signature_in_disk(disk: &Disk, pattern: &[u8]) -> Option<(usize, usize, usize)> {
    for t_idx in 0..disk.track_count() {
        let Some(track) = get_track(disk, t_idx as u8) else {
            continue;
        };
        for s_idx in 0..track.sector_count() {
            let Some(sector) = track.get_sector_by_index(s_idx) else {
                continue;
            };
            if let Some(offset) = find_pattern(sector.data(), pattern) {
                return Some((t_idx, s_idx, offset));
            }
        }
    }
    None
}

/// Search for a signature in a specific track
fn find_signature_in_track(track: &Track, pattern: &[u8]) -> Option<(usize, usize)> {
    for s_idx in 0..track.sector_count() {
        let Some(sector) = track.get_sector_by_index(s_idx) else {
            continue;
        };
        if let Some(offset) = find_pattern(sector.data(), pattern) {
            return Some((s_idx, offset));
        }
    }
    None
}

// ============================================================================
// Individual protection detection functions
// ============================================================================

fn detect_alkatraz(disk: &Disk) -> Option<ProtectionResult> {
    let sector0 = get_sector(disk, 0, 0)?;

    // Alkatraz +3 (signed)
    if let Some(offset) = find_pattern(
        sector0.data(),
        b" THE ALKATRAZ PROTECTION SYSTEM   (C) 1987  Appleby Associates",
    ) {
        return Some(ProtectionResult::new(
            "Alkatraz +3",
            format!("signed at T0/S0 +{}", offset),
        ));
    }

    // Alkatraz CPC (18 sector track with 256-byte sectors)
    for t_idx in 0..disk.track_count().saturating_sub(1) {
        let Some(track) = get_track(disk, t_idx as u8) else {
            continue;
        };
        if track.sector_count() != 18 {
            continue;
        }

        let Some(first_sector) = track.get_sector_by_index(0) else {
            continue;
        };

        if first_sector.actual_size() == 256 {
            return Some(ProtectionResult::new(
                "Alkatraz CPC",
                format!("18 sector T{}", t_idx),
            ));
        }

        if first_sector.advertised_size() != 256 {
            continue;
        }

        let Some(next_track) = get_track(disk, (t_idx + 1) as u8) else {
            continue;
        };
        let Some(next_sector) = next_track.get_sector_by_index(0) else {
            continue;
        };

        if next_sector.fdc_status2.0 == 64 {
            return Some(ProtectionResult::new(
                "Alkatraz CPC",
                format!("18 sector T{}", t_idx),
            ));
        }
    }

    None
}

fn detect_frontier(disk: &Disk) -> Option<ProtectionResult> {
    if disk.track_count() <= 10 {
        return None;
    }

    let track1 = get_track(disk, 1)?;
    if track1.sector_count() == 0 {
        return None;
    }

    let sector0 = get_sector(disk, 0, 0)?;
    if sector0.actual_size() <= 1 {
        return None;
    }

    // Signed version
    let t1_sector0 = track1.get_sector_by_index(0)?;
    if let Some(offset) = find_pattern(
        t1_sector0.data(),
        b"W DISK PROTECTION SYSTEM. (C) 1990 BY NEW FRONTIER SOFT.",
    ) {
        return Some(ProtectionResult::new(
            "Frontier",
            format!("signed T1/S0 +{}", offset),
        ));
    }

    // Unsigned version
    let track9 = get_track(disk, 9)?;
    if track9.sector_count() == 1 && sector0.actual_size() == 4096 && sector0.fdc_status1.0 == 0 {
        return Some(ProtectionResult::new(
            "Frontier",
            "probably, unsigned".to_string(),
        ));
    }

    None
}

fn detect_hexagon(disk: &Disk) -> Option<ProtectionResult> {
    let track0 = get_track(disk, 0)?;
    if track0.sector_count() != 10 || disk.track_count() <= 2 {
        return None;
    }

    let sector8 = track0.get_sector_by_index(8)?;
    if sector8.actual_size() != 512 {
        return None;
    }

    // Search first 4 tracks for signature
    for t_idx in 0..4.min(disk.track_count()) {
        let Some(track) = get_track(disk, t_idx as u8) else {
            continue;
        };

        // Signed versions
        for pattern in [
            &b"HEXAGON DISK PROTECTION c 1989"[..],
            &b"HEXAGON Disk Protection c 1989"[..],
        ] {
            if let Some((s_idx, offset)) = find_signature_in_track(track, pattern) {
                return Some(ProtectionResult::new(
                    "Hexagon",
                    format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                ));
            }
        }

        // Unsigned detection
        if track.sector_count() == 1 {
            let Some(sector) = track.get_sector_by_index(0) else {
                continue;
            };
            if sector.id.size_code == 6
                && sector.fdc_status1.0 == 32
                && sector.fdc_status2.0 == 96
            {
                return Some(ProtectionResult::new(
                    "Hexagon",
                    "probably, unsigned".to_string(),
                ));
            }
        }
    }

    None
}

fn detect_paul_owens(disk: &Disk) -> Option<ProtectionResult> {
    let track0 = get_track(disk, 0)?;
    if track0.sector_count() != 9 || disk.track_count() <= 10 {
        return None;
    }

    let track1 = get_track(disk, 1)?;
    if track1.sector_count() != 0 {
        return None;
    }

    let sector2 = track0.get_sector_by_index(2)?;

    // Build signature with embedded 0x80 byte
    let mut sig = b"PAUL OWENS".to_vec();
    sig.push(0x80);
    sig.extend_from_slice(b"PROTECTION SYS");

    if let Some(offset) = find_pattern(sector2.data(), &sig) {
        return Some(ProtectionResult::new(
            "Paul Owens",
            format!("signed T0/S2 +{}", offset),
        ));
    }

    // Unsigned version
    let track2 = get_track(disk, 2)?;
    if track2.sector_count() != 6 {
        return None;
    }

    let t2s0 = track2.get_sector_by_index(0)?;
    if t2s0.actual_size() == 256 {
        return Some(ProtectionResult::new(
            "Paul Owens",
            "probably, unsigned".to_string(),
        ));
    }

    None
}

fn detect_speedlock(disk: &Disk) -> Option<ProtectionResult> {
    // Signed versions - search all tracks
    let signatures = [
        ("Speedlock 1985", b"SPEEDLOCK PROTECTION SYSTEM (C) 1985 " as &[u8]),
        ("Speedlock 1986", b"SPEEDLOCK PROTECTION SYSTEM (C) 1986 "),
        ("Speedlock disc 1987", b"SPEEDLOCK DISC PROTECTION SYSTEMS COPYRIGHT 1987 "),
        ("Speedlock 1987 v2.1", b"SPEEDLOCK PROTECTION SYSTEM (C) 1987 D.LOOKER & D.AUBREY JONES : VERSION D/2.1"),
        ("Speedlock 1987", b"SPEEDLOCK PROTECTION SYSTEM (C) 1987 "),
        ("Speedlock +3 1987", b"SPEEDLOCK +3 DISC PROTECTION SYSTEM COPYRIGHT 1987 SPEEDLOCK ASSOCIATES"),
        ("Speedlock +3 1988", b"SPEEDLOCK +3 DISC PROTECTION SYSTEM COPYRIGHT 1988 SPEEDLOCK ASSOCIATES"),
        ("Speedlock 1988", b"SPEEDLOCK DISC PROTECTION SYSTEMS (C) 1988 SPEEDLOCK ASSOCIATES"),
        ("Speedlock 1989", b"SPEEDLOCK DISC PROTECTION SYSTEMS (C) 1989 SPEEDLOCK ASSOCIATES"),
        ("Speedlock 1990", b"SPEEDLOCK DISC PROTECTION SYSTEMS (C) 1990 SPEEDLOCK ASSOCIATES"),
    ];

    for (name, pattern) in signatures {
        if let Some((t_idx, s_idx, offset)) = find_signature_in_disk(disk, pattern) {
            return Some(ProtectionResult::new(
                name,
                format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
            ));
        }
    }

    // Unsigned Speedlock +3 1987/1988
    let track0 = get_track(disk, 0)?;
    if track0.sector_count() == 9 {
        if let Some(track1) = get_track(disk, 1) {
            if track1.sector_count() == 5 {
                if let Some(t1s0) = track1.get_sector_by_index(0) {
                    if t1s0.actual_size() == 1024 {
                        let s6 = track0.get_sector_by_index(6)?;
                        let s8 = track0.get_sector_by_index(8)?;

                        if s6.fdc_status2.0 == 64 && s8.fdc_status2.0 == 0 {
                            return Some(ProtectionResult::new(
                                "Speedlock +3 1987",
                                "probably, unsigned".to_string(),
                            ));
                        }
                        if s6.fdc_status2.0 == 64 && s8.fdc_status2.0 == 64 {
                            return Some(ProtectionResult::new(
                                "Speedlock +3 1988",
                                "probably, unsigned".to_string(),
                            ));
                        }
                    }
                }
            }
        }
    }

    // Unsigned Speedlock 1989/1990
    if track0.sector_count() > 7 && disk.track_count() > 40 {
        if let Some(track1) = get_track(disk, 1) {
            if track1.sector_count() == 1 {
                if let Some(sector) = track1.get_sector_by_index(0) {
                    if sector.id.sector == 193 && sector.fdc_status1.0 == 32 {
                        return Some(ProtectionResult::new(
                            "Speedlock 1989/1990",
                            "probably, unsigned".to_string(),
                        ));
                    }
                }
            }
        }
    }

    None
}

fn detect_three_inch_loader(disk: &Disk) -> Option<ProtectionResult> {
    let track0 = get_track(disk, 0)?;
    let sector0 = track0.get_sector_by_index(0)?;

    // Type 1
    if let Some(offset) = find_pattern(
        sector0.data(),
        b"***Loader Copyright Three Inch Software 1988, All Rights Reserved. Three Inch Software, 73 Surbiton Road, Kingston upon Thames, KT1 2HG***",
    ) {
        return Some(ProtectionResult::new(
            "Three Inch Loader type 1",
            format!("signed T0/S0 +{}", offset),
        ));
    }

    // Type 1-0-7
    if track0.sector_count() > 7 {
        if let Some(sector7) = track0.get_sector_by_index(7) {
            if let Some(offset) = find_pattern(
                sector7.data(),
                b"***Loader Copyright Three Inch Software 1988, All Rights Reserved. Three Inch Software, 73 Surbiton Road, Kingston upon Thames, KT1 2HG***",
            ) {
                return Some(ProtectionResult::new(
                    "Three Inch Loader type 1-0-7",
                    format!("signed T0/S7 +{}", offset),
                ));
            }
        }
    }

    // Type 2
    if let Some(offset) = find_pattern(
        sector0.data(),
        b"***Loader Copyright Three Inch Software 1988, All Rights Reserved. 01-546 2754",
    ) {
        return Some(ProtectionResult::new(
            "Three Inch Loader type 2",
            format!("signed T0/S0 +{}", offset),
        ));
    }

    // Type 3-1-4 (Microprose Soccer)
    if disk.track_count() > 1 {
        if let Some(track1) = get_track(disk, 1) {
            if track1.sector_count() > 4 {
                if let Some(sector4) = track1.get_sector_by_index(4) {
                    let mut sig = b"Loader ".to_vec();
                    sig.push(0x7F);
                    sig.extend_from_slice(b"1988 Three Inch Software");

                    if let Some(offset) = find_pattern(sector4.data(), &sig) {
                        return Some(ProtectionResult::new(
                            "Three Inch Loader type 3-1-4",
                            format!("signed T1/S4 +{}", offset),
                        ));
                    }
                }
            }
        }
    }

    None
}

fn detect_laser_load(disk: &Disk) -> Option<ProtectionResult> {
    let track0 = get_track(disk, 0)?;
    if track0.sector_count() <= 2 {
        return None;
    }

    let sector2 = track0.get_sector_by_index(2)?;
    let offset = find_pattern(
        sector2.data(),
        b"Laser Load   By C.J.Pink For Consult Computer    Systems",
    )?;

    Some(ProtectionResult::new(
        "Laser Load by C.J. Pink",
        format!("signed T0/S2 +{}", offset),
    ))
}

fn detect_wrm(disk: &Disk) -> Option<ProtectionResult> {
    if disk.track_count() <= 9 {
        return None;
    }

    let track8 = get_track(disk, 8)?;
    if track8.sector_count() <= 9 {
        return None;
    }

    let sector9 = track8.get_sector_by_index(9)?;
    if sector9.actual_size() <= 128 {
        return None;
    }

    let data = sector9.data();
    if find_pattern(data, b"W.R.M Disc").map(|o| o == 0).unwrap_or(false)
        && find_pattern(data, b"Protection").is_some()
        && find_pattern(data, b"System (c) 1987").is_some()
    {
        return Some(ProtectionResult::new(
            "W.R.M Disc Protection",
            "signed T8/S9 +0".to_string(),
        ));
    }

    None
}

fn detect_pms(disk: &Disk) -> Option<ProtectionResult> {
    let sector0 = get_sector(disk, 0, 0)?;

    let signatures = [
        ("P.M.S. 1986", b"[C] P.M.S. 1986" as &[u8]),
        ("P.M.S. Loader 1986 v1", b"P.M.S. LOADER [C]1986"),
        ("P.M.S. Loader 1986 v2", b"P.M.S.LOADER [C]1986"),
        ("P.M.S. 1987", b"P.M.S.LOADER [C]1987"),
    ];

    for (name, pattern) in signatures {
        if let Some(offset) = find_pattern(sector0.data(), pattern) {
            return Some(ProtectionResult::new(
                name,
                format!("signed T0/S0 +{}", offset),
            ));
        }
    }

    // Unsigned P.M.S.
    if disk.track_count() > 2 {
        let track0 = get_track(disk, 0)?;
        let track1 = get_track(disk, 1)?;
        let track2 = get_track(disk, 2)?;

        if !track0.is_empty() && track1.is_empty() && !track2.is_empty() {
            return Some(ProtectionResult::new(
                "P.M.S. Loader 1986/1987",
                "maybe, unsigned".to_string(),
            ));
        }
    }

    None
}

fn detect_players(disk: &Disk) -> Option<ProtectionResult> {
    for t_idx in 0..disk.track_count() {
        let Some(track) = get_track(disk, t_idx as u8) else {
            continue;
        };

        if track.sector_count() != 16 {
            continue;
        }

        let is_players = (0..16).all(|s_idx| {
            track
                .get_sector_by_index(s_idx)
                .map(|s| s.id.sector == s_idx as u8 && s.id.size_code == s_idx as u8)
                .unwrap_or(false)
        });

        if is_players {
            let largest = get_largest_track_size(disk);
            return Some(ProtectionResult::new(
                "Players",
                format!("maybe, super-sized {} byte track {}", largest, t_idx),
            ));
        }
    }

    None
}

fn detect_infogrames(disk: &Disk) -> Option<ProtectionResult> {
    if disk.track_count() <= 39 {
        return None;
    }

    let track39 = get_track(disk, 39)?;
    if track39.sector_count() != 9 {
        return None;
    }

    for s_idx in 0..track39.sector_count() {
        let Some(sector) = track39.get_sector_by_index(s_idx) else {
            continue;
        };
        if sector.id.size_code == 2 && sector.actual_size() == 540 {
            return Some(ProtectionResult::new(
                "Infogrames/Logiciel",
                format!("gap data sector T39/S{}", s_idx),
            ));
        }
    }

    None
}

fn detect_rainbow_arts(disk: &Disk) -> Option<ProtectionResult> {
    if disk.track_count() <= 40 {
        return None;
    }

    let track40 = get_track(disk, 40)?;
    if track40.sector_count() != 9 {
        return None;
    }

    for s_idx in 0..track40.sector_count() {
        let Some(sector) = track40.get_sector_by_index(s_idx) else {
            continue;
        };
        if sector.id.sector == 198 && sector.fdc_status1.0 == 32 && sector.fdc_status2.0 == 32 {
            return Some(ProtectionResult::new(
                "Rainbow Arts",
                format!("weak sector T40/S{}", s_idx),
            ));
        }
    }

    None
}

fn detect_herbulot(disk: &Disk) -> Option<ProtectionResult> {
    let track0 = get_track(disk, 0)?;
    if track0.sector_count() <= 6 {
        return None;
    }

    let signatures = [
        ("ERE/Remi HERBULOT", b"PROTECTION      Remi HERBULOT" as &[u8]),
        ("ERE/Remi HERBULOT 2.1", b"PROTECTION  V2.1Remi HERBULOT"),
    ];

    for (name, pattern) in signatures {
        if let Some((s_idx, offset)) = find_signature_in_track(track0, pattern) {
            return Some(ProtectionResult::new(
                name,
                format!("signed T0/S{} +{}", s_idx, offset),
            ));
        }
    }

    None
}

fn detect_kbi(disk: &Disk) -> Option<ProtectionResult> {
    // KBI-19 and CAAV (19 sector tracks)
    let mut last_kbi_track: Option<usize> = None;

    for t_idx in 0..disk.track_count() {
        let Some(track) = get_track(disk, t_idx as u8) else {
            continue;
        };

        if track.sector_count() != 19 {
            continue;
        }

        last_kbi_track = Some(t_idx);

        if let Some(sector1) = track.get_sector_by_index(1) {
            if let Some(offset) = find_pattern(sector1.data(), b"(c) 1986 for KBI ") {
                return Some(ProtectionResult::new(
                    "KBI-19",
                    format!("signed T{}/S1 +{}", t_idx, offset),
                ));
            }
        }

        if let Some(sector0) = track.get_sector_by_index(0) {
            if let Some(offset) = find_pattern(sector0.data(), b"ALAIN LAURENT GENERATION 5 1989")
            {
                return Some(ProtectionResult::new(
                    "CAAV",
                    format!("signed T{}/S0 +{}", t_idx, offset),
                ));
            }
        }
    }

    if let Some(track_num) = last_kbi_track {
        return Some(ProtectionResult::new(
            "KBI-19 or CAAV",
            format!("probably, unsigned track {}", track_num),
        ));
    }

    // KBI-10
    if disk.track_count() >= 40 {
        let track38 = get_track(disk, 38)?;
        let track39 = get_track(disk, 39)?;

        if track39.sector_count() == 10 && track38.sector_count() == 9 {
            let sector9 = track39.get_sector_by_index(9)?;
            if sector9.fdc_status1.0 == 32 && sector9.fdc_status2.0 == 32 {
                return Some(ProtectionResult::new("KBI-10", "weak sector T39/S9".to_string()));
            }
        }
    }

    None
}

fn detect_discsys(disk: &Disk) -> Option<ProtectionResult> {
    let mut discsys_track: Option<usize> = None;

    for t_idx in 0..disk.track_count() {
        let Some(track) = get_track(disk, t_idx as u8) else {
            continue;
        };

        if track.sector_count() != 16 {
            continue;
        }

        let is_discsys = (0..16).all(|s_idx| {
            track
                .get_sector_by_index(s_idx)
                .map(|s| {
                    s.id.sector == s_idx as u8
                        && s.id.track == s_idx as u8
                        && s.id.side == s_idx as u8
                        && s.id.size_code == s_idx as u8
                })
                .unwrap_or(false)
        });

        if is_discsys {
            discsys_track = Some(t_idx);
        }
    }

    let track_num = discsys_track?;

    // Check for Mean Protection System first (if discsys is on track 1)
    if track_num == 1 {
        let track0 = get_track(disk, 0)?;
        if let Some((s_idx, offset)) = find_signature_in_track(track0, b"MEAN PROTECTION SYSTEM") {
            return Some(ProtectionResult::new(
                "Mean Protection System",
                format!("signed T0S{} +{}", s_idx, offset),
            ));
        }
    }

    let mut result = format!("DiscSYS on track {}", track_num);

    // Try to extract version info from track 2 sector 4
    if let Some(track2) = get_track(disk, 2) {
        if let Some(sector4) = track2.get_sector_by_index(4) {
            if sector4.actual_size() > 160 {
                let data = sector4.data();
                if data.len() > 107 {
                    let start = 85.min(data.len());
                    let end = (start + 22).min(data.len());
                    let extracted: String = data[start..end]
                        .iter()
                        .filter(|&&b| b >= 32 && b < 127)
                        .map(|&b| b as char)
                        .collect();
                    let cleaned = extracted.trim().to_lowercase();

                    if cleaned.starts_with("discsys") && cleaned.len() > 8 {
                        result = format!("{} ({})", result, &cleaned[8..].trim());
                    } else if cleaned.starts_with("multi-") {
                        result = format!("{} ({})", result, cleaned);
                    }
                }
            }
        }
    }

    Some(ProtectionResult::new("DiscSYS", result))
}

fn detect_amsoft_exopal(disk: &Disk) -> Option<ProtectionResult> {
    if disk.track_count() <= 3 {
        return None;
    }

    let track3 = get_track(disk, 3)?;
    let sector0 = track3.get_sector_by_index(0)?;

    if sector0.actual_size() != 512 {
        return None;
    }

    let data = sector0.data();
    let amsoft_offset = find_pattern(data, b"Amsoft disc protection system")?;

    if amsoft_offset <= 1 {
        return None;
    }

    let offset = find_pattern(data, b"EXOPAL")?;
    Some(ProtectionResult::new(
        "Amsoft/EXOPAL",
        format!("signed T3S0 +{}", offset),
    ))
}

fn detect_armourloc(disk: &Disk) -> Option<ProtectionResult> {
    let track0 = get_track(disk, 0)?;
    if track0.sector_count() != 9 {
        return None;
    }

    let sector0 = track0.get_sector_by_index(0)?;
    if find_pattern(sector0.data(), b"0K free") == Some(2) {
        return Some(ProtectionResult::new(
            "ARMOURLOC",
            "anti-hacker protection".to_string(),
        ));
    }

    None
}

fn detect_studio_b_discloc(disk: &Disk) -> Option<ProtectionResult> {
    if disk.track_count() <= 3 {
        return None;
    }

    let track0 = get_track(disk, 0)?;
    let track1 = get_track(disk, 1)?;
    let track2 = get_track(disk, 2)?;

    if track0.is_empty() || !track1.is_empty() || track2.is_empty() {
        return None;
    }

    let sector0 = track0.get_sector_by_index(0)?;
    if let Some(offset) = find_pattern(sector0.data(), b"Disc format (c) 1986 Studio B Ltd.") {
        return Some(ProtectionResult::new(
            "Studio B Disc format",
            format!("signed T0S0 +{}", offset),
        ));
    }

    let t2s0 = track2.get_sector_by_index(0)?;
    if let Some(offset) = find_pattern(t2s0.data(), b"DISCLOC") {
        return Some(ProtectionResult::new(
            "DiscLoc/Oddball",
            format!("signed T2S0 +{}", offset),
        ));
    }

    None
}

// ============================================================================
// Main detection function
// ============================================================================

/// Detect copy protection on a disk side
///
/// Returns `Some(ProtectionResult)` if a protection scheme is detected,
/// or `None` if the disk appears to be unprotected.
///
/// # Arguments
///
/// * `disk` - The disk (side) to analyze
///
/// # Example
///
/// ```no_run
/// use dskmanager::{DskImage, protection};
///
/// let image = DskImage::open("game.dsk")?;
/// if let Some(disk) = image.get_disk(0) {
///     if let Some(result) = protection::detect(disk) {
///         println!("Protection: {}", result);
///     } else {
///         println!("No protection detected");
///     }
/// }
/// # Ok::<(), dskmanager::DskError>(())
/// ```
pub fn detect(disk: &Disk) -> Option<ProtectionResult> {
    // Basic sanity checks
    if disk.track_count() < 2 {
        return None;
    }

    let track0 = disk.get_track(0)?;
    if track0.sector_count() < 1 {
        return None;
    }

    let sector0 = track0.get_sector_by_index(0)?;
    if sector0.actual_size() < 128 {
        return None;
    }

    // If disk is uniform and has no FDC errors, it's not protected
    if is_uniform(disk) && !has_fdc_errors(disk) {
        return None;
    }

    // Try each detector in order - return on first match
    let detectors: &[fn(&Disk) -> Option<ProtectionResult>] = &[
        detect_alkatraz,
        detect_frontier,
        detect_hexagon,
        detect_paul_owens,
        detect_speedlock,
        detect_three_inch_loader,
        detect_laser_load,
        detect_wrm,
        detect_pms,
        detect_players,
        detect_infogrames,
        detect_rainbow_arts,
        detect_herbulot,
        detect_kbi,
        detect_discsys,
        detect_amsoft_exopal,
        detect_armourloc,
        detect_studio_b_discloc,
    ];

    for detector in detectors {
        if let Some(result) = detector(disk) {
            return Some(result);
        }
    }

    // Unknown copy protection - disk is non-uniform or has FDC errors
    if !is_uniform(disk) && has_fdc_errors(disk) {
        return Some(ProtectionResult::new(
            "Unknown copy protection",
            "non-uniform disk with FDC errors".to_string(),
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{Sector, SectorId, Track};

    #[test]
    fn test_find_pattern() {
        let data = b"Hello SPEEDLOCK PROTECTION SYSTEM world";
        assert!(find_pattern(data, b"SPEEDLOCK").is_some());
        assert_eq!(find_pattern(data, b"SPEEDLOCK"), Some(6));
        assert!(find_pattern(data, b"NOTFOUND").is_none());
    }

    #[test]
    fn test_uniform_disk_no_protection() {
        let mut disk = Disk::new(0);

        // Create 40 uniform tracks
        for t in 0..40 {
            let mut track = Track::new(t, 0);
            for s in 0..9 {
                let id = SectorId::new(t, 0, 0xC1 + s, 2);
                track.add_sector(Sector::new(id));
            }
            disk.add_track(track);
        }

        assert!(is_uniform(&disk));
        assert!(!has_fdc_errors(&disk));
        assert!(detect(&disk).is_none());
    }

    #[test]
    fn test_protection_result_display() {
        let result = ProtectionResult::new("Speedlock 1987", "signed T0/S0 +42");
        assert_eq!(result.to_string(), "Speedlock 1987 (signed T0/S0 +42)");
    }
}
