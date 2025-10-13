use near_account_id::AccountId;
use serde::Serialize;

use crate::{error_kind::SandboxRpcError, Sandbox};

#[derive(Clone)]
pub struct PatchState<'a> {
    pub destination_account: AccountId,
    pub state: Vec<StateRecord>,
    /// We do it as a reference to avoid situations where patch state is alive but sandbox is dropped
    /// so it will end up in the situation where RPC is not available anymore
    pub sandbox: &'a Sandbox,
}

impl<'a> PatchState<'a> {
    pub fn new(destination_account: AccountId, sandbox: &'a Sandbox) -> Self {
        Self {
            state: vec![],
            destination_account,
            sandbox,
        }
    }

    pub fn account(mut self, account: impl Serialize) -> Self {
        self.state.push(StateRecord::Account {
            account_id: self.destination_account.clone(),
            account: serde_json::to_value(account).unwrap(),
        });

        self
    }

    pub fn state(mut self, state_key_base64: String, state_value_base64: String) -> Self {
        self.state.push(StateRecord::Data {
            account_id: self.destination_account.clone(),
            data_key_base64: state_key_base64,
            value_base64: state_value_base64,
        });

        self
    }

    pub fn states<I: IntoIterator<Item = (String, String)>>(mut self, states: I) -> Self {
        let account_id = self.destination_account.clone();
        self.state.extend(
            states
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

    pub fn received_data(mut self, data_id_hash: String, data_base64: Option<String>) -> Self {
        self.state.push(StateRecord::ReceivedData {
            account_id: self.destination_account.clone(),
            data_id_hash,
            data_base64,
        });

        self
    }

    pub async fn send(&self) -> Result<(), SandboxRpcError> {
        let client = reqwest::Client::new();
        let result = client
            .post(self.sandbox.rpc_addr.clone())
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "0",
                "method": "sandbox_patch_state",
                "params": {
                    "records": self.state,
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
    use crate::Sandbox;
    use near_api::{Account, AccountId, Contract, NetworkConfig};

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
            .account(account_data)
            .code(code.code_base64)
            .states(state.values.into_iter().map(|s| (s.key.0, s.value.0)))
            .send()
            .await
            .unwrap();

        let account_data = Account(account_id.clone())
            .view()
            .fetch_from(&sandbox_network)
            .await
            .unwrap()
            .data;

        assert_eq!(account_data, account_data);

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

        println!("{:?}", stats);
    }
}
