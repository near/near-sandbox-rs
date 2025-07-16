use near_api::{Account, AccountId, NetworkConfig, RPCEndpoint};
use near_sandbox::config::{
    DEFAULT_GENESIS_ACCOUNT, DEFAULT_GENESIS_ACCOUNT_BALANCE, DEFAULT_GENESIS_ACCOUNT_PUBLIC_KEY,
};
use near_sandbox::Sandbox;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let sandbox = Sandbox::start_sandbox().await.unwrap();
    let network = NetworkConfig {
        network_name: "sandbox".to_string(),
        rpc_endpoints: vec![RPCEndpoint::new(sandbox.rpc_addr.parse().unwrap())],
        ..NetworkConfig::testnet()
    };

    let genesis_account: AccountId = DEFAULT_GENESIS_ACCOUNT.into();

    let genesis_account_amount = Account(genesis_account.clone())
        .view()
        .fetch_from(&network)
        .await
        .unwrap()
        .data
        .amount;

    let genesis_account_public_key = Account(genesis_account.clone())
        .list_keys()
        .fetch_from(&network)
        .await
        .unwrap()
        .keys
        .first()
        .unwrap()
        .public_key
        .clone();

    assert!(genesis_account_amount == DEFAULT_GENESIS_ACCOUNT_BALANCE.as_yoctonear());
    assert!(genesis_account_public_key == DEFAULT_GENESIS_ACCOUNT_PUBLIC_KEY);

    Ok(())
}
