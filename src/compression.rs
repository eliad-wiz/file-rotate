//! Compression - configuration and implementation
use flate2::write::GzEncoder;
use fork::{fork, Fork};
use rustix::process;
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

/// Compression mode - when to compress files.
#[derive(Debug, Clone)]
pub enum Compression {
    /// No compression
    None,
    /// Look for files to compress when rotating.
    /// First argument: How many files to keep uncompressed (excluding the original file)
    OnRotate(usize),
}

fn compress_child_helper(path: &Path, dest_path: &PathBuf) -> io::Result<()> {
    let mut src_file = File::open(path)?;
    let dest_file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(false)
        .open(&dest_path)?;

    assert!(path.exists());
    assert!(dest_path.exists());
    let mut encoder = GzEncoder::new(dest_file, flate2::Compression::default());
    io::copy(&mut src_file, &mut encoder)?;

    fs::remove_file(path)?;
    Ok(())
}

pub(crate) fn compress(path: &Path) -> io::Result<PathBuf> {
    let dest_path = PathBuf::from(format!("{}.gz", path.display()));

    match fork() {
        Ok(Fork::Parent(child)) => {
            let child = unsafe {
                process::Pid::from_raw(child as u32)
                    .ok_or(io::Error::new(io::ErrorKind::Other, "Invalid child pid"))?
            };
            let wait_status = process::waitpid(Some(child), process::WaitOptions::empty())
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "WaitPid failed"))?
                .ok_or(io::Error::new(
                    io::ErrorKind::Other,
                    "Child sbom writer unexpected return",
                ))?;
            let exit_status = wait_status.exit_status().ok_or(io::Error::new(
                io::ErrorKind::Other,
                "Failed getting child exit code",
            ))?;
            match exit_status {
                0 => Ok(dest_path),
                _ => Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Compress child failed",
                )),
            }
        }
        Ok(Fork::Child) => {
            std::process::exit(
                compress_child_helper(path, &dest_path)
                    .and(Ok(0))
                    .unwrap_or(1),
            );
        }
        Err(_) => Err(io::Error::new(io::ErrorKind::Other, "Fork failed")),
    }
}
