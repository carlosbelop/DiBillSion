//! App-facing facade. Kotlin (Android) and Swift (iOS) call into this crate
//! through UniFFI-generated bindings (milestone M3). Keep the API coarse-grained:
//! one call per screen/action, not many small getters.

pub use dibill_domain as domain;

/// Version string shown in the app's "About" screen.
#[must_use]
pub fn core_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
