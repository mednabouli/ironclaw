# Security Audit Checklist

> Community security audit for IronClaw v1.0 readiness.
> This document tracks what has been reviewed, by whom, and any findings.

## Audit Scope

| Area | Crate | Priority | Status |
|------|-------|----------|--------|
| ShellTool allowlist bypass | ironclaw-tools | Critical | ✅ Audited (v0.1.1) |
| FileReadTool path traversal | ironclaw-tools | Critical | ✅ Audited (v0.1.1) |
| FileWriteTool path traversal | ironclaw-tools | Critical | ✅ Audited (v0.1.1) |
| REST auth middleware | ironclaw-channels | High | ✅ Audited (v0.1.1) |
| REST auth rate limiting | ironclaw-channels | High | ✅ Implemented (v0.1.1) |
| PII scrubbing completeness | ironclaw-channels | High | ⬜ Pending |
| Prompt injection detection | ironclaw-channels | High | ⬜ Pending |
| WASM sandbox escape | ironclaw-wasm | Critical | ⬜ Pending (wasmtime integration) |
| Provider API key exposure | ironclaw-providers | High | ✅ Debug impls hide keys |
| Session isolation | ironclaw-memory | High | ⬜ Pending |
| Config secret handling | ironclaw-config | Medium | ✅ ${ENV_VAR} expansion |
| Supply chain (dependencies) | workspace | Medium | ✅ cargo-deny + cargo-vet |
| SBOM generation | CI | Medium | ✅ Implemented |
| Binary signing | CI | Medium | ✅ cosign in release workflow |
| Fuzz testing coverage | workspace | Medium | ✅ cargo-fuzz targets added |

## How to Contribute

1. Pick an area marked ⬜ Pending above
2. Review the relevant source code in `crates/`
3. Open a GitHub Issue with the `security-audit` label describing your findings
4. Reference this document and the specific area you reviewed

## Professional Audit

For v1.0.0 release, consider engaging one of:
- [Trail of Bits](https://www.trailofbits.com/) — Rust & systems security
- [Cure53](https://cure53.de/) — web/API security
- [NCC Group](https://www.nccgroup.com/) — comprehensive audit

Budget estimate: $15k–$50k depending on scope and firm.
