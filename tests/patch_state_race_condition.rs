//! Test for sandbox_patch_state race condition.
//!
//! This test verifies that `sandbox_patch_state` RPC returns only after
//! the patched state is fully committed and queryable.
//!
//! ## Background
//!
//! A race condition existed in nearcore where `sandbox_patch_state` would
//! return success before the patch was applied, causing immediate queries
//! to fail with "account does not exist" errors.
//!
//! See: https://github.com/near/nearcore/pull/14893
//!
//! ## Running
//!
//! ```bash
//! # With default nearcore (must include the fix)
//! cargo test --test patch_state_race_condition -- --nocapture
//!
//! # With a specific nearcore binary
//! NEAR_SANDBOX_BIN_PATH=/path/to/neard cargo test --test patch_state_race_condition -- --nocapture
//! ```

use near_sandbox::Sandbox;

const CYCLES_PER_SANDBOX: usize = 50;
const NUM_SANDBOXES: usize = 3;

fn send_rpc_request(
    rpc_addr: &str,
    json_body: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let response = ureq::post(rpc_addr)
        .set("Content-Type", "application/json")
        .send_json(&json_body)?;

    let body: serde_json::Value = response.into_json()?;
    Ok(body)
}

fn raw_patch_state(
    rpc_addr: &str,
    account_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let response = send_rpc_request(
        rpc_addr,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "0",
            "method": "sandbox_patch_state",
            "params": {
                "records": [
                    {
                        "Account": {
                            "account_id": account_id,
                            "account": {
                                "amount": "10000000000000000000000000",
                                "locked": "0",
                                "code_hash": "11111111111111111111111111111111",
                                "storage_usage": 182,
                                "storage_paid_at": 0
                            }
                        }
                    },
                    {
                        "AccessKey": {
                            "account_id": account_id,
                            "public_key": "ed25519:5BGSaf6YjVm7565VzWQHNxoyEjwr3jUpRJSGjREvU9dB",
                            "access_key": {
                                "nonce": 0,
                                "permission": "FullAccess"
                            }
                        }
                    }
                ]
            }
        }),
    )?;

    if let Some(error) = response.get("error") {
        return Err(format!("Patch state error: {:?}", error).into());
    }

    Ok(())
}

fn query_account(
    rpc_addr: &str,
    account_id: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let response = send_rpc_request(
        rpc_addr,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "0",
            "method": "query",
            "params": {
                "finality": "optimistic",
                "request_type": "view_account",
                "account_id": account_id
            }
        }),
    )?;

    if let Some(error) = response.get("error") {
        return Err(format!("Query error: {:?}", error).into());
    }

    Ok(response)
}

/// Verifies that `sandbox_patch_state` is immediately reliable without workarounds.
///
/// This test creates multiple sandbox instances and performs many patch+query cycles.
/// Any failure indicates the race condition is present.
#[tokio::test]
async fn test_patch_state_reliability() {
    println!("\n=== Patch State Race Condition Test ===");
    println!(
        "Testing {} sandboxes x {} cycles = {} total patches\n",
        NUM_SANDBOXES,
        CYCLES_PER_SANDBOX,
        NUM_SANDBOXES * CYCLES_PER_SANDBOX
    );

    let mut success_count = 0usize;
    let mut failure_count = 0usize;
    let mut failure_details: Vec<String> = Vec::new();

    for sandbox_num in 1..=NUM_SANDBOXES {
        println!("--- Sandbox {} ---", sandbox_num);

        let sandbox = match Sandbox::start_sandbox().await {
            Ok(s) => s,
            Err(e) => {
                println!("  Failed to start sandbox: {:?}", e);
                continue;
            }
        };

        let rpc_addr = sandbox.rpc_addr.clone();

        for cycle in 0..CYCLES_PER_SANDBOX {
            let account_id = format!("patched-{}-{}.test.near", sandbox_num, cycle);

            let rpc = rpc_addr.clone();
            let account_str = account_id.clone();
            let patch_result =
                tokio::task::spawn_blocking(move || raw_patch_state(&rpc, &account_str)).await;

            if patch_result.is_err() || matches!(patch_result, Ok(Err(_))) {
                let msg = format!("Sandbox {}, Cycle {}: Patch failed", sandbox_num, cycle);
                println!("  [PATCH FAILED] {}", msg);
                failure_details.push(msg);
                failure_count += 1;
                continue;
            }

            // Query immediately after patch - this should always succeed
            let rpc = rpc_addr.clone();
            let account_str = account_id.clone();
            let query_result =
                tokio::task::spawn_blocking(move || query_account(&rpc, &account_str)).await;

            match query_result {
                Ok(Ok(_)) => {
                    success_count += 1;
                }
                Ok(Err(e)) => {
                    let msg = format!("Sandbox {}, Cycle {}: {}", sandbox_num, cycle, e);
                    println!("  [QUERY FAILED] {}", msg);
                    failure_details.push(msg);
                    failure_count += 1;
                }
                Err(e) => {
                    let msg = format!(
                        "Sandbox {}, Cycle {}: Task error: {:?}",
                        sandbox_num, cycle, e
                    );
                    failure_details.push(msg);
                    failure_count += 1;
                }
            }
        }
    }

    let total = success_count + failure_count;

    println!("\n=== Results ===");
    println!("Total: {}", total);
    println!("Successes: {}", success_count);
    println!("Failures: {}", failure_count);

    if total > 0 {
        let failure_rate = (failure_count as f64 / total as f64) * 100.0;
        println!("Failure rate: {:.2}%", failure_rate);
    }

    if failure_count > 0 {
        println!("\nFailure details (first 10):");
        for (i, detail) in failure_details.iter().take(10).enumerate() {
            println!("  {}. {}", i + 1, detail);
        }
        if failure_details.len() > 10 {
            println!("  ... and {} more", failure_details.len() - 10);
        }
    }

    assert_eq!(
        failure_count,
        0,
        "sandbox_patch_state has a race condition: {} failures out of {} queries ({:.2}%)",
        failure_count,
        total,
        if total > 0 {
            (failure_count as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    );

    println!("\n✓ All {} patches were immediately queryable.", total);
}
