use fs4::fs_std::FileExt;
use tokio::process::{Child, Command};

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::error_kind::{SandboxError, TcpError};

#[cfg(feature = "singleton_cleanup")]
pub(crate) mod cleanup;

// Must be an IP address as `neard` expects socket address for network address.
const DEFAULT_RPC_HOST: &str = "127.0.0.1";

pub fn rpc_socket(port: u16) -> String {
    format!("{DEFAULT_RPC_HOST}:{port}")
}

/// Initialize a sandbox node with the provided version and home directory.
pub fn init_with_version(home_dir: impl AsRef<Path>, version: &str) -> Result<Child, SandboxError> {
    let bin_path = ensure_sandbox_bin_with_version(version)?;
    let home_dir = home_dir.as_ref().to_str().unwrap();
    Command::new(&bin_path)
        .envs(log_vars())
        .args(["--home", home_dir, "init", "--fast"])
        .spawn()
        .map_err(SandboxError::RuntimeError)
}

/// Spawn neard process with port reservation guards
///
/// The TcpListeners are held until immediately before spawning to prevent
/// port reallocation by the OS. They are dropped just before Command::spawn()
/// to minimize the race window where another process could claim the ports.
///
/// `stderr` variable is passed to `neard` process and defaults to `Stdio::inherit` if `None` is passed
pub fn run_neard_with_port_guards(
    home_dir: &Path,
    version: &str,
    rpc_listener_guard: tokio::net::TcpSocket,
    net_listener_guard: tokio::net::TcpSocket,
    stderr: Option<Stdio>,
) -> Result<Child, SandboxError> {
    let bin_path = ensure_sandbox_bin_with_version(version)?;

    let rpc_addr = rpc_socket(
        rpc_listener_guard
            .local_addr()
            .map_err(TcpError::LocalAddrError)?
            .port(),
    );

    let net_addr = rpc_socket(
        net_listener_guard
            .local_addr()
            .map_err(TcpError::LocalAddrError)?
            .port(),
    );

    let options = &[
        "--home",
        home_dir.to_str().expect("home_dir is valid utf8"),
        "run",
        "--rpc-addr",
        &rpc_addr,
        "--network-addr",
        &net_addr,
    ];

    // NOTE: Dropping listeners in order to enable usage of ports for neard
    // not the best solution, but at least lowers the window for possible race condition
    drop(rpc_listener_guard);
    drop(net_listener_guard);

    // NOTE: We discard stderr of `neard`, as there might be port collisions resulting in `neard`
    // panicing that `near-sandbox` is taking care of.
    Command::new(&bin_path)
        .args(options)
        .envs(log_vars())
        .stderr(stderr.unwrap_or(Stdio::inherit()))
        .kill_on_drop(true)
        .spawn()
        .map_err(SandboxError::RuntimeError)
}

const fn platform() -> Option<&'static str> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Some("Linux-x86_64");

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return Some("Linux-aarch64");

    // Darwin-x86_64 is not supported for some time now.
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return None;

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return Some("Darwin-arm64");

    #[cfg(all(
        not(target_os = "macos"),
        not(all(target_os = "linux", target_arch = "x86_64")),
        not(all(target_os = "linux", target_arch = "aarch64"))
    ))]
    return None;
}

/// Installs sandbox node with the default version. This is a version that is usually stable
/// and has landed into mainnet to reflect the latest stable features and fixes.
pub fn install() -> Result<PathBuf, SandboxError> {
    ensure_sandbox_bin_with_version(crate::DEFAULT_NEAR_SANDBOX_VERSION)
}

// if the `SANDBOX_ARTIFACT_URL` env var is set, we short-circuit and use that.
fn bin_url(version: &str) -> Option<String> {
    if let Ok(val) = std::env::var("SANDBOX_ARTIFACT_URL") {
        return Some(val);
    }

    Some(format!(
        "https://s3-us-west-1.amazonaws.com/build.nearprotocol.com/nearcore/{}/{}/near-sandbox.tar.gz",
        platform()?,
        version,
    ))
}

/// Check if the sandbox version is already downloaded to the bin path.
/// It does not disambiguate between a commit hash and a tagged version, so it's recommeded to
/// pick one format and stick to it.
fn check_for_version(version: &str) -> Result<Option<PathBuf>, SandboxError> {
    // short circuit if we are using the sandbox binary from the environment
    if let Ok(bin_path) = &std::env::var("NEAR_SANDBOX_BIN_PATH") {
        return Ok(Some(PathBuf::from(bin_path)));
    }

    // version saved under {home}/.near/near-sandbox-{version}/near-sandbox
    let out_dir = download_path(version).join("near-sandbox");
    if !out_dir.exists() {
        return Ok(None);
    }

    Ok(Some(out_dir))
}

/// Install the sandbox node given the version, which is either a commit hash or tagged version
/// number from the nearcore project. Note that commits pushed to master within the latest 12h
/// will likely not have the binaries made available quite yet.
fn install_with_version(version: &str) -> Result<PathBuf, SandboxError> {
    if let Some(bin_path) = check_for_version(version)? {
        return Ok(bin_path);
    }

    let url = bin_url(version).ok_or_else(|| {
        SandboxError::UnsupportedPlatformError(
            "only linux-x86_64, linux-aarch64, and darwin-arm64 are supported".to_owned(),
        )
    })?;

    // Download and extract the tar.gz archive
    let response = ureq::get(&url)
        .config()
        .timeout_connect(Some(std::time::Duration::from_secs(30)))
        .timeout_recv_response(Some(std::time::Duration::from_secs(30)))
        .build()
        .call()
        .map_err(|e| SandboxError::DownloadError(e.to_string()))?;

    let decoder = flate2::read::GzDecoder::new(response.into_body().into_reader());
    let mut archive = tar::Archive::new(decoder);

    let dest = download_path(version).join("near-sandbox");

    for entry in archive
        .entries()
        .map_err(|e| SandboxError::InstallError(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| SandboxError::InstallError(e.to_string()))?;
        let path = entry
            .path()
            .map_err(|e| SandboxError::InstallError(e.to_string()))?;

        if path.file_name() == Some(std::ffi::OsStr::new("near-sandbox"))
            && entry.header().entry_type().is_file()
        {
            // Unpack to a temporary file first, then atomically rename into place.
            // This prevents a partial file from being treated as a valid binary
            // if extraction is interrupted (e.g. network drop, disk full).
            let tmp_dest = dest.with_extension("tmp");
            entry
                .unpack(&tmp_dest)
                .map_err(|e| SandboxError::InstallError(e.to_string()))?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&tmp_dest, std::fs::Permissions::from_mode(0o755))
                    .map_err(SandboxError::FileError)?;
            }

            std::fs::rename(&tmp_dest, &dest).map_err(SandboxError::FileError)?;

            return Ok(dest);
        }
    }

    Err(SandboxError::InstallError(
        "near-sandbox binary not found in archive".to_owned(),
    ))
}

fn installable(bin_path: &Path) -> Result<Option<std::fs::File>, SandboxError> {
    // Sandbox bin already exists
    if bin_path.exists() {
        return Ok(None);
    }

    let mut lockpath = bin_path.to_path_buf();
    lockpath.set_extension("lock");

    // Acquire the lockfile
    let lockfile = File::create(lockpath).map_err(SandboxError::FileError)?;
    lockfile.lock_exclusive().map_err(SandboxError::FileError)?;

    // Check again after acquiring if no one has written to the dest path
    if bin_path.exists() {
        Ok(None)
    } else {
        Ok(Some(lockfile))
    }
}

fn normalize_name(input: &str) -> String {
    input.replace('/', "_")
}

// Returns a path to the binary in the form of: `{home}/.near/near-sandbox-{version}` || `{$OUT_DIR}/.near/near-sandbox-{version}`
fn download_path(version: &str) -> PathBuf {
    #[cfg(feature = "global_install")]
    let mut out = dirs_next::home_dir().expect("could not retrieve home_dir");
    #[cfg(not(feature = "global_install"))]
    let mut out = PathBuf::from(env!("OUT_DIR"));

    out.push(".near");
    out.push(format!("near-sandbox-{}", normalize_name(version)));
    if !out.exists() {
        std::fs::create_dir_all(&out).expect("could not create download path");
    }

    out
}

/// Returns a path to the binary in the form of {home}/.near/near-sandbox-{version}/near-sandbox
fn bin_path(version: &str) -> Result<PathBuf, SandboxError> {
    if let Ok(path) = std::env::var("NEAR_SANDBOX_BIN_PATH") {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(SandboxError::BinaryError(format!(
                "{} does not exists",
                path.display()
            )));
        }
        return Ok(path);
    }

    let mut buf = download_path(version);
    buf.push("near-sandbox");

    Ok(buf)
}

fn ensure_sandbox_bin_with_version(version: &str) -> Result<PathBuf, SandboxError> {
    let mut bin_path = bin_path(version)?;
    if let Some(lockfile) = installable(&bin_path)? {
        bin_path = install_with_version(version)?;
        FileExt::unlock(&lockfile).map_err(SandboxError::FileError)?;
    }

    Ok(bin_path)
}

fn log_vars() -> Vec<(String, String)> {
    let mut vars = Vec::new();
    if let Ok(val) = std::env::var("NEAR_SANDBOX_LOG") {
        vars.push(("RUST_LOG".into(), val));
    }
    if let Ok(val) = std::env::var("NEAR_SANDBOX_LOG_STYLE") {
        vars.push(("RUST_LOG_STYLE".into(), val));
    }
    vars
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::sync::Mutex;

    // `NEAR_SANDBOX_BIN_PATH` is process-wide state. Serialize every test that
    // reads or writes it so they don't race each other under the (multi-threaded)
    // test runner.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Sets `NEAR_SANDBOX_BIN_PATH` for the duration of the guard and restores
    /// whatever value (or absence of one) preceded it when dropped, including on
    /// the panicking/failure path.
    struct EnvVarGuard {
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(value: &OsStr) -> Self {
            let previous = std::env::var("NEAR_SANDBOX_BIN_PATH").ok();
            unsafe {
                std::env::set_var("NEAR_SANDBOX_BIN_PATH", value);
            }
            Self { previous }
        }

        fn unset() -> Self {
            let previous = std::env::var("NEAR_SANDBOX_BIN_PATH").ok();
            unsafe {
                std::env::remove_var("NEAR_SANDBOX_BIN_PATH");
            }
            Self { previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.previous {
                    Some(val) => std::env::set_var("NEAR_SANDBOX_BIN_PATH", val),
                    None => std::env::remove_var("NEAR_SANDBOX_BIN_PATH"),
                }
            }
        }
    }

    /// `download_path` creates the version directory as a side effect of
    /// resolving a path, so resolving a made-up version leaves an empty
    /// directory behind — under `global_install` that is the real `~/.near`.
    /// Remove it again, but only while it is still empty, so a directory that
    /// holds an actual downloaded binary is never touched.
    struct VersionDirGuard {
        dir: PathBuf,
    }

    impl VersionDirGuard {
        fn of(bin_path: &Path) -> Self {
            Self {
                dir: bin_path
                    .parent()
                    .expect("resolved binary path has a parent directory")
                    .to_path_buf(),
            }
        }
    }

    impl Drop for VersionDirGuard {
        fn drop(&mut self) {
            if let Ok(mut entries) = std::fs::read_dir(&self.dir) {
                if entries.next().is_none() {
                    let _ = std::fs::remove_dir(&self.dir);
                }
            }
        }
    }

    #[test]
    fn bin_path_resolves_independently_per_version_without_override() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _guard = EnvVarGuard::unset();

        let path_a = bin_path("version-a").expect("resolves for version a");
        let path_b = bin_path("version-b").expect("resolves for version b");
        let _dir_a = VersionDirGuard::of(&path_a);
        let _dir_b = VersionDirGuard::of(&path_b);

        assert_ne!(path_a, path_b);
        assert_eq!(path_a.file_name().unwrap(), "near-sandbox");
        assert_eq!(path_b.file_name().unwrap(), "near-sandbox");
        assert!(path_a.to_string_lossy().contains("version-a"));
        assert!(path_b.to_string_lossy().contains("version-b"));
    }

    #[test]
    fn bin_path_returns_override_when_set_to_existing_file() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let tmp = tempfile::NamedTempFile::new().expect("create temp file");
        let _guard = EnvVarGuard::set(tmp.path().as_os_str());

        let resolved = bin_path("any-version").expect("resolves via override");
        assert_eq!(resolved, tmp.path());
    }

    #[test]
    fn bin_path_errors_when_override_points_to_missing_file() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let missing = std::env::temp_dir().join("near-sandbox-rs-test-does-not-exist");
        let _guard = EnvVarGuard::set(missing.as_os_str());

        let result = bin_path("any-version");
        assert!(matches!(result, Err(SandboxError::BinaryError(_))));
    }

    #[test]
    fn check_for_version_returns_override_when_set() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let tmp = tempfile::NamedTempFile::new().expect("create temp file");
        let _guard = EnvVarGuard::set(tmp.path().as_os_str());

        let resolved = check_for_version("any-version").expect("resolves via override");
        assert_eq!(resolved, Some(tmp.path().to_path_buf()));
    }
}
