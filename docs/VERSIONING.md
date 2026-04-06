# Versioning Policy

> Defines IronClaw's approach to Semantic Versioning, Long-Term Support,
> and the deprecation lifecycle.
>
> See also: [`STABILITY.md`](STABILITY.md) for what is stable vs experimental,
> [`MIGRATION.md`](MIGRATION.md) for upgrade instructions.

---

## Semantic Versioning

IronClaw follows [Semantic Versioning 2.0.0](https://semver.org/):

```
MAJOR.MINOR.PATCH
```

| Bump | Meaning | Example |
|------|---------|---------|
| **MAJOR** | Breaking API change | Removing a trait method, changing return types |
| **MINOR** | Backwards-compatible addition | New provider, new enum variant, new default trait method |
| **PATCH** | Backwards-compatible fix | Bug fix, documentation correction, dependency patch |

### Pre-1.0 Rules (current)

During the `0.x` series:

- **Minor** bumps (`0.1 → 0.2`) may contain breaking changes, but every
  break is documented in `CHANGELOG.md`.
- **Patch** bumps (`0.1.0 → 0.1.1`) are always backwards-compatible.
- MSRV bumps are treated as minor breaking changes.

### Post-1.0 Rules

After `1.0.0`:

- Breaking changes require a **major** bump.
- MSRV bumps require a **minor** bump at minimum.
- `#[non_exhaustive]` allows adding enum variants and struct fields in
  **minor** releases without a major bump.

---

## Release Cadence

| Release type | Frequency | Description |
|-------------|-----------|-------------|
| **Patch** | As needed | Bug fixes, security patches |
| **Minor** | ~Monthly | New features, new providers/channels |
| **Major** | ~Annually | Breaking API changes (batched) |

Breaking changes are **batched** into major releases rather than spread
across many small breaks. Between majors, deprecated items remain
functional for at least one minor release.

---

## Long-Term Support (LTS)

Starting from 1.0, IronClaw will designate **LTS releases**:

| Policy | Detail |
|--------|--------|
| **LTS cadence** | Every other major version (`1.x`, `3.x`, `5.x`, …) |
| **Support window** | 12 months of patch releases after the next major is published |
| **What's included** | Security fixes, critical bug fixes |
| **What's excluded** | New features, performance improvements |

**Example timeline:**

```
v1.0.0 (LTS) ──── v1.1, v1.2, … ──── v2.0.0 ──── v1.x EOL (12 months after v2.0)
                                        v2.0 is NOT LTS
v3.0.0 (LTS) ──── v3.1, v3.2, … ──── v4.0.0 ──── v3.x EOL (12 months after v4.0)
```

---

## Deprecation Process

Items are deprecated before removal to give downstream crates time to migrate.

### Step 1 — Mark as Deprecated

```rust
#[deprecated(since = "0.3.0", note = "Use `CompletionRequest::builder()` instead")]
pub fn new_request(model: &str) -> CompletionRequest {
    // ...
}
```

### Step 2 — Compiler Warnings

The deprecated item continues to function. Users see a compiler warning
with a migration hint for **at least one minor release**.

### Step 3 — Removal

- During `0.x`: removed in the next minor release after the deprecation
  notice was published.
- During `1.x+`: removed only in the next **major** release.

### Deprecation Summary

| Version range | Minimum warning period | Removed in |
|---------------|----------------------|------------|
| `0.x` | 1 minor release | Next minor |
| `1.x+` | 1 minor release | Next major |

---

## Crate Versioning

All crates in the workspace share a single version number defined in the
root `Cargo.toml` under `[workspace.package]`. This means:

- Every crate is published at the same version.
- A breaking change in **any** crate bumps the version for **all** crates.
- This simplifies dependency management — users only track one version.

---

## MSRV Policy

The Minimum Supported Rust Version is declared in `Cargo.toml`:

```toml
[workspace.package]
rust-version = "1.86"
```

- MSRV is bumped only when a dependency or language feature requires it.
- MSRV bumps are treated as breaking (minor bump during 0.x, major bump
  after 1.0).
- MSRV is tested in CI on every PR.

---

## Git Tags and Releases

```
v0.1.0    ← git tag, GitHub Release, crates.io publish
v0.1.1    ← patch
v0.2.0    ← minor (may break during 0.x)
v1.0.0    ← first stable release
```

Every release includes:

1. A git tag (`vX.Y.Z`)
2. A GitHub Release with auto-generated notes
3. A `CHANGELOG.md` entry
4. A crates.io publish for all workspace crates

---

## Checking for SemVer Compliance

```bash
cargo install cargo-semver-checks
cargo semver-checks check-release
```

This is run in CI to prevent accidental breaking changes in patch/minor releases.
