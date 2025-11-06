use near_api::{NearToken, NetworkConfig, Signer};
use near_sandbox::{
    config::{DEFAULT_GENESIS_ACCOUNT, DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY},
    Sandbox,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let sandbox = Sandbox::start_sandbox().await.unwrap();
    let sandbox_network =
        near_api::NetworkConfig::from_rpc_url("sandbox", sandbox.rpc_addr.parse().unwrap());

    let account_id: near_api::AccountId = "race-of-sloths.testnet".parse().unwrap();

    let rpc = NetworkConfig::testnet();
    let rpc = rpc.rpc_endpoints.first().unwrap().url.as_ref();

    sandbox
        .patch_state(account_id.clone())
        .fetch_account(rpc)
        .await
        .unwrap()
        .with_default_access_key()
        .fetch_code(rpc)
        .await
        .unwrap()
        .fetch_storage(rpc)
        .await
        .unwrap()
        .send()
        .await
        .unwrap();

    near_api::Tokens::account(account_id.clone())
        .send_to(DEFAULT_GENESIS_ACCOUNT.to_owned())
        .near(NearToken::from_near(1))
        .with_signer(
            Signer::new(Signer::from_secret_key(
                DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY.parse().unwrap(),
            ))
            .unwrap(),
        )
        .send_to(&sandbox_network)
        .await
        .unwrap()
        .assert_success();

    let value: serde_json::Value = near_api::Contract(account_id)
        .call_function(
            "user",
            serde_json::json!({
                "user": "akorchyn",
                "periods": vec!["all-time"]
            }),
        )
        .unwrap()
        .read_only()
        .fetch_from(&sandbox_network)
        .await
        .unwrap()
        .data;

    assert!(!value["period_data"].as_array().unwrap().is_empty());

    Ok(())
}
