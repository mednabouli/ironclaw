# 🔐 Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x (latest) | ✅ Active |
| < 0.1.0 | ❌ Not supported |

Once v1.0.0 is released, the two most recent minor versions will receive security backports.

---

## Reporting a Vulnerability

**Please do NOT report security vulnerabilities via GitHub Issues.** Public issues expose users before a fix is available.

### Preferred: GitHub Private Advisory

1. Go to the [Security tab](https://github.com/mednabouli/ironclaw/security/advisories)
2. Click **"Report a vulnerability"**
3. Fill in the details — include reproduction steps, affected versions, and impact assessment

### Alternative: Email

Send a PGP-encrypted email to `security@ironclaw.dev`  
PGP key: _(published on keys.openpgp.org — fingerprint TBD at launch)_

---

## What to Include

A good report includes:

- **Summary** — one sentence describing the issue
- **Affected component** — which crate, file, or feature
- **Affected versions** — which versions are vulnerable
- **Reproduction steps** — minimal working example to trigger the issue
- **Impact** — what an attacker can achieve (data exposure, RCE, DoS, etc.)
- **Suggested fix** (optional) — if you have one

---

## Response Timeline

| Stage | Target Time |
|-------|-------------|
| Acknowledgement | Within 48 hours |
| Severity assessment | Within 5 business days |
| Fix developed | Within 14 days (critical) / 30 days (high) |
| Coordinated disclosure | After fix is published |
| CVE assignment | Requested for CVSS ≥ 7.0 |

We follow [responsible disclosure](https://en.wikipedia.org/wiki/Coordinated_vulnerability_disclosure). We will credit reporters in the release notes unless they prefer anonymity.

---

## Threat Model

IronClaw is a local-first AI agent runtime. Key security boundaries:

### WASM Plugin Sandbox _(v0.8.0+)_
Plugins execute in `wasmtime` with WASI capability-based permissions. By default:
- ❌ No filesystem access (unless `allow_fs = true` in `plugin.toml`)
- ❌ No network access (unless specific hosts allowlisted)
- ❌ No process spawning
- ✅ CPU + memory limits enforced

A vulnerability that escapes the WASM sandbox is **Critical severity**.

### Shell Tool
The `ShellTool` only executes commands in the explicit `[tools.shell] allowlist`.  
A bypass of the allowlist is **High severity**.

### REST Channel
The REST channel authenticates via Bearer token from `[channels.rest] auth_token`.  
Token bypass or injection vulnerabilities are **High severity**.

### Provider API Keys
API keys are read from environment variables or `ironclaw.toml`.  
They must never appear in logs, debug output, error messages, or traces.  
A key exposure vulnerability is **High severity**.

### Memory / Session Isolation
Different `session_id` values must never share memory history.  
A cross-session data leak is **High severity**.

---

## Known Limitations

- `ShellTool` with a permissive allowlist can be abused by a compromised LLM — keep the allowlist minimal
- `ironclaw.toml` stores config in plaintext — use `${ENV_VAR}` references instead of hardcoding secrets
- The REST channel does not implement rate limiting in v0.1.x — expose only on trusted networks

---

## Security-Relevant Configuration

```toml
# ironclaw.toml — security best practices

[channels.rest]
# Always set a strong auth token for REST channel
auth_token = "${REST_AUTH_TOKEN}"

# Bind to localhost only unless you need external access
host = "127.0.0.1"
port = 8080

[tools.shell]
# Keep allowlist as minimal as possible
allowlist = ["echo", "date"]  # NOT ["bash", "sh", "python"]
timeout_secs = 10             # Short timeout limits damage

[providers.claude]
# Use env vars — never hardcode
api_key = "${ANTHROPIC_API_KEY}"
```

---

## Disclosure History

_No disclosed vulnerabilities to date._

---

## Acknowledgements

We thank all security researchers who responsibly disclose vulnerabilities.  
Hall of fame will be maintained here after the first coordinated disclosure.
