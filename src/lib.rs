/*!
# dskmanager

A Rust library for reading and writing DSK disk image files with CP/M filesystem support.

## Features

- Read and write Standard, Extended and SamDisk Extended DSK formats
- Track and sector abstraction with FDC status codes
- CP/M filesystem support for reading files
- Idiomatic Rust API with comprehensive error handling

## Quick Start

```rust,no_run
use dskmanager::{DskImage, FormatSpec, FileSystem, CpmFileSystem};

// Open an existing DSK file
let mut image = DskImage::open("disk.dsk")?;

// Read a sector
let data = image.read_sector(0, 0, 0xC1)?;

// Write a sector
let new_data = vec![0xE5; 512];
image.write_sector(0, 0, 0xC1, &new_data)?;

// Save changes
image.save("disk.dsk")?;

// Create a new DSK
let spec = FormatSpec::amstrad_data();
let new_image = DskImage::create(spec)?;

// Mount CP/M filesystem
let fs = CpmFileSystem::from_image(&image)?;
for entry in fs.read_dir()? {
    println!("{}: {} bytes", entry.name, entry.size);
}

// Read a file
let contents = fs.read_file("README.TXT")?;
# Ok::<(), dskmanager::DskError>(())
```

## DSK Format Specifications

The library supports both Standard and Extended DSK formats used by:
- Amstrad CPC
- ZX Spectrum +3
- Amstrad PCW
- SAM Coupe

## Modules

- `format`: DSK format specifications and constants
- `image`: Core image data structures (DskImage, Track, Sector)
- `filesystem`: Filesystem implementations (CP/M)
- `fdc`: FDC (Floppy Disk Controller) status codes
- `error`: Error types and Result alias
*/

#![warn(missing_docs)]

/// Error types and Result alias
pub mod error;
/// FDC (Floppy Disk Controller) status codes
pub mod fdc;
/// Filesystem implementations (CP/M)
pub mod filesystem;
/// DSK format specifications and constants
pub mod format;
/// Core image data structures (DskImage, Track, Sector)
pub mod image;
/// I/O operations for reading and writing DSK files
pub mod io;
/// Sector map visualization
pub mod map;
/// Copy protection detection
pub mod protection;

// Re-export common types
pub use error::{DskError, Result};
pub use fdc::{FdcStatus1, FdcStatus2};
pub use filesystem::{
    CpmFileSystem, DirEntry, ExtendedDirEntry, FileAttributes, FileHeader, FileSystem,
    FileSystemInfo, HeaderType,
};
pub use filesystem::try_parse_header;
pub use format::{
    AllocationSize, DiskSpecFormat, DiskSpecSide, DiskSpecTrack, DiskSpecification, DskFormat,
    FormatSpec, SideMode,
};
pub use image::{
    DataRate, Disk, DskImage, DskImageBuilder, RecordingMode, Sector, SectorId, SectorStatus,
    Track,
};
