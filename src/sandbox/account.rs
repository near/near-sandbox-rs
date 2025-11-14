use near_account_id::AccountId;
use near_token::NearToken;
use reqwest::IntoUrl;

use crate::{config::DEFAULT_ACCOUNT_FOR_CLONING, error_kind::SandboxRpcError, FetchData, Sandbox};

#[derive(Clone)]
pub struct AccountCreation<'a> {
    pub account_id: AccountId,
    pub sandbox: &'a Sandbox,

    pub balance: Option<NearToken>,
    pub public_key: Option<String>,
}

impl<'a> AccountCreation<'a> {
    pub const fn new(account_id: AccountId, sandbox: &'a Sandbox) -> Self {
        Self {
            account_id,
            sandbox,
            balance: None,
            public_key: None,
        }
    }

    pub const fn initial_balance(mut self, balance: NearToken) -> Self {
        self.balance = Some(balance);
        self
    }

    pub fn public_key(mut self, public_key: String) -> Self {
        self.public_key = Some(public_key);
        self
    }

    pub async fn send(self) -> Result<(), SandboxRpcError> {
        let mut patch = self
            .sandbox
            .patch_state(self.account_id.clone())
            .fetch_from_account(
                &DEFAULT_ACCOUNT_FOR_CLONING.to_owned(),
                &self.sandbox.rpc_addr,
                FetchData::NONE.account(),
            )
            .await?;

        if let Some(balance) = self.balance {
            patch = patch.initial_balance(balance);
        }
        if let Some(public_key) = self.public_key {
            patch = patch.access_key(
                public_key,
                serde_json::json!({
                    "nonce": 0,
                    "permission": "FullAccess"
                }),
            );
        } else {
            patch = patch.with_default_access_key();
        }
        patch.send().await?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct AccountImport<'a, T: IntoUrl> {
    pub account_id: AccountId,
    pub sandbox: &'a Sandbox,
    pub from_rpc: T,

    pub fetch_data: FetchData,
    pub initial_balance: Option<NearToken>,
    pub public_key: Option<String>,
}

impl<'a, T: IntoUrl> AccountImport<'a, T> {
    pub const fn new(account_id: AccountId, from_rpc: T, sandbox: &'a Sandbox) -> Self {
        Self {
            account_id,
            sandbox,
            from_rpc,
            fetch_data: FetchData::NONE.account().code(),
            initial_balance: None,
            public_key: None,
        }
    }

    pub const fn with_storage(mut self) -> Self {
        self.fetch_data = self.fetch_data.storage();
        self
    }

    pub const fn with_access_keys(mut self) -> Self {
        self.fetch_data = self.fetch_data.access_keys();
        self
    }

    pub const fn initial_balance(mut self, balance: NearToken) -> Self {
        self.initial_balance = Some(balance);
        self
    }

    pub fn public_key(mut self, public_key: String) -> Self {
        self.public_key = Some(public_key);
        self
    }

    pub async fn send(self) -> Result<(), SandboxRpcError> {
        let mut patch = self
            .sandbox
            .patch_state(self.account_id.clone())
            .fetch_from(self.from_rpc, self.fetch_data)
            .await?;

        if let Some(public_key) = self.public_key {
            patch = patch.access_key(
                public_key,
                serde_json::json!({
                    "nonce": 0,
                    "permission": "FullAccess"
                }),
            );
        } else {
            patch = patch.with_default_access_key();
        }

        if let Some(balance) = self.initial_balance {
            patch = patch.initial_balance(balance);
        }

        patch.send().await?;

        Ok(())
    }
}
