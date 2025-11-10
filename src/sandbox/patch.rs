use near_account_id::AccountId;
use near_token::NearToken;
use reqwest::{IntoUrl, Url};
use serde::Serialize;

use crate::{config::DEFAULT_GENESIS_ACCOUNT_PUBLIC_KEY, error_kind::SandboxRpcError, Sandbox};

/// Builder for specifying what data to fetch from an RPC endpoint
#[derive(Clone, Copy, Default)]
pub struct FetchData {
    fetch_account: bool,
    fetch_storage: bool,
    fetch_code: bool,
    fetch_access_keys: bool,
}

impl FetchData {
    pub const NONE: Self = Self::new();

    pub const ALL: Self = Self {
        fetch_account: true,
        fetch_storage: true,
        fetch_code: true,
        fetch_access_keys: true,
    };

    pub const fn new() -> Self {
        Self {
            fetch_account: false,
            fetch_storage: false,
            fetch_code: false,
            fetch_access_keys: false,
        }
    }

    pub const fn account(mut self) -> Self {
        self.fetch_account = true;
        self
    }

    pub const fn storage(mut self) -> Self {
        self.fetch_storage = true;
        self
    }

    pub const fn code(mut self) -> Self {
        self.fetch_code = true;
        self
    }

    pub const fn access_keys(mut self) -> Self {
        self.fetch_access_keys = true;
        self
    }
}

#[derive(Clone)]
pub struct PatchState<'a> {
    pub destination_account: AccountId,
    pub state: Vec<StateRecord>,
    pub sandbox: &'a Sandbox,
    pub initial_balance: Option<NearToken>,
}

impl<'a> PatchState<'a> {
    const EMPTY: Vec<serde_json::Value> = Vec::new();

    pub const fn new(destination_account: AccountId, sandbox: &'a Sandbox) -> Self {
        Self {
            state: vec![],
            destination_account,
            sandbox,
            initial_balance: None,
        }
    }

    pub fn account(mut self, account: impl Serialize) -> Self {
        self.state.push(StateRecord::Account {
            account_id: self.destination_account.clone(),
            account: serde_json::to_value(account).unwrap(),
        });

        self
    }

    /// Fetch data from an RPC endpoint using the FetchData builder
    pub async fn fetch_from(
        self,
        rpc: impl IntoUrl,
        fetch_data: FetchData,
    ) -> Result<Self, SandboxRpcError> {
        let account_id = self.destination_account.clone();
        self.fetch_from_account(&account_id, rpc, fetch_data).await
    }

    pub async fn fetch_from_account(
        mut self,
        account_id: &AccountId,
        rpc: impl IntoUrl,
        fetch_data: FetchData,
    ) -> Result<Self, SandboxRpcError> {
        let rpc = rpc.into_url()?;
        if fetch_data.fetch_account {
            self = self.fetch_account(account_id, rpc.clone()).await?;
        }
        if fetch_data.fetch_code {
            self = self.fetch_code(account_id, rpc.clone()).await?;
        }
        if fetch_data.fetch_storage {
            self = self.fetch_storage(account_id, rpc.clone()).await?;
        }
        if fetch_data.fetch_access_keys {
            self = self.fetch_access_keys(account_id, rpc).await?;
        }
        Ok(self)
    }

    pub fn storage(mut self, state_key_base64: String, state_value_base64: String) -> Self {
        self.state.push(StateRecord::Data {
            account_id: self.destination_account.clone(),
            data_key_base64: state_key_base64,
            value_base64: state_value_base64,
        });

        self
    }

    pub fn storage_entries<I: IntoIterator<Item = (String, String)>>(mut self, entries: I) -> Self {
        let account_id = self.destination_account.clone();
        self.state.extend(
            entries
                .into_iter()
                .map(|(state_key_base64, state_value_base64)| StateRecord::Data {
                    account_id: account_id.clone(),
                    data_key_base64: state_key_base64,
                    value_base64: state_value_base64,
                }),
        );

        self
    }

    pub fn code(mut self, code_base64: String) -> Self {
        self.state.push(StateRecord::Contract {
            account_id: self.destination_account.clone(),
            code_base64,
        });

        self
    }

    pub fn access_key(mut self, public_key_base64: String, access_key: impl Serialize) -> Self {
        self.state.push(StateRecord::AccessKey {
            account_id: self.destination_account.clone(),
            public_key_base64,
            access_key: serde_json::to_value(access_key).unwrap(),
        });

        self
    }

    /// Adds [DEFAULT_GENESIS_ACCOUNT_PUBLIC_KEY] as FullAccess key to the account
    ///
    /// You can get the private key from [crate::config::DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY] constant
    pub fn with_default_access_key(mut self) -> Self {
        self.state.push(StateRecord::AccessKey {
            account_id: self.destination_account.clone(),
            public_key_base64: DEFAULT_GENESIS_ACCOUNT_PUBLIC_KEY.to_owned(),
            access_key: serde_json::json!({
                "nonce": 0,
                "permission": "FullAccess"
            }),
        });

        self
    }

    pub fn received_data(mut self, data_id_hash: String, data_base64: Option<String>) -> Self {
        self.state.push(StateRecord::ReceivedData {
            account_id: self.destination_account.clone(),
            data_id_hash,
            data_base64,
        });

        self
    }

    pub fn state_record(mut self, state_record: StateRecord) -> Self {
        self.state.push(state_record);
        self
    }

    /// Will fetch account from sandbox if account is not provided and not fetched
    pub const fn initial_balance(mut self, balance: NearToken) -> Self {
        self.initial_balance = Some(balance);
        self
    }

    pub async fn send(self) -> Result<(), SandboxRpcError> {
        let records = if let Some(balance) = self.initial_balance {
            self.process_initial_balance(balance).await?
        } else {
            self.state
        };

        self.sandbox
            .send_request(
                &self.sandbox.rpc_addr,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "0",
                    "method": "sandbox_patch_state",
                    "params": {
                        "records": records,
                    },
                }),
            )
            .await?;

        // NOTE: For some reason, patching anything with account/contract related items takes two patches
        // otherwise its super non-deterministic and mostly just fails to locate the account afterwards: ¯\_(ツ)_/¯
        // From: https://github.com/near/near-workspaces-rs/commit/2b72b9b8491c3140ff2d30b0c45d09b200cb027b
        // Also: https://github.com/near/near-workspaces-rs/blob/918f6deede97170a125c1fd1d80097685015ad2a/workspaces/src/rpc/patch.rs#L328
        self.sandbox
            .send_request(
                &self.sandbox.rpc_addr,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "0",
                    "method": "sandbox_patch_state",
                    "params": {
                        "records": records,
                    },
                }),
            )
            .await?;

        Ok(())
    }

    async fn process_initial_balance(
        &self,
        balance: NearToken,
    ) -> Result<Vec<StateRecord>, SandboxRpcError> {
        let mut records = self.state.clone();
        // Find if there's already an account state record
        let account_exists = records.iter_mut().find_map(|record| {
            if let StateRecord::Account { account, .. } = record {
                Some(account)
            } else {
                None
            }
        });

        if let Some(account) = account_exists {
            // Modify existing account
            if let Some(obj) = account.as_object_mut() {
                obj["amount"] = serde_json::json!(balance);
            }
        } else {
            // Fetch from sandbox and modify
            let mut account = self
                .sandbox
                .send_request(
                    &self.sandbox.rpc_addr,
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": "0",
                        "method": "query",
                        "params": {
                            "finality": "optimistic",
                            "request_type": "view_account",
                            "account_id": self.destination_account
                        }
                    }),
                )
                .await?;

            if let Some(obj) = account
                .get_mut("result")
                .ok_or(SandboxRpcError::UnexpectedResponse)?
                .as_object_mut()
            {
                obj["amount"] = serde_json::json!(balance.to_string());
            }

            records.insert(
                0,
                StateRecord::Account {
                    account_id: self.destination_account.clone(),
                    account: account
                        .get_mut("result")
                        .ok_or(SandboxRpcError::UnexpectedResponse)?
                        .clone(),
                },
            );
        }

        Ok(records)
    }

    async fn fetch_account(
        self,
        account_id: &AccountId,
        from_rpc: Url,
    ) -> Result<PatchState<'a>, SandboxRpcError> {
        let account = self
            .sandbox
            .send_request(
                from_rpc,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "0",
                    "method": "query",
                    "params": {
                        "finality": "optimistic",
                        "request_type": "view_account",
                        "account_id": account_id
                    }
                }),
            )
            .await?;

        Ok(self.account(
            account
                .get("result")
                .ok_or(SandboxRpcError::UnexpectedResponse)?,
        ))
    }

    async fn fetch_storage(
        self,
        account_id: &AccountId,
        from_rpc: Url,
    ) -> Result<PatchState<'a>, SandboxRpcError> {
        let storage = self
            .sandbox
            .send_request(
                from_rpc,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "0",
                    "method": "query",
                    "params": {
                        "finality": "optimistic",
                        "request_type": "view_state",
                        "account_id": account_id,
                        "include_proof": false,
                        "prefix_base64": "",
                    }
                }),
            )
            .await?;

        let default_entry = Self::EMPTY;
        let entries = storage
            .get("result")
            .ok_or(SandboxRpcError::UnexpectedResponse)?
            .get("values")
            .ok_or(SandboxRpcError::UnexpectedResponse)?
            .as_array()
            .unwrap_or(&default_entry)
            .iter()
            .flat_map(|state| {
                Some((
                    state.get("key")?.as_str()?.to_owned(),
                    state.get("value")?.as_str()?.to_owned(),
                ))
            });

        Ok(self.storage_entries(entries))
    }

    async fn fetch_code(
        self,
        account_id: &AccountId,
        from_rpc: Url,
    ) -> Result<PatchState<'a>, SandboxRpcError> {
        let code_response = self
            .sandbox
            .send_request(
                from_rpc,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "0",
                    "method": "query",
                    "params": {
                        "finality": "optimistic",
                        "request_type": "view_code",
                        "account_id": account_id,
                    }
                }),
            )
            .await?;

        let code_base64 = code_response
            .get("result")
            .ok_or(SandboxRpcError::UnexpectedResponse)?
            .get("code_base64")
            .ok_or(SandboxRpcError::UnexpectedResponse)?
            .as_str()
            .unwrap_or_default()
            .to_owned();

        Ok(self.code(code_base64))
    }

    async fn fetch_access_keys(
        mut self,
        account_id: &AccountId,
        from_rpc: Url,
    ) -> Result<PatchState<'a>, SandboxRpcError> {
        let access_keys = self
            .sandbox
            .send_request(
                from_rpc,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "0",
                    "method": "query",
                    "params": {
                        "finality": "optimistic",
                        "request_type": "view_access_key_list",
                        "account_id": account_id,
                    }
                }),
            )
            .await?;

        for access_key in access_keys
            .get("result")
            .ok_or(SandboxRpcError::UnexpectedResponse)?
            .get("keys")
            .ok_or(SandboxRpcError::UnexpectedResponse)?
            .as_array()
            .unwrap_or(&Self::EMPTY)
        {
            self = self.access_key(
                access_key
                    .get("public_key")
                    .ok_or(SandboxRpcError::UnexpectedResponse)?
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                access_key
                    .get("access_key")
                    .ok_or(SandboxRpcError::UnexpectedResponse)?
                    .clone(),
            );
        }

        Ok(self)
    }
}

/// We don't want to introduce extra dependencies to the crate so we use serde_json::Value
/// to represent more complex types.
///
/// Though we still want to have at least some type safety.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum StateRecord {
    Account {
        account_id: AccountId,
        account: serde_json::Value,
    },
    Data {
        account_id: AccountId,
        #[serde(rename = "data_key")]
        data_key_base64: String,
        #[serde(rename = "value")]
        value_base64: String,
    },
    Contract {
        account_id: AccountId,
        #[serde(rename = "code")]
        code_base64: String,
    },
    AccessKey {
        account_id: AccountId,
        #[serde(rename = "public_key")]
        public_key_base64: String,
        access_key: serde_json::Value,
    },
    PostponedReceipt(serde_json::Value),
    ReceivedData {
        account_id: AccountId,
        #[serde(rename = "data_id")]
        data_id_hash: String,
        #[serde(rename = "data")]
        data_base64: Option<String>,
    },
    DelayedReceipt(serde_json::Value),
}

#[cfg(test)]
mod tests {
    use crate::{FetchData, Sandbox};
    use near_api::{Account, AccountId, Contract, NearToken, NetworkConfig};

    #[tokio::test]
    async fn test_patch_state() {
        let sandbox = Sandbox::start_sandbox().await.unwrap();
        let sandbox_network =
            NetworkConfig::from_rpc_url("sandbox", sandbox.rpc_addr.parse().unwrap());
        let account_id: AccountId = "race-of-sloths.testnet".parse().unwrap();

        let account_data = Account(account_id.clone())
            .view()
            .fetch_from_testnet()
            .await
            .unwrap()
            .data;
        let code = Contract(account_id.clone())
            .wasm()
            .fetch_from_testnet()
            .await
            .unwrap()
            .data;
        let state = Contract(account_id.clone())
            .view_storage()
            .fetch_from_testnet()
            .await
            .unwrap()
            .data;

        sandbox
            .patch_state(account_id.clone())
            .account(account_data.clone())
            .code(code.code_base64)
            .storage_entries(state.values.into_iter().map(|s| (s.key.0, s.value.0)))
            .send()
            .await
            .unwrap();

        let sandbox_account_data = Account(account_id.clone())
            .view()
            .fetch_from(&sandbox_network)
            .await
            .unwrap()
            .data;

        assert_eq!(account_data, sandbox_account_data);

        let stats: serde_json::Value = Contract(account_id)
            .call_function(
                "user",
                serde_json::json!({ "user": "akorchyn", "periods": ["all-time"] }),
            )
            .unwrap()
            .read_only()
            .fetch_from(&sandbox_network)
            .await
            .unwrap()
            .data;

        assert_eq!(stats["name"], "akorchyn");
        assert_eq!(stats["id"], 0);

        println!("{:#?}", stats);
    }

    #[tokio::test]
    async fn test_patch_state_with_own_fetcher() {
        let sandbox = Sandbox::start_sandbox().await.unwrap();
        let sandbox_network =
            NetworkConfig::from_rpc_url("sandbox", sandbox.rpc_addr.parse().unwrap());
        let account_id: AccountId = "race-of-sloths.testnet".parse().unwrap();

        let rpc = NetworkConfig::testnet();
        let rpc = rpc.rpc_endpoints.first().unwrap().url.clone();

        sandbox
            .patch_state(account_id.clone())
            .fetch_from(rpc, FetchData::ALL)
            .await
            .unwrap()
            .initial_balance(NearToken::from_near(666))
            .send()
            .await
            .unwrap();

        let sandbox_account_data = Account(account_id.clone())
            .view()
            .fetch_from(&sandbox_network)
            .await
            .unwrap()
            .data;

        assert_eq!(NearToken::from_near(666), sandbox_account_data.amount);
        assert_eq!(
            Contract(account_id.clone())
                .wasm()
                .fetch_from(&NetworkConfig::testnet())
                .await
                .unwrap()
                .data
                .code_base64,
            Contract(account_id.clone())
                .wasm()
                .fetch_from(&sandbox_network)
                .await
                .unwrap()
                .data
                .code_base64
        );

        let stats: serde_json::Value = Contract(account_id)
            .call_function(
                "user",
                serde_json::json!({ "user": "akorchyn", "periods": ["all-time"] }),
            )
            .unwrap()
            .read_only()
            .fetch_from(&sandbox_network)
            .await
            .unwrap()
            .data;

        assert_eq!(stats["name"], "akorchyn");
        assert_eq!(stats["id"], 0);

        println!("{:#?}", stats);
    }
}
