/// Copy protection detection for DSK disk images
///
/// Detects various copy protection schemes used on Amstrad CPC, ZX Spectrum +3,
/// and other systems that used the DSK format.

use crate::image::Disk;

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

    // Alkatraz CPC (18 sector track)
    for t_idx in 0..disk.track_count().saturating_sub(1) {
        if let Some(track) = disk.get_track(t_idx as u8) {
            if track.sector_count() == 18 {
                if let Some(sector) = track.get_sector_by_index(0) {
                    if sector.actual_size() == 256 {
                        return Some(ProtectionResult::new(
                            "Alkatraz CPC",
                            format!("18 sector T{}", t_idx),
                        ));
                    }
                }
            }
        }
    }

    // Alkatraz CPC (18 sector with FDC status check)
    for t_idx in 0..disk.track_count().saturating_sub(1) {
        if let Some(track) = disk.get_track(t_idx as u8) {
            if track.sector_count() == 18 {
                if let Some(sector) = track.get_sector_by_index(0) {
                    if sector.advertised_size() == 256 {
                        if let Some(next_track) = disk.get_track((t_idx + 1) as u8) {
                            if next_track.sector_count() > 0 {
                                if let Some(next_sector) = next_track.get_sector_by_index(0) {
                                    if next_sector.fdc_status2.0 == 64 {
                                        return Some(ProtectionResult::new(
                                            "Alkatraz CPC",
                                            format!("18 sector T{}", t_idx),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Frontier copy-protection
    if disk.track_count() > 10 {
        if let Some(track1) = disk.get_track(1) {
            if track1.sector_count() > 0 && sector0.actual_size() > 1 {
                if let Some(t1_sector0) = track1.get_sector_by_index(0) {
                    if let Some(offset) = find_pattern(
                        t1_sector0.data(),
                        b"W DISK PROTECTION SYSTEM. (C) 1990 BY NEW FRONTIER SOFT.",
                    ) {
                        return Some(ProtectionResult::new(
                            "Frontier",
                            format!("signed T1/S0 +{}", offset),
                        ));
                    }
                }

                if let Some(track9) = disk.get_track(9) {
                    if track9.sector_count() == 1
                        && sector0.actual_size() == 4096
                        && sector0.fdc_status1.0 == 0
                    {
                        return Some(ProtectionResult::new(
                            "Frontier",
                            "probably, unsigned".to_string(),
                        ));
                    }
                }
            }
        }
    }

    // Hexagon
    if track0.sector_count() == 10 && disk.track_count() > 2 {
        if let Some(sector8) = track0.get_sector_by_index(8) {
            if sector8.actual_size() == 512 {
                // Search first 4 tracks for signature
                for t_idx in 0..4.min(disk.track_count()) {
                    if let Some(track) = disk.get_track(t_idx as u8) {
                        for s_idx in 0..track.sector_count() {
                            if let Some(sector) = track.get_sector_by_index(s_idx) {
                                if let Some(offset) = find_pattern(
                                    sector.data(),
                                    b"HEXAGON DISK PROTECTION c 1989",
                                ) {
                                    return Some(ProtectionResult::new(
                                        "Hexagon",
                                        format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                                    ));
                                }
                                if let Some(offset) = find_pattern(
                                    sector.data(),
                                    b"HEXAGON Disk Protection c 1989",
                                ) {
                                    return Some(ProtectionResult::new(
                                        "Hexagon",
                                        format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                                    ));
                                }
                            }
                        }

                        // Unsigned detection
                        if !track.is_empty() {
                            if track.sector_count() == 1 {
                                if let Some(sector) = track.get_sector_by_index(0) {
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
                        }
                    }
                }
            }
        }
    }

    // Paul Owens
    if track0.sector_count() == 9 && disk.track_count() > 10 {
        if let Some(track1) = disk.get_track(1) {
            if track1.sector_count() == 0 {
                if let Some(sector2) = track0.get_sector_by_index(2) {
                    // Build the signature with embedded 0x80 byte
                    let mut sig = b"PAUL OWENS".to_vec();
                    sig.push(0x80);
                    sig.extend_from_slice(b"PROTECTION SYS");

                    if let Some(offset) = find_pattern(sector2.data(), &sig) {
                        return Some(ProtectionResult::new(
                            "Paul Owens",
                            format!("signed T0/S2 +{}", offset),
                        ));
                    }

                    if let Some(track2) = disk.get_track(2) {
                        if track2.sector_count() == 6 {
                            if let Some(t2s0) = track2.get_sector_by_index(0) {
                                if t2s0.actual_size() == 256 {
                                    return Some(ProtectionResult::new(
                                        "Paul Owens",
                                        "probably, unsigned".to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Speedlock signatures - search all tracks
    for t_idx in 0..disk.track_count() {
        if let Some(track) = disk.get_track(t_idx as u8) {
            for s_idx in 0..track.sector_count() {
                if let Some(sector) = track.get_sector_by_index(s_idx) {
                    let data = sector.data();

                    // Speedlock 1985 (CPC)
                    if let Some(offset) =
                        find_pattern(data, b"SPEEDLOCK PROTECTION SYSTEM (C) 1985 ")
                    {
                        return Some(ProtectionResult::new(
                            "Speedlock 1985",
                            format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                        ));
                    }

                    // Speedlock 1986 (CPC)
                    if let Some(offset) =
                        find_pattern(data, b"SPEEDLOCK PROTECTION SYSTEM (C) 1986 ")
                    {
                        return Some(ProtectionResult::new(
                            "Speedlock 1986",
                            format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                        ));
                    }

                    // Speedlock disc 1987
                    if let Some(offset) =
                        find_pattern(data, b"SPEEDLOCK DISC PROTECTION SYSTEMS COPYRIGHT 1987 ")
                    {
                        return Some(ProtectionResult::new(
                            "Speedlock disc 1987",
                            format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                        ));
                    }

                    // Speedlock 1987 v2.1
                    if let Some(offset) = find_pattern(
                        data,
                        b"SPEEDLOCK PROTECTION SYSTEM (C) 1987 D.LOOKER & D.AUBREY JONES : VERSION D/2.1",
                    ) {
                        return Some(ProtectionResult::new(
                            "Speedlock 1987 v2.1",
                            format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                        ));
                    }

                    // Speedlock 1987 (CPC)
                    if let Some(offset) =
                        find_pattern(data, b"SPEEDLOCK PROTECTION SYSTEM (C) 1987 ")
                    {
                        return Some(ProtectionResult::new(
                            "Speedlock 1987",
                            format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                        ));
                    }

                    // Speedlock +3 1987
                    if let Some(offset) = find_pattern(
                        data,
                        b"SPEEDLOCK +3 DISC PROTECTION SYSTEM COPYRIGHT 1987 SPEEDLOCK ASSOCIATES",
                    ) {
                        return Some(ProtectionResult::new(
                            "Speedlock +3 1987",
                            format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                        ));
                    }

                    // Speedlock +3 1988
                    if let Some(offset) = find_pattern(
                        data,
                        b"SPEEDLOCK +3 DISC PROTECTION SYSTEM COPYRIGHT 1988 SPEEDLOCK ASSOCIATES",
                    ) {
                        return Some(ProtectionResult::new(
                            "Speedlock +3 1988",
                            format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                        ));
                    }

                    // Speedlock 1988
                    if let Some(offset) = find_pattern(
                        data,
                        b"SPEEDLOCK DISC PROTECTION SYSTEMS (C) 1988 SPEEDLOCK ASSOCIATES",
                    ) {
                        return Some(ProtectionResult::new(
                            "Speedlock 1988",
                            format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                        ));
                    }

                    // Speedlock 1989
                    if let Some(offset) = find_pattern(
                        data,
                        b"SPEEDLOCK DISC PROTECTION SYSTEMS (C) 1989 SPEEDLOCK ASSOCIATES",
                    ) {
                        return Some(ProtectionResult::new(
                            "Speedlock 1989",
                            format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                        ));
                    }

                    // Speedlock 1990
                    if let Some(offset) = find_pattern(
                        data,
                        b"SPEEDLOCK DISC PROTECTION SYSTEMS (C) 1990 SPEEDLOCK ASSOCIATES",
                    ) {
                        return Some(ProtectionResult::new(
                            "Speedlock 1990",
                            format!("signed T{}/S{} +{}", t_idx, s_idx, offset),
                        ));
                    }
                }
            }
        }
    }

    // Unsigned Speedlock +3 1987
    if track0.sector_count() == 9 {
        if let Some(track1) = disk.get_track(1) {
            if track1.sector_count() == 5 {
                if let Some(t1s0) = track1.get_sector_by_index(0) {
                    if t1s0.actual_size() == 1024 {
                        if let (Some(s6), Some(s8)) = (
                            track0.get_sector_by_index(6),
                            track0.get_sector_by_index(8),
                        ) {
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
    }

    // Unsigned Speedlock 1989/1990
    if track0.sector_count() > 7 && disk.track_count() > 40 {
        if let Some(track1) = disk.get_track(1) {
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

    // Three Inch Loader type 1
    if let Some(offset) = find_pattern(
        sector0.data(),
        b"***Loader Copyright Three Inch Software 1988, All Rights Reserved. Three Inch Software, 73 Surbiton Road, Kingston upon Thames, KT1 2HG***",
    ) {
        return Some(ProtectionResult::new(
            "Three Inch Loader type 1",
            format!("signed T0/S0 +{}", offset),
        ));
    }

    // Three Inch Loader type 1-0-7
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

    // Three Inch Loader type 2
    if let Some(offset) = find_pattern(
        sector0.data(),
        b"***Loader Copyright Three Inch Software 1988, All Rights Reserved. 01-546 2754",
    ) {
        return Some(ProtectionResult::new(
            "Three Inch Loader type 2",
            format!("signed T0/S0 +{}", offset),
        ));
    }

    // Three Inch Loader type 3-1-4 (Microprose Soccer)
    if disk.track_count() > 1 {
        if let Some(track1) = disk.get_track(1) {
            if track1.sector_count() > 4 {
                if let Some(sector4) = track1.get_sector_by_index(4) {
                    // Build signature with embedded 0x7F byte
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

    // Laser loader (War in Middle Earth CPC)
    if track0.sector_count() > 2 {
        if let Some(sector2) = track0.get_sector_by_index(2) {
            if let Some(offset) = find_pattern(
                sector2.data(),
                b"Laser Load   By C.J.Pink For Consult Computer    Systems",
            ) {
                return Some(ProtectionResult::new(
                    "Laser Load by C.J. Pink",
                    format!("signed T0/S2 +{}", offset),
                ));
            }
        }
    }

    // W.R.M. (Martech)
    if disk.track_count() > 9 {
        if let Some(track8) = disk.get_track(8) {
            if track8.sector_count() > 9 {
                if let Some(sector9) = track8.get_sector_by_index(9) {
                    if sector9.actual_size() > 128 {
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
                    }
                }
            }
        }
    }

    // P.M.S. signatures
    if let Some(offset) = find_pattern(sector0.data(), b"[C] P.M.S. 1986") {
        return Some(ProtectionResult::new(
            "P.M.S. 1986",
            format!("signed T0/S0 +{}", offset),
        ));
    }

    if let Some(offset) = find_pattern(sector0.data(), b"P.M.S. LOADER [C]1986") {
        return Some(ProtectionResult::new(
            "P.M.S. Loader 1986 v1",
            format!("signed T0/S0 +{}", offset),
        ));
    }

    if let Some(offset) = find_pattern(sector0.data(), b"P.M.S.LOADER [C]1986") {
        return Some(ProtectionResult::new(
            "P.M.S. Loader 1986 v2",
            format!("signed T0/S0 +{}", offset),
        ));
    }

    if let Some(offset) = find_pattern(sector0.data(), b"P.M.S.LOADER [C]1987") {
        return Some(ProtectionResult::new(
            "P.M.S. 1987",
            format!("signed T0/S0 +{}", offset),
        ));
    }

    // Unsigned P.M.S.
    if disk.track_count() > 2 {
        if let (Some(track1), Some(track2)) = (disk.get_track(1), disk.get_track(2)) {
            if !track0.is_empty() && track1.is_empty() && !track2.is_empty() {
                return Some(ProtectionResult::new(
                    "P.M.S. Loader 1986/1987",
                    "maybe, unsigned".to_string(),
                ));
            }
        }
    }

    // Players
    for t_idx in 0..disk.track_count() {
        if let Some(track) = disk.get_track(t_idx as u8) {
            if track.sector_count() == 16 {
                let mut is_players = true;
                for s_idx in 0..track.sector_count() {
                    if let Some(sector) = track.get_sector_by_index(s_idx) {
                        if sector.id.sector != s_idx as u8 || sector.id.size_code != s_idx as u8 {
                            is_players = false;
                            break;
                        }
                    } else {
                        is_players = false;
                        break;
                    }
                }
                if is_players {
                    let largest = get_largest_track_size(disk);
                    return Some(ProtectionResult::new(
                        "Players",
                        format!("maybe, super-sized {} byte track {}", largest, t_idx),
                    ));
                }
            }
        }
    }

    // Infogrames / Loriciel Gap
    if disk.track_count() > 39 {
        if let Some(track39) = disk.get_track(39) {
            if track39.sector_count() == 9 {
                for s_idx in 0..track39.sector_count() {
                    if let Some(sector) = track39.get_sector_by_index(s_idx) {
                        if sector.id.size_code == 2 && sector.actual_size() == 540 {
                            return Some(ProtectionResult::new(
                                "Infogrames/Logiciel",
                                format!("gap data sector T39/S{}", s_idx),
                            ));
                        }
                    }
                }
            }
        }
    }

    // Rainbow Arts weak sector
    if disk.track_count() > 40 {
        if let Some(track40) = disk.get_track(40) {
            if track40.sector_count() == 9 {
                for s_idx in 0..track40.sector_count() {
                    if let Some(sector) = track40.get_sector_by_index(s_idx) {
                        if sector.id.sector == 198
                            && sector.fdc_status1.0 == 32
                            && sector.fdc_status2.0 == 32
                        {
                            return Some(ProtectionResult::new(
                                "Rainbow Arts",
                                format!("weak sector T40/S{}", s_idx),
                            ));
                        }
                    }
                }
            }
        }
    }

    // ERE/Remi HERBULOT - note: this can combine with others in Pascal, but we'll return it
    if track0.sector_count() > 6 {
        for s_idx in 0..track0.sector_count() {
            if let Some(sector) = track0.get_sector_by_index(s_idx) {
                if let Some(offset) =
                    find_pattern(sector.data(), b"PROTECTION      Remi HERBULOT")
                {
                    return Some(ProtectionResult::new(
                        "ERE/Remi HERBULOT",
                        format!("signed T0/S{} +{}", s_idx, offset),
                    ));
                }

                if let Some(offset) =
                    find_pattern(sector.data(), b"PROTECTION  V2.1Remi HERBULOT")
                {
                    return Some(ProtectionResult::new(
                        "ERE/Remi HERBULOT 2.1",
                        format!("signed T0/S{} +{}", s_idx, offset),
                    ));
                }
            }
        }
    }

    // KBI-19 and CAAV
    let mut last_kbi_track: Option<usize> = None;
    for t_idx in 0..disk.track_count() {
        if let Some(track) = disk.get_track(t_idx as u8) {
            if track.sector_count() == 19 {
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
                    if let Some(offset) =
                        find_pattern(sector0.data(), b"ALAIN LAURENT GENERATION 5 1989")
                    {
                        return Some(ProtectionResult::new(
                            "CAAV",
                            format!("signed T{}/S0 +{}", t_idx, offset),
                        ));
                    }
                }
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
        if let (Some(track38), Some(track39)) = (disk.get_track(38), disk.get_track(39)) {
            if track39.sector_count() == 10 && track38.sector_count() == 9 {
                if let Some(sector9) = track39.get_sector_by_index(9) {
                    if sector9.fdc_status1.0 == 32 && sector9.fdc_status2.0 == 32 {
                        return Some(ProtectionResult::new("KBI-10", "weak sector T39/S9".to_string()));
                    }
                }
            }
        }
    }

    // DiscSYS
    let mut discsys_track: Option<usize> = None;
    for t_idx in 0..disk.track_count() {
        if let Some(track) = disk.get_track(t_idx as u8) {
            if track.sector_count() == 16 {
                let mut is_discsys = true;
                for s_idx in 0..track.sector_count() {
                    if let Some(sector) = track.get_sector_by_index(s_idx) {
                        if sector.id.sector != s_idx as u8
                            || sector.id.track != s_idx as u8
                            || sector.id.side != s_idx as u8
                            || sector.id.size_code != s_idx as u8
                        {
                            is_discsys = false;
                            break;
                        }
                    } else {
                        is_discsys = false;
                        break;
                    }
                }
                if is_discsys {
                    discsys_track = Some(t_idx);
                }
            }
        }
    }

    if let Some(track_num) = discsys_track {
        let mut result = format!("DiscSYS on track {}", track_num);

        // Try to extract version info from track 2 sector 4
        if disk.track_count() > 2 {
            if let Some(track2) = disk.get_track(2) {
                if track2.sector_count() > 4 {
                    if let Some(sector4) = track2.get_sector_by_index(4) {
                        if sector4.actual_size() > 160 {
                            let data = sector4.data();
                            if data.len() > 77 {
                                // Extract and clean string from offset 85 (0-indexed: 85), length 22
                                // Adjusting for 0-based indexing
                                let start = 85.min(data.len());
                                let end = (start + 22).min(data.len());
                                let extracted: String = data[start..end]
                                    .iter()
                                    .filter(|&&b| b >= 32 && b < 127)
                                    .map(|&b| b as char)
                                    .collect();
                                let cleaned = extracted.trim().to_lowercase();

                                if cleaned.starts_with("discsys") {
                                    result = format!("{} ({})", result, &cleaned[8..].trim());
                                } else if cleaned.starts_with("multi-") {
                                    result = format!("{} ({})", result, cleaned);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Mean Protection System check
        if track_num == 1 {
            for s_idx in 0..track0.sector_count() {
                if let Some(sector) = track0.get_sector_by_index(s_idx) {
                    if let Some(offset) = find_pattern(sector.data(), b"MEAN PROTECTION SYSTEM") {
                        return Some(ProtectionResult::new(
                            "Mean Protection System",
                            format!("signed T0S{} +{}", s_idx, offset),
                        ));
                    }
                }
            }
        }

        return Some(ProtectionResult::new("DiscSYS", result));
    }

    // Amsoft/EXOPAL
    if disk.track_count() > 3 {
        if let Some(track3) = disk.get_track(3) {
            if track3.sector_count() > 0 {
                if let Some(sector0) = track3.get_sector_by_index(0) {
                    if sector0.actual_size() == 512 {
                        let data = sector0.data();
                        if find_pattern(data, b"Amsoft disc protection system")
                            .map(|o| o > 1)
                            .unwrap_or(false)
                        {
                            if let Some(offset) = find_pattern(data, b"EXOPAL") {
                                return Some(ProtectionResult::new(
                                    "Amsoft/EXOPAL",
                                    format!("signed T3S0 +{}", offset),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // ARMOURLOC
    if track0.sector_count() == 9 {
        if find_pattern(sector0.data(), b"0K free")
            .map(|o| o == 2)
            .unwrap_or(false)
        {
            return Some(ProtectionResult::new(
                "ARMOURLOC",
                "anti-hacker protection".to_string(),
            ));
        }
    }

    // Studio B / DiscLoc/Oddball
    if disk.track_count() > 3 {
        if let (Some(track1), Some(track2)) = (disk.get_track(1), disk.get_track(2)) {
            if !track0.is_empty() && track1.is_empty() && !track2.is_empty() {
                if let Some(offset) =
                    find_pattern(sector0.data(), b"Disc format (c) 1986 Studio B Ltd.")
                {
                    return Some(ProtectionResult::new(
                        "Studio B Disc format",
                        format!("signed T0S0 +{}", offset),
                    ));
                }

                if let Some(t2s0) = track2.get_sector_by_index(0) {
                    if let Some(offset) = find_pattern(t2s0.data(), b"DISCLOC") {
                        return Some(ProtectionResult::new(
                            "DiscLoc/Oddball",
                            format!("signed T2S0 +{}", offset),
                        ));
                    }
                }
            }
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
        if let Some(track) = disk.get_track(t_idx as u8) {
            for s_idx in 0..track.sector_count() {
                if let Some(sector) = track.get_sector_by_index(s_idx) {
                    if sector.has_error() {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Get the largest track size in bytes
fn get_largest_track_size(disk: &Disk) -> usize {
    let mut largest = 0;
    for t_idx in 0..disk.track_count() {
        if let Some(track) = disk.get_track(t_idx as u8) {
            let size = track.total_data_size();
            if size > largest {
                largest = size;
            }
        }
    }
    largest
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
