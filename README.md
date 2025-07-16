<div align="center">

  <h1>NEAR Sandbox (Rust Edition)</h1>

  <p>
    <strong>Rust library for running a local NEAR node for development and testing.</strong>
  </p>

  <p>
    <a href="https://crates.io/crates/near-sandbox-utils"><img src="https://img.shields.io/crates/v/near-sandbox-utils.svg?style=flat-square" alt="Crates.io version" /></a>
    <a href="https://crates.io/crates/near-sandbox-utils"><img src="https://img.shields.io/crates/d/near-sandbox-utils.svg?style=flat-square" alt="Download" /></a>
    <a href="https://docs.rs/near-sandbox-utils"><img src="https://docs.rs/near-sandbox-utils/badge.svg" alt="Reference Documentation" /></a>
  </p>
</div>

## Release Notes

Release notes and unreleased changes can be found in the [CHANGELOG](./CHANGELOG.md).

## Requirements

- Rust v1.85.0 or newer.
- MacOS (ARM64) or Linux (x86) for sandbox execution.

## What is NEAR Sandbox?

NEAR Sandbox is a [custom build](https://github.com/near/nearcore/blob/9f5e20b29f1a15a00fc50d6051b3b44bb6db60b6/Makefile#L67-L69) of the NEAR blockchain, optimized for local development and testing.  
If you're familiar with [Ganache for Ethereum](https://www.trufflesuite.com/ganache), this serves a similar purpose for NEAR.

This library provides a Rust API to easily start and configure your local NEAR Sandbox instance. The sandbox binary is automatically downloaded and managed for you.

## Installation

Add `near-sandbox-utils` to your `[dev-dependencies]`:

```toml
[dev-dependencies]
near-sandbox-utils = "..."
```

## Simple Testing Example

```rust
use near_sandbox::Sandbox;

#[tokio::test]
async fn test_basic_sandbox() -> Result<(), Box<dyn std::error::Error>> {
    // Start a sandbox instance
    let sandbox = Sandbox::start_sandbox().await?;

    // Your test code here
    // The sandbox will be automatically cleaned up when it's dropped

    Ok(())
}
```

## Examples

More examples can be found in the [`examples/`](./examples/) directory.

## Features

- **Easy sandbox startup:** Start a local NEAR node with default or custom configuration.
- **Version selection:** Choose a specific NEAR Sandbox version.
- **Custom configuration:** Configure the `config.json` and `genesis.json` for your `near-sandbox` instance.
- **Automatic binary management:** The required sandbox binary is downloaded and managed automatically.
- **RPC access:** Interact with your sandbox node via its RPC endpoint.
- **Environment variable configuration:** Control binary source, logging, and timeouts via environment variables.

### Starting a Sandbox

```rust
use near_sandbox::{Sandbox, SandboxConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start with default configuration
    let sandbox = Sandbox::start_sandbox().await?;

    // Or with a specific version
    let sandbox = Sandbox::start_sandbox_with_version("2.6.3").await?;

    // Or with custom configuration
    let config = SandboxConfig::default();
    let sandbox = Sandbox::start_sandbox_with_config(config).await?;

    // Or with both custom config and version
    let sandbox = Sandbox::start_sandbox_with_config_and_version(config, "2.6.3").await?;

    // The sandbox is automatically cleaned up when dropped
    Ok(())
}
```

### Accessing RPC

```rust
use near_sandbox::Sandbox;

#[tokio::test]
async fn test_rpc_access() -> Result<(), Box<dyn std::error::Error>> {
    let sandbox = Sandbox::start_sandbox().await?;

    // Access the RPC endpoint
    let rpc_url = &sandbox.rpc_addr;
    println!("Sandbox RPC available at: {}", rpc_url);

    // Make RPC calls to your sandbox
    // ...

    Ok(())
}
```

### Configuring RPC

```rust
use near_sandbox::{GenesisAccount, Sandbox, SandboxConfig};
use serde_json::json;

#[tokio::test]
async fn test_custom_rpc_config() -> Result<(), Box<dyn std::error::Error>> {
    let alice_genesis = GenesisAccount {
        account_id: "alice.near".to_string(),
        // You can also use `near_api::signer::get_secret_key()` and `signer.public_key()`
        public_key: "ed25519:AzBN9XwQDRuLvGvor2JnMitkRxBxn2TLY4yEM3othKUF".to_string(),
        private_key: "ed25519:5byt6y8h1uuHwkr2ozfN5gt8xGiHujpcT5KyNhZpG62BrnU51sMQk5eTVNwWp7RRiMgKHp7W1jrByxLCr2apXNGB".to_string(),
        // You can also use `NearToken::from_near(1000).as_yoctonear()`
        balance: 10_00u128 * 10u128.pow(24),
    };

    let config = SandboxConfig {
        additional_genesis: Some(json!({ "epoch_length": 100 })),
        additional_accounts: vec![alice_genesis.clone()],
        additional_config: Some(json!({ "network": { "trusted_stun_servers": [] } })),
        max_payload_size: None,
        max_open_files: None,
    };

    let sandbox = Sandbox::start_sandbox_with_config(config).await?;

    // Test of your custom sandbox instance
    // ...

    Ok(())
}
```

### Automatic `near-sandbox` Binary Management

1. When you start a sandbox, the appropriate binary for your platform is automatically downloaded if it is not already present.
2. The sandbox process runs in the background.
3. When the `Sandbox` struct is dropped, the process is automatically killed.

## Environment Variables

Customize sandbox behavior with these environment variables:

- `SANDBOX_ARTIFACT_URL`: Override the download link for `neard`. Useful if you have trouble downloading from the default IPFS gateway.
- `NEAR_RPC_TIMEOUT_SECS`: Set the timeout (in seconds) for waiting for the sandbox to start (default: 10).
- `NEAR_SANDBOX_BIN_PATH`: Use your own pre-built `neard-sandbox` binary instead of the default. Be careful not to use NodeJs package!
- `NEAR_ENABLE_SANDBOX_LOG`: Set to `1` to enable sandbox logging of `near-sandbox` (helpful for debugging).
- `NEAR_SANDBOX_LOG`: Specify custom log levels for the sandbox (forwarded to the `RUST_LOG` environment variable).
- `NEAR_SANDBOX_LOG_STYLE`: Specify custom log style for the sandbox (forwarded to the `RUST_LOG_STYLE` environment variable).

## API Reference

API documentation is available at [docs.rs/near-sandbox-utils](https://docs.rs/near-sandbox-utils).
