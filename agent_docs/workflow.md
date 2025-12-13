# Development Workflow

ToDo → Plan → Implement → Test → Commit(develop) → Harden → Commit(main) → Archive feature file (_archive)

## Stage Definitions

### 1. ToDo
- Receive task from user OR identify from existing feature backlog
- If no feature file exists: **stop, create it first**
- Create entry in appropriate feature-set file: `features/#XY_FeatureName.md`
- Format:
  ```
  ### FeatureName
  - Status: todo | in-progress | testing | hardening | done
  - Test: tests/path/to/test_file.py
  - Description: Purpose
  - Acceptance: What "done" looks like
  ```

### 2. Plan
- Break down into implementation steps
- Read existing related code before writing new code
- Identify dependencies and potential conflicts
- If feature touches multiple modules, note integration points in feature file
- For complex features: write pseudocode or architecture notes in feature-set file
- Flag anything requiring user approval (new deps, arch changes)
- Update feature file with plan
- Output: Feature file updated, STATUS remains: planning until user confirms

### 3. Implement
- Work through plan step-by-step
- Update `agent_docs/structure.md` if adding modules/files
- STATUS: implementing
- Output: Working code, structure.md current

## 4. Testing
- see agent_docs/testing.md

## 5. Commit to Develop
- **Enforced**: Full test suite must pass (PreToolUse hook blocks otherwise)
- Commit message format: `[#XX-FeatureName] <verb> <what>`
- Update feature status to `testing` or `hardening`
- Use `/commit` slash command for guided process

## 6. Hardening (if required)
Hardening required when:
- Feature handles external input
- Feature modifies persistent state
- Feature is a dependency for other features
Hardening means:
- Extended test scenarios (load, malformed input, interruption)
- Run in realistic environment
- Document known limitations

### 7. Commit (main)
- **User request required**
- Only after hardening or user skip
- Target: main branch

## Branch Model

```
main     ─────●─────────────●─────────────●──────
              ↑             ↑             ↑
develop  ──●──┴──●──●──●────┴──●──●───────┴──●───
           │     │  │  │       │  │          │
           f1    f2 f2 f3      f4 f4         f5
```

- `develop`: Integration branch, may break
- `main`: Stable releases only
- Features merge to develop first, then promote to main

## Status Transitions

```
planning → implementing → testing → done(develop) → done(main)
                ↓              ↓
            (blocked)      (failed)
```

## Blocked?
If implementation reveals missing dependencies or design flaws:
1. Document blocker in feature-set file
2. Mark status as `blocked: [reason]`
3. Ask for guidance before proceeding

---

## ZFS-Specific Guidelines

### libzetta First
Before implementing any ZFS feature:
1. Check if libzetta already supports it
2. Review docs.rs/libzetta and existing code in `src/zfs_management.rs`
3. If libzetta lacks the capability, consult `_resources/ZFS_documentation/` for proper ZFS semantics
4. When researching libzfs/libzetta internals, check local cargo registry first:
   - `~/.cargo/registry/src/*/libzetta-*`
   - `~/.cargo/registry/src/*/libzfs-*`
   - Use grep to search for relevant functions/structs (faster than online docs)

### CLI as Last Resort
**Only after user approval:** Use direct CLI (`zpool`/`zfs`) to implement ZFS features.
- Mark such features as **experimental** in module documentation
- Document the CLI dependency clearly

### ZFS Safety First
ZFS operations are critical. Prefer battle-tested patterns over clever shortcuts.

---

## ZFS Feature Implementation Checklist

When implementing a new ZFS feature, follow this workflow:


1. **Update ZFS/API documentation**
   - Document new/modified endpoints in `openapi.yaml`
   - Update features.json with ZFS Features
   - Ensure request/response schemas are complete

2. **Test the feature**
   - Run targeted tests during development
   - see agent_docs/testing.md
   - Full integration test before commit

3. **Commit to develop**
   - Follow standard commit workflow

