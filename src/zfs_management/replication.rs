// zfs_management/replication.rs
// Replication operations: send, receive, replicate, estimate_size

use super::helpers::errno_to_string;
use super::manager::ZfsManager;
use super::types::ZfsError;
use libzetta::zfs::{SendFlags, ZfsEngine};
use libzetta_zfs_core_sys::{lzc_send_flags, lzc_send_space};
use std::ffi::CString;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::ptr;

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

        if !output_file.starts_with('/') {
            return Err("Output file path must be absolute".to_string());
        }

        let output_path = std::path::Path::new(output_file);
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
            .open(output_file)
            .map_err(|e| format!("Failed to create output file '{}': {}", output_file, e))?;

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

        let metadata = std::fs::metadata(output_file)
            .map_err(|e| format!("Failed to read output file: {}", e))?;
        Ok(metadata.len())
    }

    /// Receive a snapshot from a file
    pub async fn receive_snapshot_from_file(
        &self,
        target_dataset: &str,
        input_file: &str,
        force: bool,
    ) -> Result<String, ZfsError> {
        if !std::path::Path::new(input_file).exists() {
            return Err(format!("Input file '{}' does not exist", input_file));
        }

        let mut args = vec!["receive".to_string()];

        if force {
            args.push("-F".to_string());
        }

        args.push("-v".to_string());
        args.push(target_dataset.to_string());

        let cmd_str = format!("zfs {} < '{}'", args.join(" "), input_file);

        let output = std::process::Command::new("sh")
            .args(["-c", &cmd_str])
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
