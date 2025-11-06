use near_api::{NearToken, Signer};
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

    // Step 1: Fetch the data you want to patch from other Network
    let account_data = near_api::Account(account_id.clone())
        .view()
        .fetch_from_testnet()
        .await
        .unwrap()
        .data;
    let code = near_api::Contract(account_id.clone())
        .wasm()
        .fetch_from_testnet()
        .await
        .unwrap()
        .data;
    let state = near_api::Contract(account_id.clone())
        .view_storage()
        .fetch_from_testnet()
        .await
        .unwrap()
        .data;

    // Step 2: Patch the state
    sandbox
        .patch_state(account_id.clone())
        .account(account_data.clone())
        .code(code.code_base64)
        .storage_entries(state.values.into_iter().map(|s| (s.key.0, s.value.0)))
        .with_default_access_key()
        .send()
        .await
        .unwrap();

    // Step 3: Query the state
    let sandbox_account_data = near_api::Account(account_id.clone())
        .view()
        .fetch_from(&sandbox_network)
        .await
        .unwrap()
        .data;

    assert_eq!(account_data, sandbox_account_data);

    near_api::Tokens::account(account_id)
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

    Ok(())
}
