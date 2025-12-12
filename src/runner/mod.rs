use binary_install::Cache;
use fs4::FileExt;
use tokio::process::{Child, Command};

use std::fs::File;
use std::path::{Path, PathBuf};

use crate::error_kind::{SandboxError, TcpError};

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
pub fn run_neard_with_port_guards(
    home_dir: &Path,
    version: &str,
    rpc_listener_guard: tokio::net::TcpSocket,
    net_listener_guard: tokio::net::TcpSocket,
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

    Command::new(&bin_path)
        .args(options)
        .envs(log_vars())
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

    // Download binary into temp dir
    let bin_name = format!("near-sandbox-{}", normalize_name(version));
    let dl_cache = Cache::at(&download_path(version));
    let bin_path = bin_url(version).ok_or_else(|| {
        SandboxError::UnsupportedPlatformError(
            "only linux-x86 and darwin-arm are supported".to_owned(),
        )
    })?;
    let dl = dl_cache
        .download(true, &bin_name, &["near-sandbox"], &bin_path)
        .map_err(|e| SandboxError::DownloadError(e.to_string()))?
        .ok_or_else(|| SandboxError::InstallError("Could not install near-sandbox".to_owned()))?;

    let path = dl
        .binary("near-sandbox")
        .map_err(|e| SandboxError::InstallError(e.to_string()))?;

    // Move near-sandbox binary to correct location from temp folder.
    let dest = download_path(version).join("near-sandbox");
    std::fs::rename(path, &dest).map_err(SandboxError::FileError)?;

    Ok(dest)
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
        unsafe {
            std::env::set_var("NEAR_SANDBOX_BIN_PATH", bin_path.as_os_str());
        }
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
