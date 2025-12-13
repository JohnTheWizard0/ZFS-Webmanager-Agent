# CLI-Based ZFS Functionalities

This document tracks all ZFS operations that rely on CLI commands (`zfs`/`zpool`) rather than library bindings (libzetta/libzfs FFI).

> **Policy**: Per CLAUDE.md rule #8, CLI should only be used as a last resort after user approval, and such features must be marked as **experimental**.

---

## Summary

| Operation | CLI Command | Reason | Status |
|-----------|-------------|--------|--------|
| Set dataset property | `zfs set` | libzetta/libzfs FFI lacks property setting | EXPERIMENTAL |
| Import pool with rename | `zpool import old new` | libzetta doesn't expose rename on import | CLI fallback |
| Receive snapshot | `zfs receive` | lzc_receive() too low-level (no stream header parsing) | CLI |
| Replicate (receive part) | `zfs receive` | Hybrid: libzetta send + CLI receive | HYBRID |
| Send dry-run estimation | `zfs send -n -P` | Size estimation before actual send | CLI |
| Execute command | User-provided | Generic command execution endpoint | CLI |

---

## Detailed Documentation

### 1. Set Dataset Property

**Endpoint**: `PUT /v1/datasets/{path}/properties`

**Implementation**: [src/zfs_management.rs:386](../src/zfs_management.rs#L386)

```rust
let output = std::process::Command::new("zfs")
    .args(["set", &format!("{}={}", property, value), name])
    .output()
```

**Why CLI?**
- libzetta doesn't expose property setting
- libzfs FFI bindings don't include `zfs_prop_set()`
- Would require FROM-SCRATCH FFI implementation

**Security**: Property names are validated against safe patterns to prevent injection.

---

### 2. Import Pool with Rename

**Endpoint**: `POST /v1/pools/import` (with `new_name` parameter)

**Implementation**: [src/zfs_management.rs:217](../src/zfs_management.rs#L217)

```rust
let mut cmd = Command::new("zpool");
cmd.arg("import");
if let Some(d) = dir {
    cmd.arg("-d").arg(d);
}
cmd.arg(name).arg(new_name);
```

**Why CLI?**
- libzetta's `import()` and `import_from_dir()` don't support renaming
- ZFS rename-on-import requires passing both old and new names to `zpool import`

**Note**: Standard import (without rename) uses libzetta.

---

### 3. Receive Snapshot from File

**Endpoint**: `POST /v1/datasets/{path}/receive`

**Implementation**: [src/zfs_management.rs:984](../src/zfs_management.rs#L984)

```rust
let cmd_str = format!("zfs {} < '{}'", args.join(" "), input_file);
let output = std::process::Command::new("sh")
    .args(["-c", &cmd_str])
    .output()
```

**Why CLI?**
- `lzc_receive()` FFI is too low-level
- Doesn't parse stream headers (dmu_replay_record)
- Would need `lzc_receive_one()` with complex stream parsing
- CLI `zfs receive` is battle-tested and handles all stream formats

---

### 4. Replicate Snapshot (Hybrid)

**Endpoint**: `POST /v1/snapshots/{dataset}/{snapshot}/replicate`

**Implementation**: [src/zfs_management.rs:1097](../src/zfs_management.rs#L1097)

```rust
// Send via libzetta (writes to pipe)
engine.send_full(PathBuf::from(&snapshot_owned), pipe_write, flags)

// Receive via CLI (reads from pipe)
let mut recv_cmd = std::process::Command::new("zfs");
recv_cmd.arg("receive");
recv_cmd.stdin(unsafe { std::process::Stdio::from_raw_fd(pipe_read_fd) });
```

**Why Hybrid?**
- libzetta's `send_full()`/`send_incremental()` work well
- But `lzc_receive()` FFI is too low-level (same as #3)
- Solution: Pipe libzetta send → CLI receive

---

### 5. Send Dry-Run Size Estimation

**Endpoint**: `POST /v1/snapshots/{dataset}/{snapshot}/send` (with `dry_run: true`)

**Implementation**: [src/handlers.rs:1003](../src/handlers.rs#L1003)

```rust
let mut args = vec!["send", "-n", "-P"];
if body.raw { args.push("-w"); }
if body.recursive { args.push("-R"); }
args.push(&full_snapshot);

let output = Command::new("zfs").args(&args).output();
```

**Why CLI?**
- Quick size estimation without actual send
- Parses `zfs send -n -P` output for "size" line
- Alternative: `lzc_send_space()` FFI is used for `GET /send-size` endpoint

**Note**: The `GET /v1/snapshots/{ds}/{snap}/send-size` endpoint uses FROM-SCRATCH `lzc_send_space()` FFI instead.

---

### 6. Execute Arbitrary Command

**Endpoint**: `POST /v1/command`

**Implementation**: [src/handlers.rs:858](../src/handlers.rs#L858)

```rust
let mut cmd = Command::new(&body.command);
if let Some(args) = body.args {
    cmd.args(args);
}
```

**Why CLI?**
- Intentionally exposes shell command execution
- For advanced operations not covered by API
- **Use with caution** - security risk

---

## OpenAPI Documentation

All CLI-based operations are marked in [openapi.yaml](../openapi.yaml):

- **CLI-BASED**: Falls back to `zfs`/`zpool` commands
- **HYBRID**: Combination (e.g., libzetta send + CLI receive)
- **EXPERIMENTAL**: Marked in schema descriptions

Example from OpenAPI:
```yaml
ReceiveSnapshotRequest:
  description: |
    **CLI-BASED**: Request to receive a snapshot from a file.
    Uses `zfs receive` command because lzc_receive() is too low-level
    (doesn't parse stream headers properly).
```

---

## Future Improvements

To reduce CLI dependency:

1. **Property Setting**: Implement FROM-SCRATCH `zfs_prop_set()` FFI
2. **Import with Rename**: Investigate libzetta PR or direct FFI
3. **Receive**: Implement proper `lzc_receive_one()` with stream header parsing

---

## References

- [CLAUDE.md](../CLAUDE.md) - Rule #8: libzetta First policy
- [openapi.yaml](../openapi.yaml) - API specification with CLI markers
- [src/zfs_management.rs](../src/zfs_management.rs) - ZFS operations implementation
- [src/handlers.rs](../src/handlers.rs) - HTTP request handlers
