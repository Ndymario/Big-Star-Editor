//! Big Star Editor `level` layer. Populated by a later phase of docs/rust-port-plan.md.

/// Crate name, so the stub has something to test.
pub const NAME: &str = "bigstar-level";

#[cfg(test)]
mod tests {
    #[test]
    fn name_is_set() {
        assert_eq!(super::NAME, "bigstar-level");
    }
}
