use near_sandbox::Sandbox;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let sandbox = Sandbox::start_sandbox().await.unwrap();
    let network =
        near_api::NetworkConfig::from_rpc_url("sandbox", sandbox.rpc_addr.parse().unwrap());

    let height = near_api::Chain::block_number()
        .fetch_from(&network)
        .await
        .unwrap();

    sandbox.fast_forward(1000).await.unwrap();

    let new_height = near_api::Chain::block_number()
        .fetch_from(&network)
        .await
        .unwrap();

    assert!(new_height >= height + 1000, "expected new height({new_height}) to be at least 1000 blocks higher than the original height({height})");

    Ok(())
}
