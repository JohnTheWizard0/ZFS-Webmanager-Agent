# CURL example
# Remove a cache device
curl -X DELETE "http://localhost:9876/v1/pools/tank/vdev/dev/nvme0n1" \
  -H "X-API-Key: $KEY"

API Key: 08670612-43df-4a0c-a556-2288457726a5

## Systemd Service

The ZFS agent runs as a systemd service for reliable operation and testing!!

**Service file:** `/etc/systemd/system/zfs-agent.service`

| Action | Command |
|--------|---------|
| Start service | `systemctl start zfs-agent` |
| Stop service | `systemctl stop zfs-agent` |
| Restart service | `systemctl restart zfs-agent` |
| Check status | `systemctl status zfs-agent` |
| Follow logs | `journalctl -u zfs-agent -f` |
| Rebuild & restart | `cargo build --release && systemctl restart zfs-agent` |

**Categorize for integration tests**
    - tests/zfs_stress_a_long.sh
    - tests/zfs_stress_b_long.sh
    - tests/zfs_stress_a_short.sh
    - tests/zfs_stress_b_short.sh
    - **Test-A**: Dataset, Snapshot, Property stress tests
    - **Test-B**: Pool, Replication, Auth, API edge cases