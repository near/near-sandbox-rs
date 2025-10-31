use std::net::SocketAddrV4;
use std::time::Duration;
use std::{fs::File, net::Ipv4Addr};

use fs4::FileExt;
use near_account_id::AccountId;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::process::Child;
use tracing::info;

use crate::config::{self, SandboxConfig};
use crate::error_kind::{SandboxError, SandboxRpcError, TcpError};
use crate::runner::{init_with_version, run_with_options_with_version};
use crate::sandbox::patch::PatchState;

pub mod patch;

// Must be an IP address as `neard` expects socket address for network address.
const DEFAULT_RPC_HOST: &str = "127.0.0.1";

fn rpc_socket(port: u16) -> String {
    format!("{DEFAULT_RPC_HOST}:{port}")
}

/// Request an unused port from the OS.
async fn pick_unused_port() -> Result<u16, SandboxError> {
    // Port 0 means the OS gives us an unused port
    // Important to use localhost as using 0.0.0.0 leads to users getting brief firewall popups to
    // allow inbound connections on MacOS.
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| TcpError::BindError(addr.port(), e))?;
    let port = listener
        .local_addr()
        .map_err(TcpError::LocalAddrError)?
        .port();
    Ok(port)
}

/// Acquire an unused port and lock it for the duration until the sandbox server has
/// been started.
async fn acquire_unused_port() -> Result<(u16, File), SandboxError> {
    loop {
        let port = pick_unused_port().await?;
        let lockpath = std::env::temp_dir().join(format!("near-sandbox-port{port}.lock"));
        let lockfile = File::create(lockpath).map_err(TcpError::LockingError)?;
        if lockfile.try_lock_exclusive().is_ok() {
            break Ok((port, lockfile));
        }
    }
}

/// Try to acquire a specific port and lock it.
/// Returns the port and lock file if successful.
async fn try_acquire_specific_port(port: u16) -> Result<(u16, File), SandboxError> {
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| TcpError::BindError(addr.port(), e))?;
    let port = listener
        .local_addr()
        .map_err(TcpError::LocalAddrError)?
        .port();

    let lockpath = std::env::temp_dir().join(format!("near-sandbox-port{port}.lock"));
    let lockfile = File::create(&lockpath).map_err(TcpError::LockingError)?;
    lockfile
        .try_lock_exclusive()
        .map_err(TcpError::LockingError)?;

    Ok((port, lockfile))
}

async fn acquire_or_lock_port(configured_port: Option<u16>) -> Result<(u16, File), SandboxError> {
    match configured_port {
        Some(port) => try_acquire_specific_port(port).await,
        None => acquire_unused_port().await,
    }
}

/// An sandbox instance that can be used to launch local near network to test against.
///
/// All the [examples](https://github.com/near/near-api-rs/tree/main/examples) are using Sandbox implementation.
///
/// This is work-in-progress and not all the features are supported yet.
pub struct Sandbox {
    /// Home directory for sandbox instance. Will be cleaned up once Sandbox is dropped
    pub home_dir: TempDir,
    /// URL that can be used to access RPC. In format of `http://127.0.0.1:{port}`
    pub rpc_addr: String,
    /// File lock preventing other processes from using the same RPC port until this sandbox is started
    pub rpc_port_lock: File,
    /// File lock preventing other processes from using the same network port until this sandbox is started
    pub net_port_lock: File,
    process: Child,
}

impl Sandbox {
    /// Start a new sandbox with the default near-sandbox-utils version.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use near_sandbox::*;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Launch with default config and version
    /// let sandbox = Sandbox::start_sandbox().await?;
    /// println!("Sandbox RPC endpoint: {}", sandbox.rpc_addr);
    /// // ... do your testing ...
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start_sandbox() -> Result<Self, SandboxError> {
        Self::start_sandbox_with_config_and_version(
            SandboxConfig::default(),
            crate::DEFAULT_NEAR_SANDBOX_VERSION,
        )
        .await
    }

    /// Start a new sandbox with the given near-sandbox-utils version.
    ///
    /// # Arguments
    /// * `version` - the version of the near-sandbox-utils to use.
    ///
    /// # Exmaple:
    ///
    /// ```rust,no_run
    /// use near_sandbox::*;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Launch with default config
    /// let sandbox = Sandbox::start_sandbox_with_version("2.6.3").await?;
    /// println!("Sandbox RPC endpoint: {}", sandbox.rpc_addr);
    /// // ... do your testing ...
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start_sandbox_with_version(version: &str) -> Result<Self, SandboxError> {
        Self::start_sandbox_with_config_and_version(SandboxConfig::default(), version).await
    }

    /// Start a new sandbox with the custom configuration and default version.
    ///
    /// # Arguments
    /// * `config` - custom configuration for the sandbox
    ///
    /// # Example
    ///
    /// ``` rust,no_run
    /// use near_sandbox::*;
    /// use near_token::NearToken;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut cfg = SandboxConfig::default();
    /// cfg.rpc_port = Some(3030);
    /// cfg.additional_genesis = Some(json!({ "epoch_length": 200 }));
    /// cfg.additional_accounts = vec![
    ///     GenesisAccount {
    ///         account_id: "bob.near".parse().unwrap(),
    ///         public_key: "ed25519:...".to_string(),
    ///         private_key: "ed25519:...".to_string(),
    ///         balance: NearToken::from_near(10_000),
    ///     },
    /// ];
    ///
    /// let sandbox = Sandbox::start_sandbox_with_config(cfg).await?;
    /// println!("Custom sandbox running at {}", sandbox.rpc_addr);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start_sandbox_with_config(config: SandboxConfig) -> Result<Self, SandboxError> {
        Self::start_sandbox_with_config_and_version(config, crate::DEFAULT_NEAR_SANDBOX_VERSION)
            .await
    }

    /// Start a new sandbox with a custom configuration and specific near-sandbox-utils version.
    ///
    /// # Arguments
    /// * `config` - custom configuration for the sandbox
    /// * `version` - the version of the near-sandbox-utils to use
    ///
    /// # Example
    ///
    /// ``` rust,no_run
    /// use near_sandbox::*;
    /// use near_token::NearToken;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut cfg = SandboxConfig::default();
    /// cfg.rpc_port = Some(3030);
    /// cfg.additional_genesis = Some(json!({ "epoch_length": 200 }));
    /// cfg.additional_accounts = vec![
    ///     GenesisAccount {
    ///         account_id: "bob.near".parse().unwrap(),
    ///         public_key: "ed25519:...".to_string(),
    ///         private_key: "ed25519:...".to_string(),
    ///         balance: NearToken::from_near(10_000),
    ///     },
    /// ];
    ///
    /// let sandbox = Sandbox::start_sandbox_with_config_and_version(cfg, "2.6.3").await?;
    /// println!("Custom sandbox running at {}", sandbox.rpc_addr);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start_sandbox_with_config_and_version(
        config: SandboxConfig,
        version: &str,
    ) -> Result<Self, SandboxError> {
        suppress_sandbox_logs_if_required();
        let home_dir = Self::init_home_dir_with_version(version).await?;

        let (rpc_port, rpc_port_lock) = acquire_or_lock_port(config.rpc_port).await?;
        let (net_port, net_port_lock) = acquire_or_lock_port(config.net_port).await?;

        let rpc_addr = rpc_socket(rpc_port);
        let net_addr = rpc_socket(net_port);

        config::set_sandbox_configs_with_config(&home_dir, &config)?;
        config::set_sandbox_genesis_with_config(&home_dir, &config)?;

        let options = &[
            "--home",
            home_dir.path().to_str().expect("home_dir is valid utf8"),
            "run",
            "--rpc-addr",
            &rpc_addr,
            "--network-addr",
            &net_addr,
        ];

        let child = run_with_options_with_version(options, version)?;

        info!(target: "sandbox", "Started up sandbox at localhost:{} with pid={:?}", rpc_port, child.id());

        let rpc_addr = format!("http://{rpc_addr}");

        Self::wait_until_ready(&rpc_addr).await?;

        Ok(Self {
            home_dir,
            rpc_addr,
            rpc_port_lock,
            net_port_lock,
            process: child,
        })
    }

    async fn init_home_dir_with_version(version: &str) -> Result<TempDir, SandboxError> {
        let home_dir = tempfile::tempdir().map_err(SandboxError::FileError)?;

        let output = init_with_version(&home_dir, version)?
            .wait_with_output()
            .await
            .map_err(SandboxError::RuntimeError)?;
        info!(target: "sandbox", "sandbox init: {:?}", output);

        Ok(home_dir)
    }

    async fn wait_until_ready(rpc: &str) -> Result<(), SandboxError> {
        let timeout_secs = match std::env::var("NEAR_RPC_TIMEOUT_SECS") {
            Ok(secs) => secs
                .parse::<u64>()
                .expect("Failed to parse NEAR_RPC_TIMEOUT_SECS"),
            Err(_) => 10,
        };

        let mut interval = tokio::time::interval(Duration::from_millis(500));
        for _ in 0..timeout_secs * 2 {
            interval.tick().await;
            let response = reqwest::get(format!("{rpc}/status")).await;
            if response.is_ok() {
                return Ok(());
            }
        }
        Err(SandboxError::TimeoutError)
    }

    pub async fn fast_forward(&self, blocks: u64) -> Result<(), SandboxRpcError> {
        let client = reqwest::Client::new();
        let result = client
            .post(self.rpc_addr.clone())
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "0",
                "method": "sandbox_fast_forward",
                "params": {
                    "delta_height": blocks,
                },
            }))
            .send()
            .await?;

        let body = result.json::<serde_json::Value>().await?;

        if body["error"].is_object() {
            return Err(SandboxRpcError::SandboxRpcError(body["error"].to_string()));
        }

        Ok(())
    }

    pub const fn patch_state(&self, account_id: AccountId) -> PatchState<'_> {
        PatchState::new(account_id, self)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        info!(
            target: "sandbox",
            "Cleaning up sandbox: pid={:?}",
            self.process.id()
        );

        self.process.start_kill().expect("failed to kill sandbox");
        let _ = self.process.try_wait();
    }
}

/// Turn off neard-sandbox logs by default. Users can turn them back on with
/// NEAR_ENABLE_SANDBOX_LOG=1 and specify further parameters with the custom
/// NEAR_SANDBOX_LOG for higher levels of specificity. NEAR_SANDBOX_LOG args
/// will be forward into RUST_LOG environment variable as to not conflict
/// with similar named log targets.
fn suppress_sandbox_logs_if_required() {
    if let Ok(val) = std::env::var("NEAR_ENABLE_SANDBOX_LOG") {
        if val != "0" {
            return;
        }
    }

    // non-exhaustive list of targets to suppress, since choosing a default LogLevel
    // does nothing in this case, since nearcore seems to be overriding it somehow:

    // SAFETY: well, overall, it might be unsafe, but I think it's fine here.
    // As the worst case scenario is that the logs are not suppressed, which is not a big deal.
    unsafe {
        std::env::set_var("NEAR_SANDBOX_LOG", "near=error,stats=error,network=error");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fast_forward() {
        let sandbox = Sandbox::start_sandbox().await.unwrap();
        let network =
            near_api::NetworkConfig::from_rpc_url("sandbox", sandbox.rpc_addr.parse().unwrap());

        let height = near_api::Chain::block_number()
            .fetch_from(&network)
            .await
            .unwrap();

        sandbox.fast_forward(1000).await.unwrap();

        let new_height = near_api::Chain::block_number()
            .fetch_from(&network)
            .await
            .unwrap();

        assert!(new_height >= height + 1000, "expected new height({new_height}) to be at least 1000 blocks higher than the original height({height})");
    }
}
