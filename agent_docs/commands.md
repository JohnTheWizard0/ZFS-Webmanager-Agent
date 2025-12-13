# Platform Commands

> Target OS: [windows | linux]

## File Operations

| Action | Command |
|--------|---------|
| List directory | `dir` / `ls -la` |
| Create directory | `mkdir [path]` |
| Remove directory | `rmdir /s /q [path]` / `rm -rf [path]` |
| Copy file | `copy` / `cp` |
| Move file | `move` / `mv` |

## Remote/SSH

### Key-Based Auth (per project)

Setup sequence (user gives password once during step 2):
```bash
# 1. Generate key
mkdir -p .ssh
ssh-keygen -t ed25519 -f "$PWD/.ssh/project_vm_key" -N "" -C "claude-agent-projectname"

# 2. Install on remote (requires password)
ssh [user]@[host] "mkdir -p ~/.ssh && chmod 700 ~/.ssh && cat >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys" < "$PWD/.ssh/project_vm_key.pub"

# 3. Verify (no password)
ssh -i "$PWD/.ssh/project_vm_key" -o StrictHostKeyChecking=no [user]@[host] "echo 'Success'"
```

### Command Patterns

| Action | Command |
|--------|---------|
| SSH (with key) | `ssh -i "$PWD/.ssh/project_vm_key" [user]@[host] "[cmd]"` |
| SSH (password) | `ssh [user]@[host] -p [port]` |
| SCP upload | `scp -i "$PWD/.ssh/project_vm_key" [local] [user]@[host]:[remote]` |
| SCP download | `scp -i "$PWD/.ssh/project_vm_key" [user]@[host]:[remote] [local]` |

### Cleanup (project end)
```bash
rm -rf .ssh/  # removes local key, revokes access
```

## Environment

| Variable | Value |
|----------|-------|
| Python | `[path or command]` |
| Node | `[path or command]` |
| Package manager | `[npm/yarn/pnpm/pip/etc]` |

```

## Notes

- SSH key lives in project `.ssh/` folder — isolated per project
- Key has no passphrase (automation-friendly)
- User/password auth remains functional parallel to key auth
