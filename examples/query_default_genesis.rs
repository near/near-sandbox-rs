use near_api::{Account, AccountId, NetworkConfig};
use near_sandbox::Sandbox;
use near_sandbox::config::{
    DEFAULT_GENESIS_ACCOUNT, DEFAULT_GENESIS_ACCOUNT_BALANCE, DEFAULT_GENESIS_ACCOUNT_PUBLIC_KEY,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let sandbox = Sandbox::start_sandbox().await.unwrap();
    let network = NetworkConfig::from_rpc_url("sandbox", sandbox.rpc_addr.parse().unwrap());

    let genesis_account: AccountId = DEFAULT_GENESIS_ACCOUNT.as_str().parse().unwrap();

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
        .data
        .first()
        .unwrap()
        .0
        .clone();

    assert!(genesis_account_amount == DEFAULT_GENESIS_ACCOUNT_BALANCE);
    assert!(genesis_account_public_key.to_string() == DEFAULT_GENESIS_ACCOUNT_PUBLIC_KEY);

    Ok(())
}
