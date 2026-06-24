use once_cell::sync::Lazy;
use regex::RegexSet;

/// A single compiled `RegexSet` matching filenames (not full paths) that are
/// likely to contain credentials or secrets.  We match against the filename
/// only, case-insensitively, to avoid false positives from path components.
static SENSITIVE_PATTERNS: Lazy<RegexSet> = Lazy::new(|| {
    RegexSet::new([
        r"(?i)^\.env$",
        r"(?i)^\.env\.", // .env.local, .env.production, etc.
        r"(?i)\.pem$",
        r"(?i)\.key$",
        r"(?i)\.p12$",
        r"(?i)\.pfx$",
        r"(?i)^credential",
        r"(?i)^secret",
        r"(?i)^password",
        r"(?i)^passwd",
        r"(?i)^token",
        r"(?i)^private[_-]?key",
        r"(?i)^id_rsa",
        r"(?i)^id_dsa",
        r"(?i)^id_ecdsa",
        r"(?i)^id_ed25519",
        r"(?i)^\.netrc$",
        r"(?i)^\.pgpass$",
        r"(?i)^\.aws", // .aws/credentials, .aws/config
        r"(?i)^\.ssh", // .ssh/id_rsa, etc.
        r"(?i)^kubeconfig",
        r"(?i)\.kubeconfig$",
        r"(?i)^service[_-]?account", // GCP service account JSON
        r"(?i)^gcloud",
        r"(?i)^auth\.json$",
        r"(?i)^secrets\.ya?ml$",
        r"(?i)^vault",
    ])
    .expect("sensitive file patterns are valid regexes")
});

/// Returns `true` if the filename (not full path) looks like it may contain
/// secrets or credentials.
pub fn is_sensitive_filename(filename: &str) -> bool {
    SENSITIVE_PATTERNS.is_match(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_dotenv() {
        assert!(is_sensitive_filename(".env"));
        assert!(is_sensitive_filename(".env.local"));
        assert!(is_sensitive_filename(".env.production"));
    }

    #[test]
    fn detects_key_files() {
        assert!(is_sensitive_filename("server.pem"));
        assert!(is_sensitive_filename("private.key"));
        assert!(is_sensitive_filename("id_rsa"));
        assert!(is_sensitive_filename("id_ed25519"));
    }

    #[test]
    fn detects_credential_prefixes() {
        assert!(is_sensitive_filename("credentials.json"));
        assert!(is_sensitive_filename("secrets.yaml"));
        assert!(is_sensitive_filename("password.txt"));
        assert!(is_sensitive_filename("token.txt"));
    }

    #[test]
    fn detects_cloud_configs() {
        assert!(is_sensitive_filename(".aws"));
        assert!(is_sensitive_filename("kubeconfig"));
        assert!(is_sensitive_filename(".kubeconfig"));
        assert!(is_sensitive_filename("service_account.json"));
    }

    #[test]
    fn does_not_flag_safe_files() {
        assert!(!is_sensitive_filename("README.md"));
        assert!(!is_sensitive_filename("main.py"));
        assert!(!is_sensitive_filename("Cargo.toml"));
        assert!(!is_sensitive_filename("config.yaml")); // generic config is fine
    }
}
