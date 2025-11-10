pub mod config;
pub mod error_kind;
pub mod sandbox;

mod runner;

// Re-export important types for better user experience
pub use config::{GenesisAccount, SandboxConfig};
pub use sandbox::Sandbox;

// Include the version constant generated at build time
include!(concat!(env!("OUT_DIR"), "/nearcore_version.rs"));
