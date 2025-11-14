use near_api::{AccountId, NearToken};
use near_sandbox::Sandbox;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let sandbox = Sandbox::start_sandbox().await?;
    let sandbox_network =
        near_api::NetworkConfig::from_rpc_url("sandbox", sandbox.rpc_addr.parse()?);

    let user: AccountId = "user.testnet".parse()?;

    near_api::Tokens::account(user.clone())
        .near_balance()
        .fetch_from(&sandbox_network)
        .await
        .expect_err("User account should not exist");

    sandbox
        .create_account(user.clone())
        .initial_balance(NearToken::from_near(1))
        .send()
        .await?;

    let balance = near_api::Tokens::account(user.clone())
        .near_balance()
        .fetch_from(&sandbox_network)
        .await?;
    assert_eq!(balance.total, NearToken::from_near(1));

    Ok(())
}
