use near_api::{NearToken, NetworkConfig};
use near_sandbox::{GenesisAccount, Sandbox, SandboxConfig};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = SandboxConfig {
        additional_genesis: Some(json!({
            "epoch_length": 100,
        })),
        additional_accounts: vec![
            GenesisAccount {
                account_id: "alice.near".parse().unwrap(),
                public_key:"ed25519:AzBN9XwQDRuLvGvor2JnMitkRxBxn2TLY4yEM3othKUF".to_string(),
                private_key: "ed25519:5byt6y8h1uuHwkr2ozfN5gt8xGiHujpcT5KyNhZpG62BrnU51sMQk5eTVNwWp7RRiMgKHp7W1jrByxLCr2apXNGB".to_string(),
                balance: NearToken::from_near(1000),
            },
        ],
        rpc_port: Some(3030),
        ..Default::default()
    };

    let sandbox = Sandbox::start_sandbox_with_config(config).await?;
    let network_config = NetworkConfig::from_rpc_url("sandbox", sandbox.rpc_addr.parse().unwrap());

    println!("Sandbox is running at: {}", sandbox.rpc_addr);
    println!("Sandbox home directory is: {:?}", sandbox.home_dir);

    let tokens = near_api::Tokens::account("alice.near".parse().unwrap())
        .near_balance()
        .fetch_from(&network_config)
        .await?;

    assert_eq!(tokens.total, NearToken::from_near(1000));

    Ok(())
}
