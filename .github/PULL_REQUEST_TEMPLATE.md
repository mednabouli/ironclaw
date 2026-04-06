## Summary

<!-- One paragraph describing what this PR changes and why. -->

Closes #<!-- issue number -->

---

## Changes

<!-- Bullet list of specific changes made. -->

-
-
-

---

## Checklist

### Automated (CI will verify these)
- [ ] `cargo test --workspace --all-features` passes
- [ ] `cargo clippy --workspace --all-features -- -D warnings` clean
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo deny check` passes
- [ ] `cargo doc --workspace --no-deps` has zero warnings

### Manual
- [ ] New public items have `///` doc comments
- [ ] New features have unit tests (happy path + at least one error case)
- [ ] No `unwrap()` / `expect()` in library code
- [ ] No `println!()` in library code — used `tracing::` instead
- [ ] `CHANGELOG.md` updated under `[Unreleased]`
- [ ] Breaking changes to `ironclaw-core` noted in PR description

---

## Type of Change

- [ ] 🐛 Bug fix (non-breaking)
- [ ] ✨ New feature (non-breaking)
- [ ] 💥 Breaking change (requires RFC + community notice)
- [ ] 🔐 Security fix
- [ ] 📚 Documentation only
- [ ] ⚡ Performance improvement
- [ ] 🧹 Chore (deps, CI, tooling)

---

## Testing

<!-- Describe how you tested this change. -->

```bash
# Commands you ran to verify
cargo test -p ironclaw-<crate>
```

---

## Screenshots / Output

<!-- If this changes CLI output or REST responses, paste before/after here. -->
