use crate::cli::RpcCommands;
use crate::error::CliResult;
use crate::output;
use colored::Colorize;
use comfy_table::{
    Cell, Color as TColor, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};
use orbitflare_sdk::RpcClient;
use serde_json::{Value, json};

pub async fn run(cmd: RpcCommands, rpc: &RpcClient) -> CliResult<()> {
    match cmd {
        RpcCommands::Account { address } => account(&address, rpc).await,
        RpcCommands::Tokens { wallet } => tokens(&wallet, rpc).await,
        RpcCommands::Balance { address } => balance(&address, rpc).await,
        RpcCommands::Tx { signature } => tx(&signature, rpc).await,
        RpcCommands::History { wallet, limit } => history(&wallet, limit, rpc).await,
        RpcCommands::ProgramAccounts {
            program_id,
            limit,
            data_size,
        } => program_accounts(&program_id, limit, data_size, rpc).await,
        RpcCommands::Slot => slot(rpc).await,
        RpcCommands::Epoch => epoch(rpc).await,
        RpcCommands::Blockhash => blockhash(rpc).await,
        RpcCommands::Stats => stats(rpc).await,
        RpcCommands::PriorityFees { account } => priority_fees(account, rpc).await,
        RpcCommands::Raw { body } => raw(&body, rpc).await,
    }
}

fn new_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}
async fn account(address: &str, rpc: &RpcClient) -> CliResult<()> {
    let result = rpc
        .request(
            "getAccountInfo",
            json!([address, {"encoding": "base64", "commitment": rpc.commitment()}]),
        )
        .await?;

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if output::is_raw() {
        println!("{}", result);
        return Ok(());
    }

    let value = &result["value"];
    if value.is_null() {
        return Err(format!("Account not found: {}", address).into());
    }

    let owner = value["owner"].as_str().unwrap_or("unknown");
    let lamports = value["lamports"].as_u64().unwrap_or(0);
    let executable = value["executable"].as_bool().unwrap_or(false);
    let space = value["space"].as_u64().unwrap_or_else(|| {
        value["data"]
            .as_array()
            .and_then(|d| d.first())
            .and_then(|d| d.as_str())
            .map(|s| (s.len() as u64) * 3 / 4)
            .unwrap_or(0)
    });

    let sol = lamports as f64 / 1_000_000_000.0;

    println!("\n  {} Account Info\n", "●".cyan().bold());

    let mut table = new_table();
    table.set_header(vec!["Field", "Value"]);
    table.add_row(vec![
        Cell::new("Address"),
        Cell::new(address).fg(TColor::Cyan),
    ]);
    table.add_row(vec![Cell::new("Owner"), Cell::new(owner)]);
    table.add_row(vec![
        Cell::new("Lamports"),
        Cell::new(format!("{} ({:.9} SOL)", format_number(lamports), sol)),
    ]);
    table.add_row(vec![Cell::new("Executable"), Cell::new(executable)]);
    table.add_row(vec![
        Cell::new("Data Size"),
        Cell::new(format!("{} bytes", space)),
    ]);

    println!("{table}\n");
    Ok(())
}
async fn tokens(wallet: &str, rpc: &RpcClient) -> CliResult<()> {
    let result = rpc
        .request(
            "getTokenAccountsByOwner",
            json!([
                wallet,
                {"programId": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"},
                {"encoding": "jsonParsed", "commitment": rpc.commitment()}
            ]),
        )
        .await?;

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if output::is_raw() {
        println!("{}", result);
        return Ok(());
    }

    let accounts = result["value"].as_array();
    let accounts = accounts.ok_or("No token accounts found")?;

    if accounts.is_empty() {
        println!("No token accounts found for: {}", wallet.cyan());
        return Ok(());
    }

    println!(
        "\n  {} Token Accounts for {}\n",
        "●".cyan().bold(),
        truncate_addr(wallet)
    );

    let mut table = new_table();
    table.set_header(vec!["#", "Mint", "Balance", "Decimals"]);

    for (i, acct) in accounts.iter().enumerate() {
        let info = &acct["account"]["data"]["parsed"]["info"];
        let mint = info["mint"].as_str().unwrap_or("unknown");
        let ui_amount = info["tokenAmount"]["uiAmountString"]
            .as_str()
            .unwrap_or("0");
        let decimals = info["tokenAmount"]["decimals"].as_u64().unwrap_or(0);

        table.add_row(vec![
            Cell::new(i + 1),
            Cell::new(mint),
            Cell::new(ui_amount),
            Cell::new(decimals),
        ]);
    }

    println!("{table}");
    println!(
        "\n  {} {} token account(s)\n",
        "✓".green().bold(),
        accounts.len()
    );
    Ok(())
}
async fn balance(address: &str, rpc: &RpcClient) -> CliResult<()> {
    let result = rpc
        .request(
            "getBalance",
            json!([address, {"commitment": rpc.commitment()}]),
        )
        .await?;

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if output::is_raw() {
        println!("{}", result);
        return Ok(());
    }

    let lamports = result["value"].as_u64().unwrap_or(0);
    let sol = lamports as f64 / 1_000_000_000.0;

    println!("\n  {} Balance\n", "●".cyan().bold());

    let mut table = new_table();
    table.set_header(vec!["Address", "SOL", "Lamports"]);
    table.add_row(vec![
        Cell::new(address).fg(TColor::Cyan),
        Cell::new(format!("{:.9}", sol)).fg(TColor::Green),
        Cell::new(format_number(lamports)),
    ]);

    println!("{table}\n");
    Ok(())
}
async fn tx(signature: &str, rpc: &RpcClient) -> CliResult<()> {
    let result = rpc
        .request(
            "getTransaction",
            json!([
                signature,
                {
                    "encoding": "jsonParsed",
                    "maxSupportedTransactionVersion": 0,
                    "commitment": rpc.commitment()
                }
            ]),
        )
        .await?;

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if output::is_raw() {
        println!("{}", result);
        return Ok(());
    }

    if result.is_null() {
        return Err(format!("Transaction not found: {}", signature).into());
    }

    let meta = &result["meta"];
    let slot = result["slot"].as_u64().unwrap_or(0);
    let block_time = result["blockTime"].as_i64().unwrap_or(0);
    let fee = meta["fee"].as_u64().unwrap_or(0);
    let compute = meta["computeUnitsConsumed"].as_u64().unwrap_or(0);
    let err = &meta["err"];

    let (status_text, status_color) = if err.is_null() {
        ("✓ Success".to_string(), TColor::Green)
    } else {
        (format!("✗ Failed: {}", err), TColor::Red)
    };

    let time_str = chrono::DateTime::from_timestamp(block_time, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("\n  {} Transaction Details\n", "●".cyan().bold());

    let mut table = new_table();
    table.set_header(vec!["Field", "Value"]);
    table.add_row(vec![
        Cell::new("Signature"),
        Cell::new(signature).fg(TColor::Cyan),
    ]);
    table.add_row(vec![
        Cell::new("Status"),
        Cell::new(&status_text).fg(status_color),
    ]);
    table.add_row(vec![Cell::new("Slot"), Cell::new(format_number(slot))]);
    table.add_row(vec![Cell::new("Block Time"), Cell::new(&time_str)]);
    table.add_row(vec![
        Cell::new("Fee"),
        Cell::new(format!("{} SOL", fee as f64 / 1_000_000_000.0)),
    ]);
    table.add_row(vec![
        Cell::new("Compute"),
        Cell::new(format!("{} CU", format_number(compute))),
    ]);

    if let Some(keys) = result["transaction"]["message"]["accountKeys"].as_array() {
        let signers: Vec<&str> = keys
            .iter()
            .filter(|k| k["signer"].as_bool().unwrap_or(false))
            .filter_map(|k| k["pubkey"].as_str())
            .collect();
        if !signers.is_empty() {
            table.add_row(vec![Cell::new("Signers"), Cell::new(signers.join("\n"))]);
        }
    }

    println!("{table}");

    if let Some(instructions) = result["transaction"]["message"]["instructions"].as_array() {
        println!("\n  {} Instructions\n", "●".cyan().bold());

        let mut ix_table = new_table();
        ix_table.set_header(vec!["#", "Program", "Type"]);

        for (i, ix) in instructions.iter().enumerate() {
            let program = ix["programId"].as_str().unwrap_or("unknown");
            let parsed = &ix["parsed"];
            let ix_type = if !parsed.is_null() {
                parsed["type"].as_str().unwrap_or("—").to_string()
            } else {
                "—".to_string()
            };
            ix_table.add_row(vec![
                Cell::new(i + 1),
                Cell::new(program),
                Cell::new(ix_type),
            ]);
        }

        println!("{ix_table}");
    }

    println!();
    Ok(())
}
async fn history(wallet: &str, limit: u32, rpc: &RpcClient) -> CliResult<()> {
    let result = rpc
        .request(
            "getSignaturesForAddress",
            json!([wallet, {"limit": limit, "commitment": rpc.commitment()}]),
        )
        .await?;

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if output::is_raw() {
        println!("{}", result);
        return Ok(());
    }

    let sigs = result.as_array().ok_or("Invalid response")?;

    if sigs.is_empty() {
        println!("No transactions found for: {}", wallet.cyan());
        return Ok(());
    }

    println!(
        "\n  {} Transaction History for {} (showing {})\n",
        "●".cyan().bold(),
        truncate_addr(wallet),
        sigs.len()
    );

    let mut table = new_table();
    table.set_header(vec!["Time", "Status", "Slot", "Signature"]);

    for sig_info in sigs {
        let sig = sig_info["signature"].as_str().unwrap_or("unknown");
        let slot = sig_info["slot"].as_u64().unwrap_or(0);
        let block_time = sig_info["blockTime"].as_i64().unwrap_or(0);
        let err = &sig_info["err"];

        let time_str = chrono::DateTime::from_timestamp(block_time, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let (status_cell, sig_color) = if err.is_null() {
            (Cell::new("✓ OK").fg(TColor::Green), TColor::White)
        } else {
            (Cell::new("✗ Err").fg(TColor::Red), TColor::Red)
        };

        let sig_short = if sig.len() > 20 {
            format!("{}...{}", &sig[..12], &sig[sig.len() - 4..])
        } else {
            sig.to_string()
        };

        table.add_row(vec![
            Cell::new(&time_str),
            status_cell,
            Cell::new(format_number(slot)),
            Cell::new(sig_short).fg(sig_color),
        ]);
    }

    println!("{table}\n");
    Ok(())
}
async fn program_accounts(
    program_id: &str,
    limit: u32,
    data_size: Option<u64>,
    rpc: &RpcClient,
) -> CliResult<()> {
    let mut config = json!({
        "encoding": "base64",
        "commitment": rpc.commitment(),
    });

    if let Some(size) = data_size {
        config["filters"] = json!([{"dataSize": size}]);
    }
    config["dataSlice"] = json!({"offset": 0, "length": 0});

    let result = rpc
        .request("getProgramAccounts", json!([program_id, config]))
        .await?;

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if output::is_raw() {
        println!("{}", result);
        return Ok(());
    }

    let accounts = result.as_array().ok_or("Invalid response")?;

    let showing = limit.min(accounts.len() as u32);

    println!(
        "\n  {} Program Accounts for {} (showing {} of {})\n",
        "●".cyan().bold(),
        truncate_addr(program_id),
        showing,
        accounts.len()
    );

    if let Some(size) = data_size {
        println!("  Data size filter: {} bytes\n", size);
    }

    let mut table = new_table();
    table.set_header(vec!["#", "Public Key", "Data Size"]);

    for (i, acct) in accounts.iter().take(limit as usize).enumerate() {
        let pubkey = acct["pubkey"].as_str().unwrap_or("unknown");
        let space = acct["account"]["space"].as_u64().unwrap_or(0);
        table.add_row(vec![
            Cell::new(i + 1),
            Cell::new(pubkey),
            Cell::new(format!("{} bytes", space)),
        ]);
    }

    println!("{table}\n");
    Ok(())
}
async fn slot(rpc: &RpcClient) -> CliResult<()> {
    let confirmed = rpc
        .request("getSlot", json!([{"commitment": "confirmed"}]))
        .await?;
    let finalized = rpc
        .request("getSlot", json!([{"commitment": "finalized"}]))
        .await?;

    if output::is_json() {
        let combined = json!({"confirmed": confirmed, "finalized": finalized});
        println!("{}", serde_json::to_string_pretty(&combined)?);
        return Ok(());
    }
    if output::is_raw() {
        let combined = json!({"confirmed": confirmed, "finalized": finalized});
        println!("{}", combined);
        return Ok(());
    }

    let confirmed_slot = confirmed.as_u64().unwrap_or(0);
    let finalized_slot = finalized.as_u64().unwrap_or(0);

    println!("\n  {} Current Slot\n", "●".cyan().bold());

    let mut table = new_table();
    table.set_header(vec!["Commitment", "Slot"]);
    table.add_row(vec![
        Cell::new("Confirmed"),
        Cell::new(format_number(confirmed_slot)).fg(TColor::Green),
    ]);
    table.add_row(vec![
        Cell::new("Finalized"),
        Cell::new(format_number(finalized_slot)),
    ]);

    println!("{table}\n");
    Ok(())
}
async fn epoch(rpc: &RpcClient) -> CliResult<()> {
    let result = rpc
        .request("getEpochInfo", json!([{"commitment": rpc.commitment()}]))
        .await?;

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if output::is_raw() {
        println!("{}", result);
        return Ok(());
    }

    let epoch_num = result["epoch"].as_u64().unwrap_or(0);
    let slot_index = result["slotIndex"].as_u64().unwrap_or(0);
    let slots_in_epoch = result["slotsInEpoch"].as_u64().unwrap_or(1);
    let absolute_slot = result["absoluteSlot"].as_u64().unwrap_or(0);

    let progress = (slot_index as f64 / slots_in_epoch as f64) * 100.0;
    let epoch_start = absolute_slot - slot_index;
    let epoch_end = epoch_start + slots_in_epoch;

    let remaining_slots = slots_in_epoch - slot_index;
    let remaining_secs = (remaining_slots as f64 * 0.4) as u64;
    let hours = remaining_secs / 3600;
    let mins = (remaining_secs % 3600) / 60;

    println!("\n  {} Epoch Info\n", "●".cyan().bold());

    let mut table = new_table();
    table.set_header(vec!["Field", "Value"]);
    table.add_row(vec![
        Cell::new("Epoch"),
        Cell::new(format_number(epoch_num)).fg(TColor::Green),
    ]);
    table.add_row(vec![
        Cell::new("Slot Range"),
        Cell::new(format!(
            "{} — {}",
            format_number(epoch_start),
            format_number(epoch_end)
        )),
    ]);
    table.add_row(vec![
        Cell::new("Progress"),
        Cell::new(format!(
            "{:.1}% ({} / {} slots)",
            progress,
            format_number(slot_index),
            format_number(slots_in_epoch)
        )),
    ]);
    table.add_row(vec![
        Cell::new("Time Left"),
        Cell::new(format!("~{}h {}m", hours, mins)),
    ]);

    println!("{table}\n");
    Ok(())
}
async fn blockhash(rpc: &RpcClient) -> CliResult<()> {
    let result = rpc
        .request(
            "getLatestBlockhash",
            json!([{"commitment": rpc.commitment()}]),
        )
        .await?;

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if output::is_raw() {
        println!("{}", result);
        return Ok(());
    }

    let hash = result["value"]["blockhash"].as_str().unwrap_or("unknown");
    let last_valid = result["value"]["lastValidBlockHeight"]
        .as_u64()
        .unwrap_or(0);

    println!("\n  {} Latest Blockhash\n", "●".cyan().bold());

    let mut table = new_table();
    table.set_header(vec!["Field", "Value"]);
    table.add_row(vec![Cell::new("Hash"), Cell::new(hash).fg(TColor::Cyan)]);
    table.add_row(vec![
        Cell::new("Last Valid Height"),
        Cell::new(format_number(last_valid)),
    ]);

    println!("{table}\n");
    Ok(())
}
async fn stats(rpc: &RpcClient) -> CliResult<()> {
    let perf = rpc
        .request("getRecentPerformanceSamples", json!([4]))
        .await?;
    let vote_accounts = rpc.request("getVoteAccounts", json!([])).await?;

    if output::is_json() {
        let combined = json!({"performanceSamples": perf, "voteAccounts": vote_accounts});
        println!("{}", serde_json::to_string_pretty(&combined)?);
        return Ok(());
    }
    if output::is_raw() {
        let combined = json!({"performanceSamples": perf, "voteAccounts": vote_accounts});
        println!("{}", combined);
        return Ok(());
    }

    let samples = perf.as_array();
    let (current_tps, avg_tps) = if let Some(samples) = samples {
        let current = samples
            .first()
            .map(|s| {
                let txs = s["numTransactions"].as_u64().unwrap_or(0);
                let secs = s["samplePeriodSecs"].as_u64().unwrap_or(60);
                if secs > 0 { txs / secs } else { 0 }
            })
            .unwrap_or(0);

        let avg = if !samples.is_empty() {
            let total: u64 = samples
                .iter()
                .map(|s| {
                    let txs = s["numTransactions"].as_u64().unwrap_or(0);
                    let secs = s["samplePeriodSecs"].as_u64().unwrap_or(60);
                    if secs > 0 { txs / secs } else { 0 }
                })
                .sum();
            total / samples.len() as u64
        } else {
            0
        };

        (current, avg)
    } else {
        (0, 0)
    };

    let current_validators = vote_accounts["current"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let delinquent = vote_accounts["delinquent"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);

    println!("\n  {} Network Stats\n", "●".cyan().bold());

    let mut table = new_table();
    table.set_header(vec!["Metric", "Value"]);
    table.add_row(vec![
        Cell::new("TPS (current)"),
        Cell::new(format_number(current_tps)).fg(TColor::Green),
    ]);
    table.add_row(vec![
        Cell::new("TPS (avg recent)"),
        Cell::new(format_number(avg_tps)),
    ]);
    table.add_row(vec![
        Cell::new("Active Validators"),
        Cell::new(format_number(current_validators as u64)),
    ]);
    table.add_row(vec![
        Cell::new("Delinquent"),
        Cell::new(delinquent).fg(if delinquent > 0 {
            TColor::Yellow
        } else {
            TColor::Green
        }),
    ]);

    println!("{table}\n");
    Ok(())
}
async fn priority_fees(account: Option<String>, rpc: &RpcClient) -> CliResult<()> {
    let params = if let Some(ref addr) = account {
        json!([[addr]])
    } else {
        json!([])
    };

    let result = rpc.request("getRecentPrioritizationFees", params).await?;

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if output::is_raw() {
        println!("{}", result);
        return Ok(());
    }

    let fees = result.as_array().ok_or("Invalid response")?;

    if let Some(ref addr) = account {
        println!(
            "\n  {} Priority Fees for {}\n",
            "●".cyan().bold(),
            truncate_addr(addr)
        );
    } else {
        println!("\n  {} Priority Fees (global)\n", "●".cyan().bold());
    }

    if fees.is_empty() {
        println!("  No recent priority fee data available.\n");
        return Ok(());
    }

    let mut fee_values: Vec<u64> = fees
        .iter()
        .filter_map(|f| f["prioritizationFee"].as_u64())
        .filter(|&f| f > 0)
        .collect();

    if fee_values.is_empty() {
        println!("  All recent fees are 0 (no priority fees paid).\n");
        return Ok(());
    }

    fee_values.sort();

    let percentile = |p: usize| -> u64 {
        let idx = (p as f64 / 100.0 * fee_values.len() as f64) as usize;
        fee_values[idx.min(fee_values.len() - 1)]
    };

    let mut table = new_table();
    table.set_header(vec!["Percentile", "Fee (microlamports/CU)"]);
    table.add_row(vec![
        Cell::new("25th"),
        Cell::new(format_number(percentile(25))),
    ]);
    table.add_row(vec![
        Cell::new("50th"),
        Cell::new(format_number(percentile(50))),
    ]);
    table.add_row(vec![
        Cell::new("75th"),
        Cell::new(format_number(percentile(75))),
    ]);
    table.add_row(vec![
        Cell::new("95th"),
        Cell::new(format_number(percentile(95))).fg(TColor::Yellow),
    ]);

    println!("{table}");
    println!(
        "\n  {} Based on {} recent slots ({} with non-zero fees)\n",
        "→".dimmed(),
        fees.len(),
        fee_values.len()
    );
    Ok(())
}
async fn raw(body: &str, rpc: &RpcClient) -> CliResult<()> {
    let parsed: Value = serde_json::from_str(body).map_err(|e| format!("Invalid JSON: {}", e))?;

    let method = parsed["method"]
        .as_str()
        .ok_or("Missing \"method\" in JSON body")?;
    let params = parsed.get("params").cloned().unwrap_or(json!([]));

    let result = rpc.request(method, params).await?;

    if output::is_json() || output::is_raw() {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!("\n  {} Raw RPC: {}\n", "●".cyan().bold(), method);
    println!("{}\n", serde_json::to_string_pretty(&result)?);

    Ok(())
}
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn truncate_addr(addr: &str) -> String {
    if addr.len() > 16 {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}
