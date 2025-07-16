//! Network specific configurations used to modify behavior inside a chain.
//!
//! This is so far only useable with sandbox networks since it would require
//! direct access to a node to change the config. Each network like mainnet
//! and testnet already have pre-configured settings; meanwhile sandbox can
//! have additional settings on top of them to facilitate custom behavior
//! such as sending large requests to the sandbox network.
//
// NOTE: nearcore has many, many configs which can easily change in the future
// so this config.rs file just purely modifies the data and does not try to
// replicate all the structs from nearcore side; which can be a huge maintenance
// churn if we were to.

use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;
use std::str::FromStr;

use near_account_id::{AccountId, AccountIdRef};
use near_token::NearToken;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_GENESIS_ACCOUNT: &AccountIdRef = AccountIdRef::new_or_panic("sandbox");
pub const DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY: &str = "ed25519:3tgdk2wPraJzT4nsTuf86UX41xgPNk3MHnq8epARMdBNs29AFEztAuaQ7iHddDfXG9F2RzV1XNQYgJyAyoW51UBB";
pub const DEFAULT_GENESIS_ACCOUNT_PUBLIC_KEY: &str =
    "ed25519:5BGSaf6YjVm7565VzWQHNxoyEjwr3jUpRJSGjREvU9dB";
pub const DEFAULT_GENESIS_ACCOUNT_BALANCE: NearToken = NearToken::from_near(10_000);

#[derive(thiserror::Error, Debug)]
pub enum SandboxConfigError {
    #[error("Error while performing r/w on config file: {0}")]
    FileError(std::io::Error),

    #[error("Error while parsing config file: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("Invalid environment variables: {0}")]
    EnvParseError(String),
}

#[cfg(feature = "generate")]
pub(crate) fn random_account_id() -> AccountId {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let random_num = rng.gen_range(0..999999999999999999);
    let account_id = format!(
        "dev-acc-{}-{}.sandbox",
        chrono::Utc::now().format("%H%M%S"),
        random_num
    );

    account_id.parse().expect("should be valid account id")
}

/// Generates pseudo-random base58 encoded ed25519 secret and public keys
///
/// WARNING: Prefer using `SecretKey` and `PublicKey` from [`near_crypto`](https://crates.io/crates/near-crypto) or [`near_sandbox_utils::GenesisAccount::generate_random()`](near_sandbox_utils::GenesisAccount::generate_random())
///
/// ## Generating random key pair for genesis account:
/// ```rust,no_run
/// # fn example() {
/// let (private_key, public_key) = near_sandbox_utils::random_key_pair();
/// let custom_genesis = near_sandbox_utils::GenesisAccount {
///     account_id: "alice",
///     private_key,
///     public_key,
///     ..Default::default()
/// }
/// # }
/// ```
#[cfg(feature = "generate")]
pub(crate) fn random_key_pair() -> (String, String) {
    let mut rng = rand::rngs::OsRng;
    let signing_key: [u8; ed25519_dalek::KEYPAIR_LENGTH] =
        ed25519_dalek::SigningKey::generate(&mut rng).to_keypair_bytes();

    let secret_key = format!(
        "ed25519:{}",
        bs58::encode(&signing_key.to_vec()).into_string()
    );
    let public_key = format!(
        "ed25519:{}",
        bs58::encode(&signing_key[ed25519_dalek::SECRET_KEY_LENGTH..].to_vec()).into_string()
    );

    (secret_key, public_key)
}

/// Genesis account configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisAccount {
    pub account_id: AccountId,
    pub public_key: String,
    pub private_key: String,
    pub balance: near_token::NearToken,
}

impl GenesisAccount {
    pub fn default_with_name(name: AccountId) -> Self {
        Self {
            account_id: name,
            public_key: DEFAULT_GENESIS_ACCOUNT_PUBLIC_KEY.to_string(),
            private_key: DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY.to_string(),
            balance: DEFAULT_GENESIS_ACCOUNT_BALANCE,
        }
    }
}

#[cfg(feature = "generate")]
impl GenesisAccount {
    /// Generates pseudo-random genesis account
    ///
    /// WARNING: Prefer using `GenesisAccount::default()` or defining `GenesisAccount` from a
    /// scratch
    pub fn generate_random() -> Self {
        let (private_key, public_key) = random_key_pair();

        Self {
            account_id: random_account_id(),
            public_key,
            private_key,
            balance: DEFAULT_GENESIS_ACCOUNT_BALANCE,
        }
    }

    pub fn generate_with_name(name: AccountId) -> Self {
        let (private_key, public_key) = random_key_pair();

        Self {
            account_id: name,
            public_key,
            private_key,
            balance: DEFAULT_GENESIS_ACCOUNT_BALANCE,
        }
    }

    pub fn generate_with_name_and_balance(name: AccountId, balance: NearToken) -> Self {
        let (private_key, public_key) = random_key_pair();

        Self {
            account_id: name,
            public_key,
            private_key,
            balance,
        }
    }

    pub fn generate_with_balance(balance: NearToken) -> Self {
        let (private_key, public_key) = random_key_pair();

        Self {
            account_id: random_account_id(),
            public_key,
            private_key,
            balance,
        }
    }
}

impl Default for GenesisAccount {
    fn default() -> Self {
        GenesisAccount {
            account_id: DEFAULT_GENESIS_ACCOUNT.into(),
            public_key: DEFAULT_GENESIS_ACCOUNT_PUBLIC_KEY.to_string(),
            private_key: DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY.to_string(),
            balance: DEFAULT_GENESIS_ACCOUNT_BALANCE,
        }
    }
}

/// Configuration for the sandbox
#[derive(Debug, Clone, Default)]
pub struct SandboxConfig {
    /// Maximum payload size for JSON RPC requests in bytes
    pub max_payload_size: Option<usize>,
    /// Maximum number of open files
    pub max_open_files: Option<usize>,
    /// Additional JSON configuration to merge with the default config
    pub additional_config: Option<Value>,
    /// Additional accounts to add to the genesis
    pub additional_accounts: Vec<GenesisAccount>,
    /// Additional JSON configuration to merge with the genesis
    pub additional_genesis: Option<Value>,
    /// Port that RPC will be bound to. Will be picked randomly if not set.
    pub rpc_port: Option<u16>,
    /// Port that Network will be bound to. Will be picked randomly if not set.
    pub net_port: Option<u16>,
}

/// Overwrite the $home_dir/config.json file over a set of entries. `value` will be used per (key, value) pair
/// where value can also be another dict. This recursively sets all entry in `value` dict to the config
/// dict, and saves back into `home_dir` at the end of the day.
fn overwrite(home_dir: impl AsRef<Path>, value: Value) -> Result<(), SandboxConfigError> {
    let home_dir = home_dir.as_ref();
    let config_file =
        File::open(home_dir.join("config.json")).map_err(SandboxConfigError::FileError)?;
    let config = BufReader::new(config_file);
    let mut config: Value = serde_json::from_reader(config)?;

    json_patch::merge(&mut config, &value);
    let config_file =
        File::create(home_dir.join("config.json")).map_err(SandboxConfigError::FileError)?;
    serde_json::to_writer(config_file, &config)?;

    Ok(())
}

/// Parse an environment variable or return a default value.
fn parse_env<T>(env_var: &str) -> Result<Option<T>, SandboxConfigError>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match std::env::var(env_var) {
        Ok(val) => {
            let val = val
                .parse::<T>()
                .map_err(|e| SandboxConfigError::EnvParseError(e.to_string()))?;
            Ok(Some(val))
        }
        Err(_err) => Ok(None),
    }
}

/// Set extra configs for the sandbox with custom configuration.
///
/// # Arguments
/// * `home_dir` - path for home directory of neard
/// * `config` - config, with which neard configuration will be overwritten
pub(crate) fn set_sandbox_configs_with_config(
    home_dir: impl AsRef<Path>,
    config: &SandboxConfig,
) -> Result<(), SandboxConfigError> {
    let max_payload_size = config
        .max_payload_size
        .or_else(|| parse_env("NEAR_SANDBOX_MAX_PAYLOAD_SIZE").ok().flatten())
        .unwrap_or(1024 * 1024 * 1024); // Default to 1GB

    let max_open_files = config
        .max_open_files
        .or_else(|| parse_env("NEAR_SANDBOX_MAX_FILES").ok().flatten())
        .unwrap_or(3000); // Default to 3,000

    let mut json_config = serde_json::json!({
        "rpc": {
            "limits_config": {
                "json_payload_max_size": max_payload_size,
            },
        },
        "store": {
            "max_open_files": max_open_files,
        }
    });

    // Merge any additional config provided by the user
    if let Some(additional_config) = &config.additional_config {
        json_patch::merge(&mut json_config, additional_config);
    }

    overwrite(home_dir, json_config)
}

/// Overwrite the $home_dir/genesis.json file over a set of entries. `value` will be used per (key, value) pair
/// where value can also be another dict. This recursively sets all entry in `value` dict to the config
/// dict, and saves back into `home_dir` at the end of the day.
fn overwrite_genesis(
    home_dir: impl AsRef<Path>,
    config: &SandboxConfig,
) -> Result<(), SandboxConfigError> {
    let home_dir = home_dir.as_ref();
    let config_file =
        File::open(home_dir.join("genesis.json")).map_err(SandboxConfigError::FileError)?;
    let config_reader = BufReader::new(config_file);
    let mut genesis: Value = serde_json::from_reader(config_reader)?;
    let genesis_obj = genesis.as_object_mut().expect("expected to be object");
    let mut total_supply = u128::from_str(
        genesis_obj
            .get_mut("total_supply")
            .expect("expected exist total_supply")
            .as_str()
            .unwrap_or_default(),
    )
    .unwrap_or_default();

    let mut accounts_to_add = vec![GenesisAccount::default()];

    accounts_to_add.extend(config.additional_accounts.clone());

    for account in &accounts_to_add {
        total_supply += account.balance.as_yoctonear();
    }

    genesis_obj.insert(
        "total_supply".to_string(),
        Value::String(total_supply.to_string()),
    );

    let records = genesis_obj
        .get_mut("records")
        .expect("expect exist records");
    let records_array = records.as_array_mut().expect("expected to be array");

    for account in &accounts_to_add {
        records_array.push(serde_json::json!(
            {
                "Account": {
                    "account_id": account.account_id,
                    "account": {
                    "amount": account.balance,
                    "locked": "0",
                    "code_hash": "11111111111111111111111111111111",
                    "storage_usage": 182
                    }
                }
            }
        ));

        records_array.push(serde_json::json!(
            {
                "AccessKey": {
                    "account_id": account.account_id,
                    "public_key": account.public_key,
                    "access_key": {
                    "nonce": 0,
                    "permission": "FullAccess"
                    }
                }
            }
        ));
    }

    if let Some(additional_genesis) = &config.additional_genesis {
        json_patch::merge(&mut genesis, additional_genesis);
    }

    let config_file =
        File::create(home_dir.join("genesis.json")).map_err(SandboxConfigError::FileError)?;
    serde_json::to_writer(config_file, &genesis)?;
    Ok(())
}

/// Save account keys to individual JSON files
fn save_account_keys(
    home_dir: impl AsRef<Path>,
    accounts: &[GenesisAccount],
) -> Result<(), SandboxConfigError> {
    let home_dir = home_dir.as_ref();

    for account in accounts {
        let key_json = serde_json::json!({
            "account_id": account.account_id,
            "public_key": account.public_key,
            "private_key": account.private_key
        });

        let file_name = format!("{}.json", account.account_id);
        let mut key_file =
            File::create(home_dir.join(&file_name)).map_err(SandboxConfigError::FileError)?;
        let key_content = serde_json::to_string(&key_json)?;
        key_file
            .write_all(key_content.as_bytes())
            .map_err(SandboxConfigError::FileError)?;
        key_file.flush().map_err(SandboxConfigError::FileError)?;
    }

    Ok(())
}

pub fn set_sandbox_genesis(home_dir: impl AsRef<Path>) -> Result<(), SandboxConfigError> {
    let config = SandboxConfig::default();
    set_sandbox_genesis_with_config(&home_dir, &config)
}

pub fn set_sandbox_genesis_with_config(
    home_dir: impl AsRef<Path>,
    config: &SandboxConfig,
) -> Result<(), SandboxConfigError> {
    overwrite_genesis(&home_dir, config)?;

    let mut all_accounts = vec![GenesisAccount::default()];
    all_accounts.extend(config.additional_accounts.clone());

    save_account_keys(&home_dir, &all_accounts)?;

    Ok(())
}
