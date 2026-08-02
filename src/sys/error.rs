use thiserror::Error;

#[derive(Error, Debug)]
pub enum RineError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid DOS header magic: expected 0x5A4D (MZ), found 0x{0:04X}")]
    InvalidDosHeader(u16),

    #[error("Invalid PE header signature: expected 0x00004550 (PE\\0\\0), found 0x{0:08X}")]
    InvalidPeSignature(u32),

    #[error("Unsupported architecture: expected x86_64 (0x8664), found 0x{0:04X}")]
    UnsupportedArchitecture(u16),

    #[error("Unsupported PE format: expected PE32+ / Win64 (0x020B), found 0x{0:04X}")]
    UnsupportedPeFormat(u16),

    #[error("Truncated or malformed PE image data")]
    TruncatedData,

    #[error("Memory allocation or mapping failed: {0}")]
    MemoryMappingFailed(String),

    #[error("Invalid section virtual or raw data offset")]
    InvalidSectionData,
}

pub type Result<T> = std::result::Result<T, RineError>;
