//! # Singleton Sandbox Pattern
//! This example demonstrates how to share a single sandbox instance across multiple tests
//! for significantly faster test execution.
//!
//! PLEASE MIND THAT THIS MIGHT NOT WORK WITH `nextest`
//! as they are using [process-per-test](https://nexte.st/docs/design/why-process-per-test/)
//!
//! ## The Problem
//! Spinning up a new sandbox for each test is slow (~2-5 seconds per test). For test suites
//! with hundreds of tests, this adds up to 30+ minutes of just sandbox startup time.
//!
//! ## The Solution
//! Share one sandbox across all tests, isolating each test with unique subaccounts.
//!
//! ## Key Components
//! 1. `OnceCell<Sandbox>` / `LazyLock`/  `Arc` - Hold the single sandbox instance
//!     - Also, check out how Defuse team does things with [Sandbox](https://github.com/near/intents/blob/main/sandbox/src/lib.rs)
//! 2. `AtomicUsize` counter - Generates unique subaccount names for test isolation
//! 3. Subaccount pattern - Each test gets `{counter}.sandbox` for isolation
//!
//! ## Pros
//! - Faster execution of tests - Sandbox is started once, and tests share it
//! - Easy to customize - you can wrap `Sandbox` into your own `SharedEnv` with helper methods
//! - Parallel-friendly - isolating tests with subaccount prevents test interference
//!
//! ## Cons
//! - Higher memory usage - state accumulates as tests deploy contracts, produce blocks, and generate reciepts (no cleanup between tests)
//! - Shared state - tests are not fully isolated; one test's `fast_forward()` affects othres
//!
//! ## Run This Example
//! ```bash
//! cargo run --example singleton_sandbox
//! cargo test --example singleton_sandbox
//! ```

use std::sync::{atomic::AtomicUsize, Arc};

use near_api::NetworkConfig;
use tokio::sync::OnceCell;

/// Global singleton sandbox instance
///
/// Using `OnceCell` ensures that sandbox is initialized only once, even when tests run in
/// parallel. The `#[dtor]` cleanup in `near-sandbox` ensures the sandbox process is killed when
/// the test binary exits
static SHARED_SANDBOX: OnceCell<SharedEnv> = OnceCell::const_new();

/// Counter for generating unique subaccount names
/// Each test gets a new subaccount like `0.test.sandbox`, `1.test.sandbox`, etc.
/// With `generate` feature flag, for `near-sandbox`, you can also use `near_sandbox::generate_account_id()`
static ACCOUNT_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Shared environment wrapping the sandbox with helper methods
pub struct SharedEnv {
    sandbox: Arc<near_sandbox::Sandbox>,
    network: near_api::NetworkConfig,
    /// Root account that creates subaccount for tests
    root_acc: near_account_id::AccountId,
    /// Root signer for account that creates subaccounts for tests
    root_signer: Arc<near_api::Signer>,
}

impl SharedEnv {
    /// Initialize the shared environment with a new sandbox.
    async fn init() -> Self {
        let sandbox = near_sandbox::Sandbox::start_sandbox()
            .await
            .expect("Failed to start sandbox");

        let network =
            near_api::NetworkConfig::from_rpc_url("sandbox", sandbox.rpc_addr.parse().unwrap());

        // Use default "sandbox" account as root for creating subaccounts
        // You can also define your own TLA and use `near_api::signer::generate_secret_key()` for
        // secret key generation
        let root_acc: near_account_id::AccountId =
            near_sandbox::config::DEFAULT_GENESIS_ACCOUNT.to_owned();
        let root_signer = near_api::Signer::from_secret_key(
            near_sandbox::config::DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY
                .parse()
                .expect("Valid genesis secret key"),
        )
        .expect("Unable to create valid signer from secret key");

        Self {
            sandbox: sandbox.into(),
            network,
            root_acc,
            root_signer,
        }
    }

    pub fn network(&self) -> &NetworkConfig {
        &self.network
    }

    pub fn sandbox(&self) -> Arc<near_sandbox::Sandbox> {
        self.sandbox.clone()
    }

    /// Generate a new unique subaccount for test isolation.
    ///
    /// Each call returns a new account like `0.test.near`, `1.test.near`, etc.
    /// This allows tests to run in parallel without interfering with each other.
    pub async fn generate_account(
        &self,
        initial_balance: near_token::NearToken,
    ) -> (near_account_id::AccountId, Arc<near_api::Signer>) {
        let counter = ACCOUNT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let account_id: near_account_id::AccountId =
            format!("{}.{}", counter, self.root_acc).parse().unwrap();

        let secret_key =
            near_api::signer::generate_secret_key().expect("Failed to generate secret key");

        let account_signer =
            near_api::Signer::from_secret_key(secret_key.clone()).expect("Failed to create signer");

        near_api::Account::create_account(account_id.clone())
            .fund_myself(self.root_acc.clone(), initial_balance)
            .with_public_key(secret_key.public_key())
            .with_signer(self.root_signer.clone())
            .send_to(self.network())
            .await
            .expect("Failed to create subaccount")
            .assert_success();

        (account_id, account_signer)
    }
}

/// Get or initialize the shared sandbox environemnt.
///
/// This is the main entry point for tests. Call this at the start of each test to get access to
/// the shared sandbox.
pub async fn get_shared_env() -> &'static SharedEnv {
    SHARED_SANDBOX
        .get_or_init(|| async { SharedEnv::init().await })
        .await
}

#[tokio::main]
async fn main() {
    println!("=== Singleton Sandbox Pattern Demo ===");
    println!(
        "Run `cargo test --examples singleton_sandbox` for seeing this example work as intented!"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_static_account_creation() {
        let env = get_shared_env().await;
        let (account_id, _signer) = env
            .generate_account(near_token::NearToken::from_near(5))
            .await;
        println!(
            "Created account {account_id} with 5 NearTokens in shared sandbox at url: {}",
            env.sandbox.rpc_addr
        );

        let account = near_api::Account(account_id)
            .view()
            .fetch_from(env.network())
            .await
            .unwrap();

        assert!(account.data.amount >= near_token::NearToken::from_near(4));
    }

    #[tokio::test]
    async fn test_static_account_creation_2() {
        let env = get_shared_env().await;
        let (account_id, _signer) = env
            .generate_account(near_token::NearToken::from_near(5))
            .await;
        println!(
            "Created account {account_id} with 5 NearTokens in shared sandbox at url: {}",
            env.sandbox.rpc_addr
        );

        let account = near_api::Account(account_id)
            .view()
            .fetch_from(env.network())
            .await
            .unwrap();

        assert!(account.data.amount >= near_token::NearToken::from_near(4));
    }

    #[tokio::test]
    async fn test_additional_sandbox() {
        let new_env = SharedEnv::init().await;
        let (account_id, _signer) = new_env
            .generate_account(near_token::NearToken::from_near(2))
            .await;
        println!(
            "Created account {account_id} with 2 NearTokens inside in-test sandbox at url: {}",
            new_env.sandbox.rpc_addr
        );

        let account = near_api::Account(account_id)
            .view()
            .fetch_from(new_env.network())
            .await
            .unwrap();

        assert!(account.data.amount >= near_token::NearToken::from_near(1));
    }
}
