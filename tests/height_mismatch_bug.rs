//! Targeted test for fast_forward height mismatch bug
//! https://github.com/near/nearcore/issues/9690
//!
//! The specific bug: SandboxFastForwardStatus returns `finished=true`
//! based only on `fastforward_delta == 0`, but this doesn't guarantee
//! that blocks have actually been produced. The RPC can return success
//! while head.height is still behind latest_known.height.
//!
//! This test verifies that after fast_forward completes, the actual
//! block height matches the expected height.

use near_sandbox::Sandbox;
use std::time::{Duration, Instant};

fn send_rpc_request(
    rpc_addr: &str,
    json_body: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let response = ureq::post(rpc_addr)
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(60))
        .send_json(&json_body)?;
    Ok(response.into_json()?)
}

fn get_block_height(rpc_addr: &str) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let response = send_rpc_request(
        rpc_addr,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "0",
            "method": "status",
        }),
    )?;

    response
        .get("result")
        .and_then(|r| r.get("sync_info"))
        .and_then(|s| s.get("latest_block_height"))
        .and_then(|h| h.as_u64())
        .ok_or_else(|| "Failed to get block height".into())
}

fn fast_forward(
    rpc_addr: &str,
    delta: u64,
    timeout_secs: u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let response = ureq::post(rpc_addr)
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(timeout_secs))
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": "0",
            "method": "sandbox_fast_forward",
            "params": { "delta_height": delta }
        }))?;

    let body: serde_json::Value = response.into_json()?;

    if let Some(error) = body.get("error") {
        return Err(format!("fast_forward error: {:?}", error).into());
    }
    Ok(())
}

/// This test specifically checks for the height mismatch bug.
///
/// The bug: fast_forward RPC returns success, but the actual block height
/// is less than expected. This happens because the status check only
/// verifies `fastforward_delta == 0`, not that `head.height >= latest_known.height`.
///
/// EXPECTED BEHAVIOR (with fix):
/// - After fast_forward(delta) returns, height should be >= initial + delta
///
/// BUGGY BEHAVIOR (without fix):
/// - fast_forward(delta) returns success
/// - But height is still < initial + delta (blocks not yet produced)
#[tokio::test]
async fn test_height_matches_after_fast_forward() {
    println!("\n=== Test: Height Matches After Fast Forward ===");
    println!("This test verifies that after fast_forward completes,");
    println!("the actual block height matches the expected height.\n");

    let num_trials = 10;
    let delta = 100u64;
    let mut successes = 0;
    let mut height_mismatches = 0;
    let mut other_failures = 0;

    for trial in 0..num_trials {
        println!("--- Trial {}/{} ---", trial + 1, num_trials);

        let sandbox = match Sandbox::start_sandbox().await {
            Ok(s) => s,
            Err(e) => {
                println!("  Failed to start sandbox: {:?}", e);
                other_failures += 1;
                continue;
            }
        };
        let rpc_addr = sandbox.rpc_addr.clone();

        // Get initial height
        let initial_height = match tokio::task::spawn_blocking({
            let rpc = rpc_addr.clone();
            move || get_block_height(&rpc)
        })
        .await
        {
            Ok(Ok(h)) => h,
            _ => {
                println!("  Failed to get initial height");
                other_failures += 1;
                continue;
            }
        };

        println!("  Initial height: {}", initial_height);
        let expected_min_height = initial_height + delta;

        // Call fast_forward
        let start = Instant::now();
        let ff_result = tokio::task::spawn_blocking({
            let rpc = rpc_addr.clone();
            move || fast_forward(&rpc, delta, 30)
        })
        .await;

        let elapsed = start.elapsed();

        match ff_result {
            Ok(Ok(())) => {
                // fast_forward returned success - now check if height is correct
                let actual_height = match tokio::task::spawn_blocking({
                    let rpc = rpc_addr.clone();
                    move || get_block_height(&rpc)
                })
                .await
                {
                    Ok(Ok(h)) => h,
                    _ => {
                        println!("  Failed to get final height after ff success");
                        other_failures += 1;
                        continue;
                    }
                };

                println!(
                    "  fast_forward returned in {:?}, height now: {}",
                    elapsed, actual_height
                );

                if actual_height >= expected_min_height {
                    println!(
                        "  ✓ SUCCESS: height {} >= expected {}",
                        actual_height, expected_min_height
                    );
                    successes += 1;
                } else {
                    println!(
                        "  ✗ HEIGHT MISMATCH BUG: height {} < expected {}",
                        actual_height, expected_min_height
                    );
                    println!(
                        "    Missing {} blocks!",
                        expected_min_height - actual_height
                    );
                    height_mismatches += 1;
                }
            }
            Ok(Err(e)) => {
                println!("  fast_forward error: {}", e);
                other_failures += 1;
            }
            Err(e) => {
                println!("  Task failed: {:?}", e);
                other_failures += 1;
            }
        }

        // Small delay between trials
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!("\n=== Results ===");
    println!("Trials:           {}", num_trials);
    println!("Successes:        {}", successes);
    println!("Height mismatches: {} (THE BUG)", height_mismatches);
    println!("Other failures:   {}", other_failures);

    // The test passes if there are no height mismatches
    // (other failures like sandbox crashes are a separate issue)
    assert_eq!(
        height_mismatches, 0,
        "Height mismatch bug detected! fast_forward returned success but height was incorrect."
    );
}

/// A rapid-fire version that does multiple fast_forwards in a single sandbox session
/// This is more likely to trigger the race condition
#[tokio::test]
async fn test_rapid_fast_forwards_height_check() {
    println!("\n=== Test: Rapid Fast Forwards Height Check ===\n");

    let sandbox = Sandbox::start_sandbox()
        .await
        .expect("Failed to start sandbox");
    let rpc_addr = sandbox.rpc_addr.clone();

    let num_calls = 20;
    let delta = 50u64;
    let mut height_mismatches = 0;

    for i in 0..num_calls {
        let height_before = tokio::task::spawn_blocking({
            let rpc = rpc_addr.clone();
            move || get_block_height(&rpc)
        })
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(0);

        let expected_min = height_before + delta;

        let result = tokio::task::spawn_blocking({
            let rpc = rpc_addr.clone();
            move || fast_forward(&rpc, delta, 20)
        })
        .await;

        match result {
            Ok(Ok(())) => {
                let height_after = tokio::task::spawn_blocking({
                    let rpc = rpc_addr.clone();
                    move || get_block_height(&rpc)
                })
                .await
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or(0);

                if height_after < expected_min {
                    println!(
                        "[{}/{}] HEIGHT MISMATCH: {} -> {} (expected >= {}), missing {} blocks",
                        i + 1,
                        num_calls,
                        height_before,
                        height_after,
                        expected_min,
                        expected_min - height_after
                    );
                    height_mismatches += 1;
                } else if i % 5 == 0 {
                    println!(
                        "[{}/{}] OK: {} -> {} (expected >= {})",
                        i + 1,
                        num_calls,
                        height_before,
                        height_after,
                        expected_min
                    );
                }
            }
            Ok(Err(e)) => {
                println!("[{}/{}] Error: {}", i + 1, num_calls, e);
                break; // Sandbox may have crashed
            }
            Err(e) => {
                println!("[{}/{}] Task failed: {:?}", i + 1, num_calls, e);
                break;
            }
        }
    }

    println!("\nHeight mismatches detected: {}", height_mismatches);
    assert_eq!(
        height_mismatches, 0,
        "Height mismatch bug detected in rapid test!"
    );
}
