/// Tracks PIDs of running sandbox processes for cleanup.
///
/// When a `Sandbox` is stored in static (`OnceCell`, `LazyLock`), Rust doesn't run destructors on
/// program exit. The `#[dtor::dtor]` below ensures these orphan processes are killed.
///
/// We must unregister PIDs when the sandbox is killed via signals (SIGINT/SIGTERM) to prevent
/// killing unrelated processes due to PID reuse. Without this, the `dtor` might kill a different
/// process that reused the same PID.
static SANDBOX_PIDS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<u32>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

fn register_pid(pid: u32) {
    SANDBOX_PIDS.lock().unwrap().insert(pid);
}

pub fn unregister_pid(pid: u32) {
    SANDBOX_PIDS.lock().unwrap().remove(&pid);
}

#[dtor::dtor]
/// Global destructor that kills any sandbox processes still running when the program exits.
///
/// This handles the case where `Sandbox` is stored in static variable (OnceCell, LazyLock)
/// and Rust's normal Drop doesn't run.
///
/// NOTE: There is no reliable way to test this easily, only via external scripts that check
/// if there are no `near-sandbox` processes running after the test program exits.
fn cleanup_remaining_sandboxes() {
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

/// When a signal is received, the PID is unregistered to prevent the global destructor
/// from killing an unrelated process (PIDs can be reused after the sandbox dies).
///
/// The actual killing on signals is handled by `.kill_on_drop(true)` on the Child process.
pub fn spawn_signal_listener(pid: u32) -> tokio::task::JoinHandle<()> {
    register_pid(pid);

    tokio::spawn(async move {
        let ctrl_c = tokio::signal::ctrl_c();

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }

        // NOTE: Unregister before the process dies to prevent PID reuse issues.
        // The actual kill is handled by `.kill_on_drop(true)` on the `Child`.
        unregister_pid(pid);
    })
}
