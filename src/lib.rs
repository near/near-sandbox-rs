pub mod config;
pub mod error_kind;
pub mod sandbox;

mod runner;

// Re-export important types for better user experience
pub use config::{GenesisAccount, SandboxConfig};
pub use sandbox::Sandbox;

// The current version of the sandbox node we want to point to.
// Should be updated to the latest release of nearcore.
// Currently pointing to nearcore@v2.7.0 released on August 12, 2025
pub const DEFAULT_NEAR_SANDBOX_VERSION: &str = "2.7.0";
