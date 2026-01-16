/// Interactive DSK sandbox console application

use dez80::Instruction;
use dskmanager::*;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};

/// Command completer for the sandbox REPL
struct CommandCompleter {
    commands: Vec<&'static str>,
}

impl CommandCompleter {
    fn new() -> Self {
        Self {
            commands: vec![
                "create",
                "dasm",
                "detect-protection",
                "disassemble",
                "exit",
                "fs-list",
                "fs-mount",
                "fs-read",
                "help",
                "info",
                "load",
                "open",
                "quit",
                "read-sector",
                "save",
                "sectors",
                "strings",
                "tracks",
            ],
        }
    }
}

impl Completer for CommandCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Only complete the first word (command name)
        let line_to_cursor = &line[..pos];
        if line_to_cursor.contains(' ') {
            // Already past the command, don't complete
            return Ok((pos, vec![]));
        }

        let prefix = line_to_cursor.to_lowercase();
        let matches: Vec<Pair> = self
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with(&prefix))
            .map(|cmd| Pair {
                display: cmd.to_string(),
                replacement: cmd.to_string(),
            })
            .collect();

        Ok((0, matches))
    }
}

impl Hinter for CommandCompleter {
    type Hint = String;
}

impl Highlighter for CommandCompleter {}
impl Validator for CommandCompleter {}
impl Helper for CommandCompleter {}

/// Get the path to the history file
fn history_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|mut p| {
        p.push(".dskmanager_history");
        p
    })
}

fn main() {
    println!("--- DSKManager Sandbox ---");
    println!("Interactive console for exploring DSK format disk images.");
    println!("Type 'help' for available commands\n");

    let mut rl = Editor::new().expect("Failed to create editor");
    rl.set_helper(Some(CommandCompleter::new()));

    // Load history if available
    if let Some(history_path) = history_path() {
        let _ = rl.load_history(&history_path);
    }

    let mut image: Option<DskImage> = None;

    loop {
        let readline = rl.readline("> ");
        let input = match readline {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                // Save history before exiting
                if let Some(history_path) = history_path() {
                    let _ = rl.save_history(&history_path);
                }
                println!("Goodbye!");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        };

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // Add to history
        let _ = rl.add_history_entry(input);

        let parts = parse_command_line(input);
        if parts.is_empty() {
            continue;
        }
        let command = parts[0].to_lowercase();

        match command.as_str() {
            "help" => {
                print_help();
            }
            "quit" | "exit" => {
                // Save history before exiting
                if let Some(history_path) = history_path() {
                    let _ = rl.save_history(&history_path);
                }
                println!("Goodbye!");
                break;
            }
            "open" | "load" => {
                if parts.len() < 2 {
                    println!("Usage: open <path>");
                    continue;
                }
                match DskImage::open(&parts[1]) {
                    Ok(img) => {
                        println!("Opened: {}", parts[1]);
                        image = Some(img);
                    }
                    Err(e) => println!("Error: {}", e),
                }
            }
            "create" => {
                let spec = if parts.len() > 1 {
                    match parts[1].as_str() {
                        "amstrad" => FormatSpec::amstrad_data(),
                        "spectrum" => FormatSpec::spectrum_plus3(),
                        "pcw" => FormatSpec::pcw_ssdd(),
                        _ => FormatSpec::amstrad_data(),
                    }
                } else {
                    FormatSpec::amstrad_data()
                };

                match DskImage::create(spec) {
                    Ok(img) => {
                        println!("Created new {} image", img.format().name());
                        image = Some(img);
                    }
                    Err(e) => println!("Error: {}", e),
                }
            }
            "info" => {
                if let Some(ref img) = image {
                    print_info(img);
                } else {
                    println!("No image loaded. Use 'open <path>' or 'create' first.");
                }
            }
            "tracks" => {
                if let Some(ref img) = image {
                    list_tracks(img);
                } else {
                    println!("No image loaded.");
                }
            }
            "read-sector" => {
                if let Some(ref img) = image {
                    if parts.len() < 4 {
                        println!("Usage: read-sector <side> <track> <sector_id>");
                        continue;
                    }
                    let side: u8 = parts[1].parse().unwrap_or(0);
                    let track: u8 = parts[2].parse().unwrap_or(0);
                    let sector_id: u8 = if parts[3].starts_with("0x") || parts[3].starts_with("0X") {
                        u8::from_str_radix(&parts[3].as_str()[2..], 16).unwrap_or(0xC1)
                    } else {
                        parts[3].parse().unwrap_or(0xC1)
                    };

                    match img.read_sector(side, track, sector_id) {
                        Ok(data) => {
                            println!("Sector {}:{}:{} ({} bytes):", side, track, sector_id, data.len());
                            print_hex_dump(data, 256);
                        }
                        Err(e) => println!("Error: {}", e),
                    }
                } else {
                    println!("No image loaded.");
                }
            }
            "fs-mount" => {
                if let Some(ref img) = image {
                    match CpmFileSystem::from_image(img) {
                        Ok(fs) => {
                            let info = fs.info();
                            println!("Mounted {} filesystem", info.fs_type);
                            println!("  Total blocks: {}", info.total_blocks);
                            println!("  Free blocks: {}", info.free_blocks);
                            println!("  Block size: {} bytes", info.block_size);
                            println!("  Total capacity: {} KB", info.total_blocks * info.block_size / 1024);
                        }
                        Err(e) => println!("Error: {}", e),
                    }
                } else {
                    println!("No image loaded.");
                }
            }
            "fs-list" => {
                if let Some(ref img) = image {
                    match CpmFileSystem::from_image(img) {
                        Ok(fs) => {
                            match fs.read_dir() {
                                Ok(entries) => {
                                    if entries.is_empty() {
                                        println!("No files found.");
                                    } else {
                                        println!("{:<20} {:>10} {:<10} {}", "Name", "Size", "User", "Attributes");
                                        println!("{}", "-".repeat(60));
                                        for entry in entries {
                                            let attrs = format!(
                                                "{}{}{}",
                                                if entry.attributes.read_only { "R" } else { "-" },
                                                if entry.attributes.system { "S" } else { "-" },
                                                if entry.attributes.archive { "A" } else { "-" }
                                            );
                                            println!("{:<20} {:>10} {:<10} {}", entry.name, entry.size, entry.user, attrs);
                                        }
                                    }
                                }
                                Err(e) => println!("Error: {}", e),
                            }
                        }
                        Err(e) => println!("Error mounting filesystem: {}", e),
                    }
                } else {
                    println!("No image loaded.");
                }
            }
            "fs-read" => {
                if let Some(ref img) = image {
                    if parts.len() < 2 {
                        println!("Usage: fs-read <filename>");
                        continue;
                    }
                    match CpmFileSystem::from_image(img) {
                        Ok(fs) => {
                            match fs.read_file(&parts[1]) {
                                Ok(data) => {
                                    println!("File: {} ({} bytes)", parts[1], data.len());
                                    print_hex_dump(&data, 256);
                                }
                                Err(e) => println!("Error: {}", e),
                            }
                        }
                        Err(e) => println!("Error mounting filesystem: {}", e),
                    }
                } else {
                    println!("No image loaded.");
                }
            }
            "save" => {
                if let Some(ref mut img) = image {
                    if parts.len() < 2 {
                        println!("Usage: save <path>");
                        continue;
                    }
                    match img.save(&parts[1]) {
                        Ok(_) => println!("Saved to: {}", parts[1]),
                        Err(e) => println!("Error: {}", e),
                    }
                } else {
                    println!("No image loaded.");
                }
            }
            "detect-protection" => {
                if let Some(ref img) = image {
                    let mut found_any = false;
                    let has_multiple_sides = img.disks().len() > 1;
                    for (side_idx, disk) in img.disks().iter().enumerate() {
                        if let Some(result) = dskmanager::protection::detect(disk) {
                            if has_multiple_sides {
                                println!("Side {}: {} [{}]", side_idx, result.name, result.reason);
                            } else {
                                println!("{} [{}]", result.name, result.reason);
                            }
                            found_any = true;
                        }
                    }
                    if !found_any {
                        println!("No copy protection detected.");
                    }
                } else {
                    println!("No image loaded.");
                }
            }
            "sectors" => {
                if let Some(ref img) = image {
                    if parts.len() >= 2 {
                        // Track number specified
                        let track: u8 = parts[1].parse().unwrap_or(0);
                        let side: u8 = if parts.len() >= 3 {
                            parts[2].parse().unwrap_or(0)
                        } else {
                            0
                        };
                        list_sectors_on_track(img, side, track);
                    } else {
                        // List all sectors on all tracks
                        list_all_sectors(img);
                    }
                } else {
                    println!("No image loaded.");
                }
            }
            "disassemble" | "dasm" => {
                if let Some(ref img) = image {
                    let side: u8 = 0;
                    let (track, sector_id) = if parts.len() >= 3 {
                        let t: u8 = parts[1].parse().unwrap_or(0);
                        let s: u8 = parse_hex_or_dec(&parts[2]).unwrap_or(0);
                        (t, s)
                    } else if parts.len() == 2 {
                        let t: u8 = parts[1].parse().unwrap_or(0);
                        // Find lowest sector ID on specified track
                        match find_lowest_sector_id(img, side, t) {
                            Some(s) => (t, s),
                            None => {
                                println!("No sectors found on track {}.", t);
                                continue;
                            }
                        }
                    } else {
                        // Default: track 0, lowest sector ID
                        match find_lowest_sector_id(img, side, 0) {
                            Some(s) => (0, s),
                            None => {
                                println!("No sectors found on track 0.");
                                continue;
                            }
                        }
                    };

                    match img.read_sector(side, track, sector_id) {
                        Ok(data) => {
                            println!("=== Z80 Disassembly: Track {}, Sector {} ({} bytes) ===\n",
                                track, sector_id, data.len());
                            disassemble_z80(data);
                        }
                        Err(e) => println!("Error: {}", e),
                    }
                } else {
                    println!("No image loaded.");
                }
            }
            "strings" => {
                if let Some(ref img) = image {
                    // Parse optional arguments: strings [min_length] [min_unique] [charset]
                    let min_length: usize = if parts.len() >= 2 {
                        parts[1].parse().unwrap_or(4)
                    } else {
                        4
                    };

                    let min_unique: usize = if parts.len() >= 3 {
                        parts[2].parse().unwrap_or(3)
                    } else {
                        3
                    };

                    let charset: Vec<u8> = if parts.len() >= 4 {
                        parse_charset(&parts[3])
                    } else {
                        default_ascii_chars()
                    };

                    let strings = find_strings_in_disk(img, min_length, min_unique, &charset);

                    if strings.is_empty() {
                        println!("No strings found (min length: {}, min unique: {}).", min_length, min_unique);
                    } else {
                        println!("=== Strings (min length: {}, min unique: {}) ===\n", min_length, min_unique);
                        for hit in &strings {
                            println!(
                                "S{}:T{}:{}+{:03X}: {}",
                                hit.side, hit.track, hit.sector, hit.offset, hit.text
                            );
                        }
                        println!("\nFound {} strings.", strings.len());
                    }
                } else {
                    println!("No image loaded.");
                }
            }
            _ => {
                println!("Unknown command: {}. Type 'help' for available commands.", command);
            }
        }
    }
}

/// Parse command line input, respecting quoted strings
fn parse_command_line(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

fn print_help() {
    println!("Available commands:");
    println!("  open <path>                    - Open a DSK file (use quotes for paths with spaces)");
    println!("  create [amstrad|spectrum|pcw]  - Create a new DSK image");
    println!("  info                           - Show disk information");
    println!("  tracks                         - List all tracks");
    println!("  sectors [track] [side]         - List sectors (all or specific track/side)");
    println!("  read-sector <s> <t> <id>       - Read and display a sector");
    println!("  fs-mount                       - Mount CP/M filesystem");
    println!("  fs-list                        - List files on CP/M filesystem");
    println!("  fs-read <filename>             - Read file from CP/M filesystem");
    println!("  detect-protection              - Detect copy protection scheme");
    println!("  disassemble [track] [sector]   - Disassemble Z80 code from sector (dasm)");
    println!("  strings [len] [uniq] [charset] - Find strings (default: 4, 3, A-Za-z0-9...)");
    println!("  save <path>                    - Save image to file (use quotes for paths with spaces)");
    println!("  help                           - Show this help");
    println!("  quit, exit                     - Exit the sandbox");
}

fn print_info(image: &DskImage) {
    println!("=== Disk Information ===");
    println!("Format: {}", image.format().name());
    println!("Sides: {}", image.spec().num_sides);
    println!("Tracks per side: {}", image.spec().num_tracks);
    println!("Sectors per track: {}", image.spec().sectors_per_track);
    println!("Sector size: {} bytes", image.spec().sector_size);
    println!("First sector ID: {}", image.spec().first_sector_id);
    println!("Total capacity: {} KB", image.total_capacity_kb());
    println!("Changed: {}", if image.is_changed() { "Yes" } else { "No" });
}

fn list_tracks(image: &DskImage) {
    println!("=== Tracks ===");
    for (side_idx, disk) in image.disks().iter().enumerate() {
        println!("Side {}:", side_idx);
        for track in disk.tracks() {
            let sector_ids: Vec<String> = track
                .sector_ids()
                .iter()
                .map(|id| format!("{}", id))
                .collect();

            println!(
                "  Track {:2}: {} sectors [{}]",
                track.track_number,
                track.sector_count(),
                sector_ids.join(", ")
            );
        }
    }
}

fn list_sectors_on_track(image: &DskImage, side: u8, track: u8) {
    if let Some(disk) = image.disks().get(side as usize) {
        if let Some(track_data) = disk.get_track(track) {
            println!("=== Sectors on Side {}, Track {} ===", side, track);
            println!("{:<8} {:<8} {:<8} {:<12} {:<12} {:<8}", "Sector", "Track", "Side", "Size Code", "Size (bytes)", "Status");
            println!("{}", "-".repeat(70));

            for sector in track_data.sectors() {
                let status = if sector.has_error() {
                    "ERROR"
                } else if sector.is_deleted() {
                    "DELETED"
                } else {
                    "OK"
                };

                println!(
                    "{:<8} {:<8} {:<8} {:<12} {:<12} {:<8}",
                    sector.id.sector,
                    sector.id.track,
                    sector.id.side,
                    sector.id.size_code,
                    sector.actual_size(),
                    status
                );
            }

            println!("\nTotal sectors: {}", track_data.sector_count());
        } else {
            println!("Track {} not found on side {}.", track, side);
        }
    } else {
        println!("Side {} not found.", side);
    }
}

fn list_all_sectors(image: &DskImage) {
    println!("=== All Sectors ===");

    for (side_idx, disk) in image.disks().iter().enumerate() {
        for track in disk.tracks() {
            println!("\n--- Side {}, Track {} ---", side_idx, track.track_number);
            println!("{:<8} {:<8} {:<8} {:<12} {:<12} {:<8}", "Sector", "Track", "Side", "Size Code", "Size (bytes)", "Status");
            println!("{}", "-".repeat(70));

            for sector in track.sectors() {
                let status = if sector.has_error() {
                    "ERROR"
                } else if sector.is_deleted() {
                    "DELETED"
                } else {
                    "OK"
                };

                println!(
                    "{:<8} {:<8} {:<8} {:<12} {:<12} {:<8}",
                    sector.id.sector,
                    sector.id.track,
                    sector.id.side,
                    sector.id.size_code,
                    sector.actual_size(),
                    status
                );
            }

            println!("Total sectors: {}", track.sector_count());
        }
    }
}

fn print_hex_dump(data: &[u8], max_bytes: usize) {
    let len = data.len().min(max_bytes);

    for (i, chunk) in data[..len].chunks(16).enumerate() {
        print!("{:04X}: ", i * 16);

        // Print hex
        for (j, byte) in chunk.iter().enumerate() {
            print!("{:02X} ", byte);
            if j == 7 {
                print!(" ");
            }
        }

        // Pad if less than 16 bytes
        for j in chunk.len()..16 {
            print!("   ");
            if j == 7 {
                print!(" ");
            }
        }

        print!(" |");

        // Print ASCII
        for byte in chunk {
            let c = if *byte >= 32 && *byte < 127 {
                *byte as char
            } else {
                '.'
            };
            print!("{}", c);
        }

        println!("|");
    }

    if data.len() > max_bytes {
        println!("... ({} more bytes)", data.len() - max_bytes);
    }
}

fn parse_hex_or_dec(s: &str) -> Option<u8> {
    if s.starts_with("0x") || s.starts_with("0X") {
        u8::from_str_radix(&s[2..], 16).ok()
    } else {
        s.parse().ok()
    }
}

fn find_lowest_sector_id(image: &DskImage, side: u8, track: u8) -> Option<u8> {
    let disk = image.disks().get(side as usize)?;
    let track_data = disk.get_track(track)?;
    track_data
        .sectors()
        .iter()
        .map(|s| s.id.sector)
        .min()
}

fn disassemble_z80(data: &[u8]) {
    let mut slice: &[u8] = data;
    let mut address: u16 = 0;

    while !slice.is_empty() {
        let start_len = slice.len();

        match Instruction::decode_one(&mut slice) {
            Ok(instruction) => {
                let bytes_consumed = start_len - slice.len();
                let bytes: Vec<String> = data[address as usize..address as usize + bytes_consumed]
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect();

                println!(
                    "{:04X}  {:<12} {}",
                    address,
                    bytes.join(" "),
                    instruction
                );

                address += bytes_consumed as u16;
            }
            Err(_) => {
                // Invalid instruction - show as data byte
                println!("{:04X}  {:02X}           DB {:02X}h", address, slice[0], slice[0]);
                slice = &slice[1..];
                address += 1;
            }
        }
    }
}

/// Default ASCII character set for strings command
/// Conservative set to match English-like words, not random byte sequences
fn default_ascii_chars() -> Vec<u8> {
    let mut chars = Vec::new();
    // A-Z
    chars.extend(b'A'..=b'Z');
    // a-z
    chars.extend(b'a'..=b'z');
    // 0-9
    chars.extend(b'0'..=b'9');
    // Space and common punctuation found in text
    chars.extend(b" !\"'()*+,-.:;=?".iter());
    chars
}

/// Parse a charset specification like "A-Za-z0-9 " or "32-126"
fn parse_charset(spec: &str) -> Vec<u8> {
    let mut chars = Vec::new();
    let mut iter = spec.chars().peekable();

    while let Some(ch) = iter.next() {
        if iter.peek() == Some(&'-') {
            // Range like A-Z
            iter.next(); // consume '-'
            if let Some(end_ch) = iter.next() {
                let start = ch as u8;
                let end = end_ch as u8;
                if start <= end {
                    chars.extend(start..=end);
                }
            }
        } else {
            chars.push(ch as u8);
        }
    }

    if chars.is_empty() {
        default_ascii_chars()
    } else {
        chars
    }
}

/// A string found in the disk with its location
struct StringHit {
    side: usize,
    track: u8,
    sector: u8,
    offset: usize,
    text: String,
}

/// Find strings in a disk, iterating in logical order
fn find_strings_in_disk(image: &DskImage, min_length: usize, min_unique: usize, charset: &[u8]) -> Vec<StringHit> {
    let mut hits = Vec::new();
    let spec = image.spec();

    match spec.side_mode {
        SideMode::SingleSide => {
            if let Some(disk) = image.get_disk(0) {
                for track_num in 0..disk.track_count() {
                    find_strings_in_track(disk, 0, track_num as u8, min_length, min_unique, charset, &mut hits);
                }
            }
        }
        SideMode::Alternate => {
            let max_tracks = image
                .disks()
                .iter()
                .map(|d| d.track_count())
                .max()
                .unwrap_or(0);

            for track_num in 0..max_tracks {
                for (side_idx, disk) in image.disks().iter().enumerate() {
                    find_strings_in_track(disk, side_idx, track_num as u8, min_length, min_unique, charset, &mut hits);
                }
            }
        }
        SideMode::Successive => {
            for (side_idx, disk) in image.disks().iter().enumerate() {
                for track_num in 0..disk.track_count() {
                    find_strings_in_track(disk, side_idx, track_num as u8, min_length, min_unique, charset, &mut hits);
                }
            }
        }
    }

    hits
}

/// Count unique characters in a string
fn unique_char_count(s: &str) -> usize {
    let mut seen = std::collections::HashSet::new();
    for c in s.chars() {
        seen.insert(c);
    }
    seen.len()
}

/// Find strings in a single track
fn find_strings_in_track(
    disk: &image::Disk,
    side: usize,
    track_num: u8,
    min_length: usize,
    min_unique: usize,
    charset: &[u8],
    hits: &mut Vec<StringHit>,
) {
    let Some(track) = disk.get_track(track_num) else {
        return;
    };

    // Get sectors sorted by ID
    let mut sector_ids: Vec<u8> = track.sectors().iter().map(|s| s.id.sector).collect();
    sector_ids.sort();

    for sector_id in sector_ids {
        let Some(sector) = track.get_sector(sector_id) else {
            continue;
        };

        let data = sector.data();
        let mut current_string = String::new();
        let mut start_offset = 0;

        for (i, &byte) in data.iter().enumerate() {
            if charset.contains(&byte) {
                if current_string.is_empty() {
                    start_offset = i;
                }
                current_string.push(byte as char);
            } else {
                if current_string.len() >= min_length && unique_char_count(&current_string) >= min_unique {
                    hits.push(StringHit {
                        side,
                        track: track_num,
                        sector: sector_id,
                        offset: start_offset,
                        text: current_string.clone(),
                    });
                }
                current_string.clear();
            }
        }

        // Don't forget trailing string
        if current_string.len() >= min_length && unique_char_count(&current_string) >= min_unique {
            hits.push(StringHit {
                side,
                track: track_num,
                sector: sector_id,
                offset: start_offset,
                text: current_string,
            });
        }
    }
}
