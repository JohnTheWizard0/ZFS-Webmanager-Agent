// zfs_management/replication.rs
// Replication operations: send, receive, replicate, estimate_size

use super::helpers::errno_to_string;
use super::manager::ZfsManager;
use super::types::ZfsError;
use libzetta::zfs::{SendFlags, ZfsEngine};
use libzetta_zfs_core_sys::{lzc_send_flags, lzc_send_space};
use std::ffi::CString;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::ptr;

/// Blocked directory prefixes for file operations (SEC-09)
/// These are sensitive system directories that should never be accessed
const BLOCKED_PATHS: &[&str] = &[
    "/etc",
    "/root",
    "/home",
    "/var",
    "/usr",
    "/bin",
    "/sbin",
    "/lib",
    "/boot",
    "/proc",
    "/sys",
    "/dev",
    "/run",
];

/// Validate a file path for replication operations (SEC-09)
/// - Must be absolute
/// - Must not contain path traversal after canonicalization
/// - Must not access blocked system directories
fn validate_file_path(path: &str) -> Result<PathBuf, ZfsError> {
    // Check absolute path
    if !path.starts_with('/') {
        return Err("File path must be absolute (start with /)".to_string());
    }

    // Get the parent directory and ensure it exists for canonicalization
    let path_obj = Path::new(path);
    let parent = path_obj.parent().ok_or("Invalid file path")?;

    // Canonicalize parent to resolve any .. or symlinks
    let canonical_parent = parent
        .canonicalize()
        .map_err(|e| format!("Invalid path - parent directory error: {}", e))?;

    // Reconstruct the full canonical path
    let file_name = path_obj
        .file_name()
        .ok_or("Invalid file path - no filename")?;
    let canonical_path = canonical_parent.join(file_name);

    // Convert to string for prefix checking
    let canonical_str = canonical_path
        .to_str()
        .ok_or("Invalid path - not valid UTF-8")?;

    // Check against blocked directories
    for blocked in BLOCKED_PATHS {
        if canonical_str.starts_with(blocked)
            && (canonical_str.len() == blocked.len()
                || canonical_str.chars().nth(blocked.len()) == Some('/'))
        {
            return Err(format!(
                "Access denied: path '{}' is in a restricted directory",
                blocked
            ));
        }
    }

    Ok(canonical_path)
}

impl ZfsManager {
    /// Send a snapshot to a file
    #[allow(clippy::too_many_arguments)]
    pub async fn send_snapshot_to_file(
        &self,
        snapshot: &str,
        output_file: &str,
        from_snapshot: Option<&str>,
        recursive: bool,
        _properties: bool,
        raw: bool,
        compressed: bool,
        large_blocks: bool,
        overwrite: bool,
    ) -> Result<u64, ZfsError> {
        if !self
            .zfs_engine
            .exists(PathBuf::from(snapshot))
            .map_err(|e| format!("Failed to check snapshot: {}", e))?
        {
            return Err(format!("Snapshot '{}' does not exist", snapshot));
        }

        // Validate output path (SEC-09) - for new files, validate parent exists
        let output_path = if std::path::Path::new(output_file).exists() {
            validate_file_path(output_file)?
        } else {
            // For new files, validate parent directory
            let parent = std::path::Path::new(output_file)
                .parent()
                .ok_or("Invalid output path")?;
            let canonical_parent = parent
                .canonicalize()
                .map_err(|e| format!("Output directory error: {}", e))?;
            let file_name = std::path::Path::new(output_file)
                .file_name()
                .ok_or("Invalid output filename")?;
            let full_path = canonical_parent.join(file_name);

            // Check blocked paths
            let path_str = full_path.to_str().ok_or("Invalid path - not valid UTF-8")?;
            for blocked in BLOCKED_PATHS {
                if path_str.starts_with(blocked)
                    && (path_str.len() == blocked.len()
                        || path_str.chars().nth(blocked.len()) == Some('/'))
                {
                    return Err(format!(
                        "Access denied: path '{}' is in a restricted directory",
                        blocked
                    ));
                }
            }
            full_path
        };

        if output_path.exists() && !overwrite {
            return Err(format!(
                "Output file '{}' already exists. Set overwrite: true to replace.",
                output_file
            ));
        }

        if recursive {
            return Err(
                "Recursive send (-R) is not supported by libzetta. Use single snapshot sends."
                    .to_string(),
            );
        }

        let mut flags = SendFlags::empty();
        if large_blocks {
            flags |= SendFlags::LZC_SEND_FLAG_LARGE_BLOCK;
        }
        if compressed {
            flags |= SendFlags::LZC_SEND_FLAG_COMPRESS;
        }
        if raw {
            flags |= SendFlags::LZC_SEND_FLAG_RAW;
        }
        flags |= SendFlags::LZC_SEND_FLAG_EMBED_DATA;

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&output_path)
            .map_err(|e| format!("Failed to create output file '{}': {}", output_path.display(), e))?;

        if let Some(from) = from_snapshot {
            let from_path = if from.contains('@') {
                from.to_string()
            } else {
                let dataset = snapshot.split('@').next().ok_or("Invalid snapshot path")?;
                format!("{}@{}", dataset, from)
            };

            self.zfs_engine
                .send_incremental(
                    PathBuf::from(snapshot),
                    PathBuf::from(&from_path),
                    file,
                    flags,
                )
                .map_err(|e| format!("libzetta send_incremental failed: {}", e))?;
        } else {
            self.zfs_engine
                .send_full(PathBuf::from(snapshot), file, flags)
                .map_err(|e| format!("libzetta send_full failed: {}", e))?;
        }

        let metadata = std::fs::metadata(&output_path)
            .map_err(|e| format!("Failed to read output file: {}", e))?;
        Ok(metadata.len())
    }

    /// Receive a snapshot from a file
    /// Uses stdin pipe instead of shell to prevent command injection (SEC-02)
    pub async fn receive_snapshot_from_file(
        &self,
        target_dataset: &str,
        input_file: &str,
        force: bool,
    ) -> Result<String, ZfsError> {
        use std::fs::File;
        use std::os::unix::io::{FromRawFd, IntoRawFd};

        // Validate input path (SEC-09)
        let validated_path = validate_file_path(input_file)?;

        if !validated_path.exists() {
            return Err(format!("Input file '{}' does not exist", input_file));
        }

        // Open file handle directly - no shell involved (prevents injection)
        let file = File::open(&validated_path)
            .map_err(|e| format!("Failed to open input file '{}': {}", validated_path.display(), e))?;

        let mut cmd = std::process::Command::new("zfs");
        cmd.arg("receive");

        if force {
            cmd.arg("-F");
        }

        cmd.arg("-v");
        cmd.arg(target_dataset);

        // Pipe file directly to stdin (no shell, no injection risk)
        let file_fd = file.into_raw_fd();
        cmd.stdin(unsafe { std::process::Stdio::from_raw_fd(file_fd) });
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute zfs receive: {}", e))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);
            Ok(combined.trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("zfs receive failed: {}", stderr.trim()))
        }
    }

    /// Replicate a snapshot directly to another pool
    #[allow(clippy::too_many_arguments)]
    pub async fn replicate_snapshot(
        &self,
        snapshot: &str,
        target_dataset: &str,
        from_snapshot: Option<&str>,
        recursive: bool,
        _properties: bool,
        raw: bool,
        compressed: bool,
        force: bool,
    ) -> Result<String, ZfsError> {
        if !self
            .zfs_engine
            .exists(PathBuf::from(snapshot))
            .map_err(|e| format!("Failed to check snapshot: {}", e))?
        {
            return Err(format!("Snapshot '{}' does not exist", snapshot));
        }

        if recursive {
            return Err("Recursive replication (-R) is not supported by libzetta. Use single snapshot replication.".to_string());
        }

        let mut flags = SendFlags::empty();
        if compressed {
            flags |= SendFlags::LZC_SEND_FLAG_COMPRESS;
        }
        if raw {
            flags |= SendFlags::LZC_SEND_FLAG_RAW;
        }
        flags |= SendFlags::LZC_SEND_FLAG_EMBED_DATA;
        flags |= SendFlags::LZC_SEND_FLAG_LARGE_BLOCK;

        let (pipe_read, pipe_write) = std::os::unix::net::UnixStream::pair()
            .map_err(|e| format!("Failed to create pipe: {}", e))?;

        let engine = self.zfs_engine.clone();
        let snapshot_owned = snapshot.to_string();
        let from_owned = from_snapshot.map(|s| {
            if s.contains('@') {
                s.to_string()
            } else {
                let dataset = snapshot.split('@').next().unwrap_or(snapshot);
                format!("{}@{}", dataset, s)
            }
        });

        let send_handle = std::thread::spawn(move || {
            if let Some(from) = from_owned {
                engine.send_incremental(
                    PathBuf::from(&snapshot_owned),
                    PathBuf::from(&from),
                    pipe_write,
                    flags,
                )
            } else {
                engine.send_full(PathBuf::from(&snapshot_owned), pipe_write, flags)
            }
        });

        let mut recv_cmd = std::process::Command::new("zfs");
        recv_cmd.arg("receive");
        if force {
            recv_cmd.arg("-F");
        }
        recv_cmd.arg(target_dataset);

        use std::os::unix::io::{FromRawFd, IntoRawFd};
        let pipe_read_fd = pipe_read.into_raw_fd();
        recv_cmd.stdin(unsafe { std::process::Stdio::from_raw_fd(pipe_read_fd) });
        recv_cmd.stdout(std::process::Stdio::piped());
        recv_cmd.stderr(std::process::Stdio::piped());

        let recv_child = recv_cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn zfs receive: {}", e))?;

        let send_result = send_handle.join().map_err(|_| "Send thread panicked")?;

        let recv_output = recv_child
            .wait_with_output()
            .map_err(|e| format!("Failed to wait for zfs receive: {}", e))?;

        if let Err(e) = send_result {
            return Err(format!("libzetta send failed: {}", e));
        }

        if !recv_output.status.success() {
            let stderr = String::from_utf8_lossy(&recv_output.stderr);
            return Err(format!("zfs receive failed: {}", stderr.trim()));
        }

        Ok(format!("Replicated '{}' to '{}'", snapshot, target_dataset))
    }

    /// Estimate send stream size for a snapshot
    pub async fn estimate_send_size(
        &self,
        snapshot: &str,
        from_snapshot: Option<&str>,
        raw: bool,
        compressed: bool,
    ) -> Result<u64, ZfsError> {
        if !self
            .zfs_engine
            .exists(PathBuf::from(snapshot))
            .map_err(|e| format!("Failed to check snapshot: {}", e))?
        {
            return Err(format!("Snapshot '{}' does not exist", snapshot));
        }

        let c_snapshot =
            CString::new(snapshot).map_err(|_| "Invalid snapshot path: contains null byte")?;

        let c_from: Option<CString> = from_snapshot.and_then(|f| {
            if f.contains('@') {
                CString::new(f).ok()
            } else {
                let dataset = snapshot.split('@').next().unwrap_or(snapshot);
                CString::new(format!("{}@{}", dataset, f)).ok()
            }
        });

        let mut flags: lzc_send_flags::Type = 0;
        if raw {
            flags |= lzc_send_flags::LZC_SEND_FLAG_RAW;
        }
        if compressed {
            flags |= lzc_send_flags::LZC_SEND_FLAG_COMPRESS;
        }
        flags |= lzc_send_flags::LZC_SEND_FLAG_EMBED_DATA;
        flags |= lzc_send_flags::LZC_SEND_FLAG_LARGE_BLOCK;

        let mut size: u64 = 0;

        let result = unsafe {
            lzc_send_space(
                c_snapshot.as_ptr(),
                c_from.as_ref().map(|c| c.as_ptr()).unwrap_or(ptr::null()),
                flags,
                &mut size,
            )
        };

        if result == 0 {
            Ok(size)
        } else {
            Err(format!(
                "lzc_send_space failed with error code {}: {}",
                result,
                errno_to_string(result)
            ))
        }
    }
}
