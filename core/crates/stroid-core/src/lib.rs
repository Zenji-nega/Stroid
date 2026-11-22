//! Stroid core: platform-independent logic shared with the Android app.
//!
//! Phase 0 only exposes a version string and a greeting so the Kotlin <-> Rust
//! bridge can be verified end to end. Editor, terminal, git and search modules
//! are added in later phases (see the project README).

/// Version of the native core, taken from Cargo metadata.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Builds a greeting. Used to verify that strings round-trip over JNI.
pub fn greet(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        "Hello from Stroid core".to_string()
    } else {
        format!("Hello, {name}, from Stroid core")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }

    #[test]
    fn greet_with_name() {
        assert_eq!(greet(" Zenji "), "Hello, Zenji, from Stroid core");
    }

    #[test]
    fn greet_without_name() {
        assert_eq!(greet("   "), "Hello from Stroid core");
    }

    #[test]
    fn greet_handles_unicode() {
        assert_eq!(greet("زينجي"), "Hello, زينجي, from Stroid core");
    }
}
