/// Batch DSK analysis tool
///
/// Walks a directory of .dsk files and outputs a CSV with:
/// - file path
/// - format name (from specification detector)
/// - format source (how specification was determined)
/// - protection name and reason
/// - characteristics fingerprint (short name for the disk's structure)
/// - detailed characteristics (sector layout, track patterns, etc.)
///
/// Usage: dsk-analyze <directory> [output.csv]
///
/// If output.csv is not specified, prints CSV to stdout.

use std::path::Path;
use dskmanager::*;
use std::io::Write;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <directory> [output.csv]", args[0]);
        std::process::exit(1);
    }

    let dir = &args[1];
    let output_path = args.get(2);

    let mut writer: Box<dyn Write> = match output_path {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => Box::new(std::io::stdout()),
    };

    writeln!(writer, "file,format,format_source,protection,protection_reason,fingerprint,tracks,sides,sectors_per_track,sector_size,first_sector_id,is_uniform,has_fdc_errors,track_layout,nine_sector_tracks,non_nine_tracks,empty_tracks,biggest_track_bytes,sector_count_pattern")?;

    let mut dsk_count = 0;
    let mut error_count = 0;
    walk_dir(Path::new(dir), &mut writer, &mut dsk_count, &mut error_count)?;

    eprintln!("Processed {} DSK files ({} errors)", dsk_count, error_count);
    Ok(())
}

fn walk_dir(
    dir: &Path,
    writer: &mut Box<dyn Write>,
    count: &mut usize,
    errors: &mut usize,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, writer, count, errors)?;
        } else if path.extension().map(|e| e == "dsk").unwrap_or(false) {
            *count += 1;
            match analyze_one(&path) {
                Ok(line) => {
                    writeln!(writer, "{}", line)?;
                }
                Err(e) => {
                    let escaped = path.to_string_lossy().replace(',', ";");
                    writeln!(writer, "{},ERROR,ERROR,ERROR,{},ERROR,,,,,,,ERROR,,,,", escaped, e)?;
                    *errors += 1;
                }
            }
            if *count % 100 == 0 {
                eprintln!("  {} files processed...", count);
            }
        }
    }
    Ok(())
}

fn analyze_one(path: &Path) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let image = DiskImage::open(path)?;
    let spec = DiskSpecification::identify(&image);

    let mut protections = Vec::new();
    for disk in image.disks() {
        if let Some(prot) = protection::detect(disk) {
            protections.push(format!("{} | {}", prot.name, prot.reason));
        }
    }
    let protection_str = if protections.is_empty() {
        String::from("None")
    } else {
        protections.join(" ; ")
    };

    let chars = compute_characteristics(&image);
    let fingerprint = chars.fingerprint();
    let escaped_path = path.to_string_lossy().replace(',', ";");

    Ok(format!(
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        escaped_path,
        escape_csv(&spec.format),
        escape_csv(&spec.source),
        if protection_str == "None" { "".to_string() } else { escape_csv(&protection_str) },
        if protection_str == "None" { "".to_string() } else { escape_csv(&protection_str) },
        escape_csv(&fingerprint),
        chars.total_tracks,
        chars.num_sides,
        chars.sectors_per_track,
        chars.sector_size,
        chars.first_sector_id,
        chars.is_uniform,
        chars.has_fdc_errors,
        escape_csv(&chars.track_layout),
        chars.nine_sector_tracks,
        chars.non_nine_tracks,
        chars.empty_tracks,
        chars.biggest_track_bytes,
        escape_csv(&chars.sector_count_pattern),
    ))
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        s.to_string()
    }
}

#[derive(Debug, Clone)]
struct Characteristics {
    total_tracks: usize,
    num_sides: usize,
    sectors_per_track: String,
    sector_size: String,
    first_sector_id: String,
    is_uniform: bool,
    has_fdc_errors: bool,
    track_layout: String,
    nine_sector_tracks: usize,
    non_nine_tracks: String,
    empty_tracks: String,
    biggest_track_bytes: usize,
    sector_count_pattern: String,
}

impl Characteristics {
    fn fingerprint(&self) -> String {
        let size_key = match (self.sectors_per_track.as_str(), self.sector_size.as_str()) {
            ("9", "512") => "9x512",
            ("10", "512") => "10x512",
            ("9", "256") => "9x256",
            ("5", "1024") => "5x1K",
            ("16", "256") => "16x256",
            ("8", "512") => "8x512",
            _ => &format!("{}x{}", self.sectors_per_track, self.sector_size),
        };

        let track_key = if self.total_tracks > 0 {
            format!("{}T", self.total_tracks)
        } else {
            "?T".to_string()
        };

        let side_key = if self.num_sides == 2 { "DS" } else { "SS" };

        let uniform_key = if self.is_uniform { "U" } else { "NU" };
        let error_key = if self.has_fdc_errors { "+ERR" } else { "" };

        let mut parts = vec![
            format!("{}_{}_{}", size_key, track_key, side_key),
            uniform_key.to_string(),
        ];
        if !error_key.is_empty() {
            parts.push(error_key.to_string());
        }

        let sig = parts.join("-");

        let id_key = match self.first_sector_id.as_str() {
            "1" => "id1",
            "0xC1" => "idC1",
            "0x41" => "id41",
            "0" => "id0",
            "41" => "id41",
            _ => "id?",
        };

        format!("{}-{}", sig, id_key)
    }
}

fn compute_characteristics(image: &DiskImage) -> Characteristics {
    let num_sides = image.disks().len();
    let disk = match image.get_disk(0) {
        Some(d) => d,
        None => return empty_characteristics(),
    };

    let total_tracks = disk.track_count();

    // Collect per-track sector counts
    let mut sector_counts: Vec<usize> = Vec::new();
    let mut sector_sizes: Vec<Option<usize>> = Vec::new();
    let mut first_sector_ids: Vec<u8> = Vec::new();
    let mut empty_track_indices: Vec<usize> = Vec::new();
    let mut non_9_indices: Vec<(usize, usize)> = Vec::new();
    let mut max_track_size = 0usize;
    let mut has_any_fdc_error = false;
    let mut uniform = true;
    let mut first_sector_count: Option<usize> = None;
    let mut first_sector_size: Option<usize> = None;

    for t_idx in 0..total_tracks {
        let track = match disk.get_track(t_idx as u8) {
            Some(t) => t,
            None => continue,
        };

        let sc = track.sector_count();
        sector_counts.push(sc);
        if sc == 0 {
            empty_track_indices.push(t_idx);
        }

        if first_sector_count.is_none() {
            first_sector_count = Some(sc);
        }
        if let Some(fsc) = first_sector_count {
            if sc != fsc {
                uniform = false;
            }
        }

        let size = track.uniform_sector_size();
        sector_sizes.push(size);
        if first_sector_size.is_none() {
            first_sector_size = size;
        }
        if let Some(_fs) = first_sector_size {
            if size != first_sector_size && size.is_some() {
                uniform = false;
            }
        }

        if sc != 9 {
            non_9_indices.push((t_idx, sc));
        }

        let track_size = track.total_data_size();
        if track_size > max_track_size {
            max_track_size = track_size;
        }

        if let Some(first_sector) = track.get_sector_by_index(0) {
            first_sector_ids.push(first_sector.id.sector);
        }

        for s_idx in 0..sc {
            if let Some(sector) = track.get_sector_by_index(s_idx) {
                if sector.has_error() {
                    has_any_fdc_error = true;
                }
            }
        }
    }

    // Derive most common sector count and sector size
    let most_common_count = most_common(&sector_counts).unwrap_or(0);
    let most_common_size = most_common_option(&sector_sizes).unwrap_or(None).unwrap_or(0);
    let first_id = first_sector_ids.first().copied().map(|id| format_sector_id(id)).unwrap_or_default();

    // Empty tracks as ranges
    let empty_ranges = format_ranges(&empty_track_indices);

    // Non-9 tracks
    let non9_str = if non_9_indices.is_empty() {
        String::new()
    } else {
        non_9_indices.iter()
            .map(|(t, sc)| format!("T{}:{}s", t, sc))
            .collect::<Vec<_>>()
            .join("; ")
    };

    // Track layout summary
    let layout = build_track_layout(disk, total_tracks);

    // Sector count pattern (compress consecutive identical counts)
    let count_pattern = compress_sector_counts(&sector_counts);

    Characteristics {
        total_tracks,
        num_sides,
        sectors_per_track: most_common_count.to_string(),
        sector_size: most_common_size.to_string(),
        first_sector_id: first_id,
        is_uniform: uniform,
        has_fdc_errors: has_any_fdc_error,
        track_layout: layout,
        nine_sector_tracks: sector_counts.iter().filter(|&&c| c == 9).count(),
        non_nine_tracks: non9_str,
        empty_tracks: empty_ranges,
        biggest_track_bytes: max_track_size,
        sector_count_pattern: count_pattern,
    }
}

fn empty_characteristics() -> Characteristics {
    Characteristics {
        total_tracks: 0,
        num_sides: 0,
        sectors_per_track: String::new(),
        sector_size: String::new(),
        first_sector_id: String::new(),
        is_uniform: false,
        has_fdc_errors: false,
        track_layout: String::new(),
        nine_sector_tracks: 0,
        non_nine_tracks: String::new(),
        empty_tracks: String::new(),
        biggest_track_bytes: 0,
        sector_count_pattern: String::new(),
    }
}

fn most_common<T: PartialEq + Clone>(items: &[T]) -> Option<T> {
    if items.is_empty() { return None; }
    let mut best: Option<&T> = None;
    let mut best_count = 0usize;
    for item in items {
        let count = items.iter().filter(|&x| x == item).count();
        if count > best_count {
            best = Some(item);
            best_count = count;
        }
    }
    best.cloned()
}

fn most_common_option<T: PartialEq + Clone>(items: &[Option<T>]) -> Option<Option<T>> {
    let non_none: Vec<&T> = items.iter().filter_map(|x| x.as_ref()).collect();
    if non_none.is_empty() { return None; }
    let best = most_common(&non_none)?;
    Some(Some(best.clone()))
}

fn format_sector_id(id: u8) -> String {
    match id {
        0 => "0".to_string(),
        1..=9 => id.to_string(),
        0x41 => "0x41".to_string(),
        0xC1 => "0xC1".to_string(),
        _ => format!("0x{:02X}", id),
    }
}

fn format_ranges(indices: &[usize]) -> String {
    if indices.is_empty() { return String::new(); }
    let mut result = Vec::new();
    let mut start = indices[0];
    let mut end = indices[0];
    for &i in &indices[1..] {
        if i == end + 1 {
            end = i;
        } else {
            if start == end {
                result.push(start.to_string());
            } else {
                result.push(format!("{}-{}", start, end));
            }
            start = i;
            end = i;
        }
    }
    if start == end {
        result.push(start.to_string());
    } else {
        result.push(format!("{}-{}", start, end));
    }
    result.join(",")
}

fn build_track_layout(disk: &image::Disk, total_tracks: usize) -> String {
    let mut parts = Vec::new();
    let mut i = 0;

    // Group consecutive tracks with the same sector count and sector size
    while i < total_tracks {
        let track = match disk.get_track(i as u8) {
            Some(t) => t,
            None => break,
        };
        let sc = track.sector_count();
        let sz = track.uniform_sector_size().unwrap_or(0);
        let filler = track.filler_byte;

        let mut j = i + 1;
        while j < total_tracks {
            let next = match disk.get_track(j as u8) {
                Some(t) => t,
                None => break,
            };
            if next.sector_count() == sc
                && next.uniform_sector_size().unwrap_or(0) == sz
                && next.filler_byte == filler
            {
                j += 1;
            } else {
                break;
            }
        }

        let range = if j - i == 1 {
            format!("T{}", i)
        } else {
            format!("T{}-{}", i, j - 1)
        };

        if sc == 0 {
            parts.push(format!("{}=empty", range));
        } else {
            let status = track_status(disk, i);
            parts.push(format!("{}=({}x{},{})", range, sc, sz, status));
        }

        i = j;
    }

    parts.join("|")
}

fn track_status(disk: &image::Disk, track_idx: usize) -> String {
    let track = match disk.get_track(track_idx as u8) {
        Some(t) => t,
        None => return "?".to_string(),
    };

    let has_error = track.sectors().iter().any(|s| s.has_error());
    let has_deleted = track.sectors().iter().any(|s| s.is_deleted());
    let has_mismatch = track.sectors().iter().any(|s| s.has_size_mismatch());
    let sizes: Vec<String> = track.sectors().iter()
        .map(|s| format!("{}", s.actual_size()))
        .collect();
    let all_same_size = sizes.windows(2).all(|w| w[0] == w[1]);

    let mut flags = Vec::new();
    if has_error { flags.push("ERR"); }
    if has_deleted { flags.push("DEL"); }
    if has_mismatch { flags.push("SZMIS"); }
    if !all_same_size { flags.push("VAR"); }

    if flags.is_empty() {
        "ok".to_string()
    } else {
        flags.join("+")
    }
}

fn compress_sector_counts(counts: &[usize]) -> String {
    if counts.is_empty() { return String::new(); }

    let mut parts = Vec::new();
    let mut current = counts[0];
    let mut run_len = 1usize;

    for &c in &counts[1..] {
        if c == current {
            run_len += 1;
        } else {
            parts.push(format!("{}x{}", current, run_len));
            current = c;
            run_len = 1;
        }
    }
    parts.push(format!("{}x{}", current, run_len));

    parts.join("|")
}
