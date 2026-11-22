//! Stroid core: platform-independent logic shared with the Android app.
//!
//! All positions exposed by the editor are Unicode scalar (`char`) offsets, not
//! bytes and not UTF-16 units. Callers on the JVM side must convert.

pub mod editor;

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
