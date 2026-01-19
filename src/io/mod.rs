/// I/O operations for reading and writing DSK files

/// Reader implementation for DSK files
pub mod reader;
/// Reader implementation for MGT files
pub mod mgt_reader;
/// Writer implementation for DSK files
pub mod writer;

pub use mgt_reader::{is_mgt_file, read_mgt};
pub use reader::read_dsk;
pub use writer::write_dsk;
