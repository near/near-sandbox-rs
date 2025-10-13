use near_sandbox::Sandbox;

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
        .account(account_data)
        .code(code.code_base64)
        .states(state.values.into_iter().map(|s| (s.key.0, s.value.0)))
        .send()
        .await
        .unwrap();

    // Step 3: Query the state
    let account_data = near_api::Account(account_id.clone())
        .view()
        .fetch_from(&sandbox_network)
        .await
        .unwrap()
        .data;

    assert_eq!(account_data, account_data);

    Ok(())
}
