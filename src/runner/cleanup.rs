//! # Sandbox cleanup module
//!
//! This module handles cleanup of `near-sandbox` child processes across all exit scenarios.
//!
//! ## Why it was created?
//!
//! Mainly this module was created to address the issue when `Sandbox` is stored in a static (`OnceCell`, `LazyLock`), and Rust never calls `Drop`
//!
//! ## Involved mechanisms
//!
//! There are three mechanisms involved, each covering different exit path:
//! - `CleanupGuard` (RAII)
//!     - Registers PID on creation, unregisters on drop
//!     - Handles per-test sandboxes where `Sandbox::drop()` runs normally
//! - `atexit` handler
//!     - Registered once via `libc::atexit`
//!     - Kills any PIDs still in `SANDBOX_PIDS` on normal program exit
//!     - Does NOT run on signal termination
//! - SIGINT handler thread
//!     - Dedicated thread with its own tokio runtime
//!     - Catches Ctrl+C, kills all registered sandboxes, re-raises signal
//!     - Needed because `atexit` doesn't run when a signal kills the process
//!     - On normal exit, this thread is just terminated by the OS (no join needed)
//!
//! ## What's NOT covered
//! - SIGTERM to parent (cargo test) - signal isn't forwarded to test binary.
//! - SIGKILL - can't be caught. `prctl(PR_SET_PDEATHSIG)` on Linux might be improvement for this case, but most of the teams are using MacOS...
//!
//! ## How this module was tested
//! Module was tested against:
//! - [`singleton_sandbox.rs` example](../../examples/singleton_sandbox.rs)
//! - [Intents sandbox (`OnceCell`)](https://github.com/near/intents/blob/d38a46ad77/sandbox/src/lib.rs#L86-L108)
//! - [Intents sandbox (atexit-based)](https://github.com/near/intents/blob/9e45cccb32/sandbox/src/lib.rs#L109-L141)
//!
//! Scenarios verified manually:
//! - Normal exit, per-test sandbox:    Drop kills process, guard unregisters PID, no `atexit` cleanup
//! - Normal exit, static sandbox:      `atexit` cleanup
//! - Normal exit, combined sandbox:    Drop kills process, guard unregisters PID, `atexit` cleanup of static sandbox
//! - Ctrl+C (SIGINT):                  signal handler kills sandboxes, re-raises for clean exit

/// Tracks PIDs of running sandbox processes for cleanup.
///
/// When a `Sandbox` is stored in static (`OnceCell`, `LazyLock`), Rust doesn't run destructors on
/// program exit. The `atexit` handler below ensures these orphan processes are killed.
///
/// We must unregister PIDs when the sandbox is killed via signals (SIGINT/SIGTERM) to prevent
/// killing unrelated processes due to PID reuse. Without this, the `atexit` might kill a different
/// process that reused the same PID.
static SANDBOX_PIDS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<u32>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

/// Ensures `atexit` handler is registered only once
static INIT: std::sync::Once = std::sync::Once::new();

pub struct CleanupGuard {
    pid: u32,
}

impl CleanupGuard {
    /// Register guard with neard child process id.
    ///
    /// This ensures that static Sandboxes are cleaned upon exit from tests with dropping PIDs that
    /// shouldn't be cleaned.
    pub fn new(pid: u32) -> Self {
        // Register atexit handler on first PID registration
        INIT.call_once(|| {
            #[cfg(unix)]
            {
                unsafe {
                    libc::atexit(cleanup_remaining_sandboxes);
                }

                spawn_signal_handler();
            }
        });

        register_pid(pid);

        Self { pid }
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        unregister_pid(self.pid);
    }
}

fn register_pid(pid: u32) {
    SANDBOX_PIDS.lock().unwrap().insert(pid);
}

fn unregister_pid(pid: u32) {
    SANDBOX_PIDS.lock().unwrap().remove(&pid);
}

/// Spawns a dedicated thread to handle SIGINT for sandbox cleanup.
///
/// This exists because `atexit` does NOT run on signal termination (POSIX defines it as abnormal
/// exit). Without this, static sandboxes (`OnceCell`, `LazyLock`) would leak processes when the
/// user presses Ctrl+C
///
/// On normal exit (no signal), this thread is simply terminated by the OS when the process exits.
/// No explicit join should be needed.
#[cfg(unix)]
fn spawn_signal_handler() {
    // Creating new thread to be sure that tokio runtime is initialized even if we close test env
    std::thread::Builder::new()
        .name("near-sandbox-cleanup".to_owned())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
                .expect("signal handler runtime");

            rt.block_on(async {
                let mut sigint =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                        .expect("SIGINT handler");

                sigint.recv().await;

                kill_all_sandboxes();

                // Re-raise signal with default handler so the process terminates naturally (user's handles, atexit, etc.)
                // with the correct exit status. Without this, we might see errors in our test
                // suite when doing CTRL+C
                unsafe {
                    libc::signal(libc::SIGINT, libc::SIG_DFL);
                    libc::raise(libc::SIGINT);
                }
            })
        })
        .expect("failed to spawn signal handler thread");
}

/// Global destructor that kills any sandbox processes still running when the program exits.
///
/// This handles the case where `Sandbox` is stored in static variable (OnceCell, LazyLock)
/// and Rust's normal Drop doesn't run.
///
/// NOTE: There is no reliable way to test this easily, only via external scripts that check
/// if there are no `near-sandbox` processes running after the test program exits.
#[cfg(unix)]
extern "C" fn cleanup_remaining_sandboxes() {
    kill_all_sandboxes();
}

fn kill_all_sandboxes() {
    let Ok(pids) = SANDBOX_PIDS.lock() else {
        return;
    };

    for &pid in pids.iter() {
        #[cfg(unix)]
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }
}
