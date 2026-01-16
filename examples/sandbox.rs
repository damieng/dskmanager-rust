/// Interactive DSK sandbox console application

use dskmanager::*;
use std::io::{self, Write};

fn main() {
    println!("--- DSKManager Sandbox ---");
    println!("Interactive console for exploring DSK format disk images.");
    println!("Type 'help' for available commands\n");

    let mut image: Option<DskImage> = None;

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

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
                            println!("Sector {}:{}:{:#04X} ({} bytes):", side, track, sector_id, data.len());
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
    println!("  read-sector <s> <t> <id>       - Read and display a sector (hex: 0xC1)");
    println!("  fs-mount                       - Mount CP/M filesystem");
    println!("  fs-list                        - List files on CP/M filesystem");
    println!("  fs-read <filename>             - Read file from CP/M filesystem");
    println!("  detect-protection              - Detect copy protection scheme");
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
    println!("First sector ID: {:#04X}", image.spec().first_sector_id);
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
                .map(|id| format!("{:#04X}", id))
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
                    "{:<8X} {:<8} {:<8} {:<12} {:<12} {:<8}",
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
                    "{:<8X} {:<8} {:<8} {:<12} {:<12} {:<8}",
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
