//! What an operation returns: its value plus warnings, never printed by core.

/// An operation's result and the warnings it raised along the way. Clients print
/// warnings after the operation's own output (R12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome<T> {
    pub value: T,
    pub warnings: Vec<Warning>,
}

impl<T> Outcome<T> {
    /// An outcome with no warnings.
    pub fn new(value: T) -> Outcome<T> {
        Outcome {
            value,
            warnings: Vec::new(),
        }
    }
}

/// A warning, worded as the Go CLI words it (without the `warning: ` prefix the CLI
/// adds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub message: String,
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
