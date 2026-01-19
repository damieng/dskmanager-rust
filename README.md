# DSK Manager (Rust)

An idiomatic Rust library and cli for reading and writing DSK/MGT disk image files with CP/M and MGT filesystem support.

## Features

- **DSK Format Support**: Read and write Standard DSK, Extended DSK and SamDisk extended formats
- **Track & Sector Abstraction**: Low-level access to disk geometry with FDC status codes
- **CP/M Filesystem**: Read files from CP/M filesystems (Amstrad CPC, Spectrum +3, PCW)
- **Format Presets**: Built-in configurations for Amstrad CPC, Spectrum +3, PCW, and IBM PC formats
- **Copy Protection Detection**: Automatic detection of 20+ copy protection schemes (Alkatraz, Speedlock, Hexagon, Frontier, and more)
- **Comprehensive Testing**: Extensive unit and integration test coverage
- **Interactive CLI**: Command-line tool for exploring DSK files

## Quick Start

### Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
dskmanager = "0.1"
```

### Basic Usage

```rust
use dskmanager::{DiskImage, FormatSpec, CpmFileSystem, FileSystem};

// Open an existing DSK file
let image = DiskImage::open("disk.dsk")?;

// Read a sector
let data = image.read_sector(0, 0, 0xC1)?;
println!("Sector data: {} bytes", data.len());

// Create a new DSK image
let spec = FormatSpec::amstrad_data();
let mut new_image = DiskImage::create(spec)?;

// Write a sector
let data = vec![0xE5; 512];
new_image.write_sector(0, 0, 0xC1, &data)?;

// Save the image
new_image.save("new_disk.dsk")?;
```

### Detecting Copy Protection

```rust
use dskmanager::{DiskImage, protection};

let image = DiskImage::open("game.dsk")?;

// Check each side of the disk
for (side_idx, disk) in image.disks().iter().enumerate() {
    if let Some(result) = protection::detect(disk) {
        println!("Side {}: {} [{}]", side_idx, result.name, result.reason);
    }
}
```

### Working with Filesystems

```rust
use dskmanager::{DiskImage, CpmFileSystem};

let image = DiskImage::open("cpm_disk.dsk")?;
let fs = CpmFileSystem::from_image(&image)?;

// List files
for entry in fs.read_dir()? {
    println!("{}: {} bytes", entry.name, entry.size);
}

// Read a file
let contents = fs.read_file("README.TXT")?;
println!("File contents: {} bytes", contents.len());

// Get filesystem info
let info = fs.info();
println!("Filesystem: {}", info.fs_type);
println!("Free space: {} KB", info.free_blocks * info.block_size / 1024);
```

### Using the Builder Pattern

```rust
use dskmanager::{DiskImage, DiskImageFormat};

let image = DiskImage::builder()
    .format(DiskImageFormat::ExtendedDSK)
    .num_sides(2)
    .num_tracks(80)
    .sectors_per_track(9)
    .sector_size(512)
    .build()?;
```

## Interactive CLI

The library includes an interactive command-line tool for exploring DSK files:

```bash
cargo run --bin dsk
```

Or install it as a binary:

```bash
cargo install --path .
dsk
```

Available commands:

- `open <path>` or `load <path>` - Open a DSK file
- `create [amstrad|spectrum|pcw]` - Create a new DSK image
- `info` - Show disk information
- `specification` or `spec` - Show the disk specification used to understand the FS/layout
- `tracks` - List all tracks
- `sectors` - List all sectors
- `read-sector <side> <track> <sector>` - Read and display a sector (sector can be decimal or hex like 0xC1)
- `fs-export <filename> [output] [raw]` - Export file from disk to host filesystem (strips header by default, use 'raw' to keep them)
- `fs-list` - List files on the filesystem (CAT/DIR)
- `fs-mount` - Mount the file system
- `fs-switch [auto|cpm|mgt]` - Switch between file systems. Defaults to `auto`, can also specify `cpm` or `mgt`
- `fs-read <filename>` - Read file from filesystem
- `detect-protection` - Detect copy protection schemes on the disk
- `disassemble [track] [sector]` or `dasm [track] [sector]` - Disassemble Z80 code from a sector
- `strings [len] [uniq] [charset]` - Find strings in disk (reads logically)
- `map [side]` - Visual sector map (▓=in-use, ░=empty, colored by status)
- `save <path>` - Save image to file
- `help` - Show help
- `quit` or `exit` - Exit

## Supported Formats

### Disk Image File Formats

- **Standard DSK** (.DSK): Fixed track size format
- **Extended DSK** (.DSK): Variable track sizes with SAMDisk V5 extensions
- **MGT Raw** (.MGT): MGT Disciple/+D/SAM Coupe 800KB DSDD raw sector dumps

### Disk Formats

Presets for common formats:

- Amstrad CPC System/Data (40 tracks, 9 sectors, 512 bytes)
- ZX Spectrum +3 (40 tracks, 9 sectors, 512 bytes)
- Amstrad PCW (40 tracks, 9 sectors, 512 bytes)
- IBM PC 360K/720K (40/80 tracks, 9 sectors, 512 bytes)
- Tatung Einstein
- MGT Disciple/+D/SAM Coupe

### Filesystems

- **CP/M** (read-only support for Amstrad CPC, Spectrum +3, PCW, Tatung Einstein)
- **MGT** (read-only support for MGT Disciple/+D and SAM Coupe)
  - `DiscipleFileSystem` - For ZX Spectrum DISCiPLE/+D disks
  - `SamFileSystem` - For SAM Coupe disks
  - `MgtFileSystem` - Base implementation for MGT format disks

### Copy Protection Detection

The library can automatically detect over 20 copy protection schemes commonly used on Amstrad CPC and ZX Spectrum +3 disks, including:

- **Alkatraz** (CPC and +3 variants)
- **Speedlock** (multiple versions from 1985-1990)
- **Hexagon**
- **Frontier**
- **Paul Owens**
- **Three Inch Loader** (multiple types)
- **P.M.S.** (1986-1987)
- **DiscSYS** / **Mean Protection System**
- **KBI-19**, **CAAV**, **KBI-10**
- **W.R.M. Disc Protection**
- **Players**
- **Rainbow Arts**
- **Infogrames/Logiciel**
- **ERE/Remi HERBULOT**
- **Amsoft/EXOPAL**
- **ARMOURLOC**
- **Studio B** / **DiscLoc/Oddball**
- **Laser Load by C.J. Pink**
- And more...

Detection works by analyzing disk geometry, FDC status codes, and searching for known signatures in sector data. Both signed (with embedded signatures) and unsigned (pattern-based) protections are detected.

## Architecture

The library uses an idiomatic Rust ownership-based design:

```
DiskImage (top-level)
  └─ Vec<Disk> (one per side)
      └─ Vec<Track>
          └─ Vec<Sector>
              └─ data: Vec<u8>
```

Key design decisions:

- **No circular references**: Top-down ownership eliminates Rc/RefCell
- **Zero-copy parsing** where possible
- **Comprehensive error handling** with detailed context
- **Builder pattern** for constructing images
- **Trait-based** filesystem implementations

## Testing

Run the test suite:

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test integration

# All tests
cargo test

# With output
cargo test -- --nocapture
```

Current test coverage: 70+ unit tests, 13 integration tests

## Documentation

Generate and view the documentation:

```bash
cargo doc --open
```

## CLI

The `dsk` binary provides an interactive console for exploring DSK files. Run it with `cargo run --bin dsk` or install it with `cargo install --path .`.


## License

MIT OR Apache-2.0 at your convenience.

## Contributing

Contributions welcome! Please ensure:

1. Code compiles without warnings: `cargo build`
2. Tests pass: `cargo test`
3. Code is formatted: `cargo fmt`
4. No clippy warnings: `cargo clippy`

## Roadmap

Future ideas:

- [ ] File system write support (import)
- [ ] MGT file system completion for export
- [ ] Wildcard matching for import and export
- [ ] Header generation for import and existing files
- [ ] Header stripping for existing files
- [ ] Copy/Delete/Undelete support
- [ ] Formatting including custom
- [ ] Re-interleaving/skewing existing disk images
- [ ] Defragmenting existing images
- [ ] Super-optimizer for +3 disk images?
- [ ] Boot sector extraction
- [ ] Boot sector generation (+3 only)

## Acknowledgments

- Claude Code, Copilot and Cursor are used in the development of this library and tool
- Based on my original Pascal/Lazarus DiskImageManager implementation
