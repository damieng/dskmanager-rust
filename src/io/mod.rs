/// I/O operations for reading and writing DSK files

/// Reader implementation for DSK files
pub mod reader;
/// Writer implementation for DSK files
pub mod writer;

pub use reader::read_dsk;
pub use writer::write_dsk;
