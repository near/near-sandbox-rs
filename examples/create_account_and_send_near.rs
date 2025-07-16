use near_api::{signer, Account, AccountId, NearToken, NetworkConfig, RPCEndpoint, Signer, Tokens};
use near_sandbox::{GenesisAccount, Sandbox};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let sandbox = Sandbox::start_sandbox().await.unwrap();
    let network_config = NetworkConfig {
        network_name: "sandbox".to_string(),
        rpc_endpoints: vec![RPCEndpoint::new(sandbox.rpc_addr.parse().unwrap())],
        ..NetworkConfig::testnet()
    };

    let genesis_account_default = GenesisAccount::default();
    let genesis_account_id: AccountId = genesis_account_default.account_id;
    let genesis_signer: Arc<Signer> = Signer::new(Signer::from_secret_key(
        genesis_account_default.private_key.parse().unwrap(),
    ))
    .unwrap();

    let new_account_id: AccountId = format!("{}.{}", "bob", genesis_account_id).parse().unwrap();
    let new_account_secret_key = signer::generate_secret_key().unwrap();

    Account::create_account(new_account_id.clone())
        .fund_myself(genesis_account_id.clone(), NearToken::from_near(1))
        .public_key(new_account_secret_key.public_key())
        .unwrap()
        .with_signer(genesis_signer.clone())
        .send_to(&network_config)
        .await
        .unwrap();

    Tokens::account(genesis_account_id.clone())
        .send_to(new_account_id.clone())
        .near(NearToken::from_near(1))
        .with_signer(genesis_signer)
        .send_to(&network_config)
        .await
        .unwrap();

    let genesis_account_balance = Tokens::account(genesis_account_id.clone())
        .near_balance()
        .fetch_from(&network_config)
        .await
        .unwrap();

    let new_account_balance = Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network_config)
        .await
        .unwrap();

    println!("Genesis Balance: {}", genesis_account_balance.total);

    // We expect to see 2 NEAR in Bob's account. 1 NEAR from create_account and 1 NEAR from send_near.
    println!("Bob balance: {}", new_account_balance.total);

    Ok(())
}
