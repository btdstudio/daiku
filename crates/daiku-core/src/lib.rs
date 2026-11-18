//! daiku's core. It holds no UI: every operation returns data and never prints.

/// The crate's version, as Cargo built it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::VERSION.is_empty());
    }
}
