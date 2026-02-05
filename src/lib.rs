//! # Features
//!
//! | Feature | Default | Description |
//! | --- | --- | --- |
//! | `singleton_cleanup` | off | Registers an `atexit` hook and SIGINT handler to kill sandbox
//! processes stored in statics (`OnceCell`, `LazyLock`). Not needed with nextest or per-test
//! sandboxes since `kill_on_drop` already handles cleanup. |
//! | `generate` | off | Enables `random_account_id` and `random_key_pair` helpers |
//! | `global_install` | off | Installs the sandbox binary under `$HOME/.near` instead of `$OUT_DIR` |

pub mod config;
pub mod error_kind;
pub mod sandbox;

mod runner;

// Re-export important types for better user experience
pub use config::{GenesisAccount, SandboxConfig};
pub use runner::install;
pub use sandbox::Sandbox;
pub use sandbox::patch::FetchData;

#[cfg(feature = "generate")]
pub use config::{random_account_id, random_key_pair};

// The current version of the sandbox node we want to point to.
// Should be updated to the latest release of nearcore.
// Currently pointing to nearcore@v2.10.5 released on January 20, 2026
pub const DEFAULT_NEAR_SANDBOX_VERSION: &str = "2.10.5";
