//! Compression - configuration and implementation
use flate2::write::GzEncoder;
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

/// Compression type - algorithm + level
#[derive(Debug, Clone)]
pub enum CompressionType {
    /// Gzip compression
    Gzip(u32),

    /// Zstd compression
    #[cfg(feature = "zstd")]
    Zstd(u32),
}

impl CompressionType {
    /// suffix for the compressed file
    pub fn suffix(&self) -> &'static str {
        match self {
            CompressionType::Gzip(_) => "gz",
            #[cfg(feature = "zstd")]
            CompressionType::Zstd(_) => "zst",
        }
    }
}

/// Default compression type is similar to flate2::Compression::Default
impl Default for CompressionType {
    fn default() -> Self {
        CompressionType::Gzip(6)
    }
}

/// Compression mode - when to compress files.
#[derive(Debug, Clone)]
pub enum Compression {
    /// No compression
    None,
    /// Look for files to compress when rotating.
    /// First argument: How many files to keep uncompressed (excluding the original file)
    OnRotate {
        /// How many files to keep uncompressed (excluding the original file)
        keep_uncompressed: usize,
        /// Compression type
        compression: CompressionType,
    },
}

pub(crate) fn compress(path: &Path, compression: &CompressionType) -> io::Result<PathBuf> {
    let dest_path = PathBuf::from(format!("{}.{}", path.display(), compression.suffix()));

    let mut src_file = File::open(path)?;
    let dest_file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(false)
        .open(&dest_path)?;

    assert!(path.exists());
    assert!(dest_path.exists());

    match compression {
        CompressionType::Gzip(level) => {
            let mut encoder = GzEncoder::new(dest_file, flate2::Compression::new(*level));
            io::copy(&mut src_file, &mut encoder)?;
            encoder.finish()?;
        }
        #[cfg(feature = "zstd")]
        CompressionType::Zstd(level) => {
            let file_size = fs::metadata(path)?.len();
            if file_size > u32::MAX as u64 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "File size is too large for zstd",
                ));
            }

            let mut encoder = zstd::stream::Encoder::new(dest_file, *level as i32)?;
            encoder.set_parameter(zstd::zstd_safe::CParameter::SrcSizeHint(file_size as u32))?;
            io::copy(&mut src_file, &mut encoder)?;
            encoder.finish()?;
        }
    }

    fs::remove_file(path)?;

    Ok(dest_path)
}
