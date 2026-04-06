//! Capability model for WASM plugin sandboxing.
//!
//! Each plugin declares a set of capabilities it requires. The host only
//! provides the corresponding imports to the WASM module. Capabilities
//! not declared in the plugin manifest are unavailable and will trap.

use serde::{Deserialize, Serialize};

/// A capability that a WASM plugin can request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Capability {
    /// Outbound HTTP requests via `host.http-fetch`.
    Http,
    /// Sandboxed filesystem access via `host.fs-*`.
    Filesystem,
    /// Environment variable access via `host.env-get`.
    Env,
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http => write!(f, "http"),
            Self::Filesystem => write!(f, "filesystem"),
            Self::Env => write!(f, "env"),
        }
    }
}

impl std::str::FromStr for Capability {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "http" => Ok(Self::Http),
            "filesystem" | "fs" => Ok(Self::Filesystem),
            "env" => Ok(Self::Env),
            other => Err(anyhow::anyhow!("Unknown capability: '{other}'")),
        }
    }
}

/// The set of capabilities granted to a plugin, with optional constraints.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityGrant {
    /// Allowed capabilities.
    pub capabilities: Vec<Capability>,
    /// For HTTP capability: list of allowed URL prefixes (empty = allow all).
    #[serde(default)]
    pub allowed_urls: Vec<String>,
    /// For filesystem capability: sandbox root directory.
    /// Files are only accessible under this path.
    #[serde(default)]
    pub sandbox_dir: Option<String>,
    /// For env capability: list of allowed environment variable names.
    #[serde(default)]
    pub allowed_env_vars: Vec<String>,
}

impl CapabilityGrant {
    /// Check if a specific capability is granted.
    pub fn has(&self, cap: &Capability) -> bool {
        self.capabilities.contains(cap)
    }

    /// Validate an HTTP URL against the allowlist.
    ///
    /// Returns `true` if the HTTP capability is granted and either the
    /// allowlist is empty (permit all) or the URL starts with one of
    /// the allowed prefixes.
    pub fn check_url(&self, url: &str) -> bool {
        if !self.has(&Capability::Http) {
            return false;
        }
        if self.allowed_urls.is_empty() {
            return true;
        }
        self.allowed_urls
            .iter()
            .any(|prefix| url.starts_with(prefix))
    }

    /// Validate a filesystem path against the sandbox directory.
    pub fn check_path(&self, path: &str) -> bool {
        if !self.has(&Capability::Filesystem) {
            return false;
        }
        match &self.sandbox_dir {
            Some(root) => {
                // Prevent path traversal
                let normalized = path.replace('\\', "/");
                !normalized.contains("..") && normalized.starts_with(root.as_str())
            }
            None => false, // No sandbox dir configured = deny all fs access
        }
    }

    /// Validate an environment variable name against the allowlist.
    pub fn check_env_var(&self, name: &str) -> bool {
        if !self.has(&Capability::Env) {
            return false;
        }
        self.allowed_env_vars.iter().any(|v| v == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_from_str() {
        assert_eq!("http".parse::<Capability>().unwrap(), Capability::Http);
        assert_eq!(
            "filesystem".parse::<Capability>().unwrap(),
            Capability::Filesystem
        );
        assert_eq!("fs".parse::<Capability>().unwrap(), Capability::Filesystem);
        assert_eq!("env".parse::<Capability>().unwrap(), Capability::Env);
        assert!("unknown".parse::<Capability>().is_err());
    }

    #[test]
    fn capability_display() {
        assert_eq!(Capability::Http.to_string(), "http");
        assert_eq!(Capability::Filesystem.to_string(), "filesystem");
        assert_eq!(Capability::Env.to_string(), "env");
    }

    #[test]
    fn grant_has_capability() {
        let grant = CapabilityGrant {
            capabilities: vec![Capability::Http],
            ..Default::default()
        };
        assert!(grant.has(&Capability::Http));
        assert!(!grant.has(&Capability::Env));
    }

    #[test]
    fn check_url_no_cap_denied() {
        let grant = CapabilityGrant::default();
        assert!(!grant.check_url("https://example.com"));
    }

    #[test]
    fn check_url_with_cap_no_allowlist() {
        let grant = CapabilityGrant {
            capabilities: vec![Capability::Http],
            ..Default::default()
        };
        assert!(grant.check_url("https://anything.com"));
    }

    #[test]
    fn check_url_with_allowlist() {
        let grant = CapabilityGrant {
            capabilities: vec![Capability::Http],
            allowed_urls: vec![
                "https://wttr.in/".into(),
                "https://api.coingecko.com/".into(),
            ],
            ..Default::default()
        };
        assert!(grant.check_url("https://wttr.in/London"));
        assert!(grant.check_url("https://api.coingecko.com/api/v3/simple/price"));
        assert!(!grant.check_url("https://evil.com/steal-data"));
    }

    #[test]
    fn check_path_no_cap_denied() {
        let grant = CapabilityGrant::default();
        assert!(!grant.check_path("/tmp/file.txt"));
    }

    #[test]
    fn check_path_traversal_blocked() {
        let grant = CapabilityGrant {
            capabilities: vec![Capability::Filesystem],
            sandbox_dir: Some("/sandbox".into()),
            ..Default::default()
        };
        assert!(!grant.check_path("/sandbox/../etc/passwd"));
        assert!(grant.check_path("/sandbox/data.json"));
    }

    #[test]
    fn check_env_var_restricted() {
        let grant = CapabilityGrant {
            capabilities: vec![Capability::Env],
            allowed_env_vars: vec!["API_KEY".into()],
            ..Default::default()
        };
        assert!(grant.check_env_var("API_KEY"));
        assert!(!grant.check_env_var("SECRET_TOKEN"));
    }
}
