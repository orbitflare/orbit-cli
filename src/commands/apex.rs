use crate::cli::ApexCommands;
use crate::commands::pay::confirm_prompt;
use crate::error::CliResult;
use crate::output;
use base64::Engine;
use colored::Colorize;
use comfy_table::{
    Cell, Color as TColor, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};
use orbitflare_apex::prelude::*;
use orbitflare_apex::rpc::{RpcClient as ApexRpc, RpcError};
use orbitflare_apex::{AdmissionCode, ApexSenderClient, Error as ApexError, tip};
use orbitflare_sdk::RpcClient;
use serde_json::json;
use std::cmp::Ordering;
use std::time::{Duration, Instant};

const DEFAULT_KEYPAIR_PATH: &str = "~/.config/solana/id.json";
const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
const TRANSFER_CU_LIMIT: u32 = 2_000;
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(60);
const TIP_RULE: &str = "Apex requires exactly one top-level SystemProgram transfer of at least 0.001 SOL to a tip account (see `orbitflare apex tip-accounts`), with the tip account in the static account keys.";

pub fn needs_rpc(cmd: &ApexCommands) -> bool {
    matches!(
        cmd,
        ApexCommands::Transfer { .. } | ApexCommands::Send { confirm: true, .. }
    )
}

pub async fn run(
    cmd: ApexCommands,
    apex_key: Option<&str>,
    rpc: Option<&RpcClient>,
) -> CliResult<()> {
    match cmd {
        ApexCommands::Ping { region } => ping(region.as_deref()).await,
        ApexCommands::TipAccounts { region } => {
            tip_accounts(parse_region(&region)?, apex_key.unwrap_or("")).await
        }
        ApexCommands::Send {
            transaction,
            file,
            region,
            mev_protect,
            skip_tip_check,
            confirm,
        } => {
            let tx = read_transaction(transaction.as_deref(), file.as_deref())?;
            let region = parse_region(&region)?;
            let key = require_key(apex_key)?;
            let rpc = if confirm { rpc } else { None };
            send(tx, region, key, mev_protect, skip_tip_check, rpc).await
        }
        ApexCommands::Transfer {
            to,
            amount,
            wallet,
            tip,
            cu_price,
            region,
            mev_protect,
        } => {
            let rpc = rpc.ok_or("No RPC endpoint configured")?;
            let request = TransferRequest {
                to: to
                    .parse()
                    .map_err(|_| format!("Invalid recipient address '{}'", to))?,
                lamports: parse_sol(&amount)?,
                tip_lamports: parse_sol(&tip)?,
                cu_price,
            };
            let region = parse_region(&region)?;
            let key = require_key(apex_key)?;
            let wallet = wallet.as_deref().unwrap_or(DEFAULT_KEYPAIR_PATH);
            transfer(request, wallet, region, key, mev_protect, rpc).await
        }
    }
}

fn require_key(apex_key: Option<&str>) -> CliResult<&str> {
    apex_key.ok_or_else(|| {
        "No Apex API key. Run: orbitflare auth set-apex-key <KEY> (or set ORBITFLARE_APEX_KEY)"
            .into()
    })
}

fn parse_region(code: &str) -> CliResult<Region> {
    Region::parse(code).ok_or_else(|| {
        let codes: Vec<&str> = Region::ALL.iter().map(|r| r.code()).collect();
        format!(
            "Unknown Apex region '{}'. Available: {}",
            code,
            codes.join(", ")
        )
        .into()
    })
}

fn new_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

async fn ping(region: Option<&str>) -> CliResult<()> {
    let regions = match region {
        Some(code) => vec![parse_region(code)?],
        None => Region::ALL.to_vec(),
    };

    if !output::is_json() && !output::is_quiet() {
        eprintln!(
            "\n  {} Pinging {} Apex region(s)...",
            "→".dimmed(),
            regions.len()
        );
    }

    let mut tasks = tokio::task::JoinSet::new();
    for region in regions {
        tasks.spawn(async move { (region, ping_region(region).await) });
    }
    let mut results = Vec::new();
    while let Some(result) = tasks.join_next().await {
        results.push(result.map_err(|e| e.to_string())?);
    }
    results.sort_by(|a, b| match (&a.1, &b.1) {
        (Ok(x), Ok(y)) => x.total_cmp(y),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        _ => a.0.code().cmp(b.0.code()),
    });

    if output::is_json() {
        let json: Vec<_> = results
            .iter()
            .map(|(region, result)| {
                json!({
                    "region": region.code(),
                    "host": region.host(),
                    "status": if result.is_ok() { "up" } else { "down" },
                    "latency_ms": result.as_ref().ok(),
                    "error": result.as_ref().err(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    let mut table = new_table();
    table.set_header(vec!["Region", "Host", "Status", "Latency / Error"]);
    for (region, result) in &results {
        let (status, latency) = match result {
            Ok(ms) => (
                Cell::new("✓ UP").fg(TColor::Green),
                Cell::new(format!("{:.0}ms", ms)).fg(TColor::Green),
            ),
            Err(e) => (
                Cell::new("✗ DOWN").fg(TColor::Red),
                Cell::new(e).fg(TColor::Red),
            ),
        };
        table.add_row(vec![
            Cell::new(region.code().to_uppercase()),
            Cell::new(region.host()),
            status,
            latency,
        ]);
    }

    println!("\n  {} {}\n", "●".cyan().bold(), "Apex".bold());
    println!("{table}\n");

    let up = results.iter().filter(|(_, r)| r.is_ok()).count();
    println!(
        "  {} {}/{} up\n",
        if up == results.len() {
            "✓".green().bold()
        } else {
            "!".yellow().bold()
        },
        up,
        results.len()
    );
    Ok(())
}

async fn ping_region(region: Region) -> Result<f64, String> {
    let client = ApexRpc::new(region, "");
    client.ping().await.map_err(describe_ping_error)?;
    let start = Instant::now();
    client.ping().await.map_err(describe_ping_error)?;
    Ok(start.elapsed().as_secs_f64() * 1000.0)
}

fn describe_ping_error(e: RpcError) -> String {
    match e {
        RpcError::Http(e) if e.is_timeout() => "connection timed out".into(),
        RpcError::Http(e) if e.is_connect() => "connection failed".into(),
        RpcError::Http(e) => e
            .status()
            .map_or_else(|| e.to_string(), |s| format!("HTTP {}", s)),
        RpcError::BadResponse => "unexpected response".into(),
        e => e.to_string(),
    }
}

async fn tip_accounts(region: Region, key: &str) -> CliResult<()> {
    let accounts = ApexRpc::new(region, key).get_tip_accounts().await?;
    let accounts: Vec<String> = accounts.iter().map(|a| a.to_string()).collect();

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&accounts)?);
        return Ok(());
    }
    if output::is_raw() {
        for account in &accounts {
            println!("{}", account);
        }
        return Ok(());
    }

    println!(
        "\n  {} Apex Tip Accounts ({})\n",
        "●".cyan().bold(),
        region.code().to_uppercase()
    );
    let mut table = new_table();
    table.set_header(vec!["#", "Address"]);
    for (i, account) in accounts.iter().enumerate() {
        table.add_row(vec![Cell::new(i + 1), Cell::new(account).fg(TColor::Cyan)]);
    }
    println!("{table}\n");
    println!("  {}\n", TIP_RULE.dimmed());
    Ok(())
}

fn read_transaction(
    transaction: Option<&str>,
    file: Option<&str>,
) -> CliResult<VersionedTransaction> {
    let input = match file {
        Some(path) => std::fs::read(shellexpand::tilde(path).as_ref())
            .map_err(|e| format!("Cannot read transaction file '{}': {}", path, e))?,
        None => transaction
            .ok_or("Pass a transaction or --file")?
            .as_bytes()
            .to_vec(),
    };
    decode_transaction(&input).ok_or_else(|| {
        "Could not decode a signed transaction (expected base64, base58 or raw bytes)".into()
    })
}

fn decode_transaction(input: &[u8]) -> Option<VersionedTransaction> {
    let text = std::str::from_utf8(input).map(str::trim).unwrap_or("");
    let candidates = [
        base64::engine::general_purpose::STANDARD.decode(text).ok(),
        bs58::decode(text).into_vec().ok(),
        Some(input.to_vec()),
    ];
    candidates
        .into_iter()
        .flatten()
        .find_map(|wire| wincode::deserialize_exact(&wire).ok())
}

fn check_tip(tx: &VersionedTransaction, tip_accounts: &[Pubkey]) -> Result<(Pubkey, u64), String> {
    let message = &tx.message;
    let keys = message.static_account_keys();
    let signers = message.header().num_required_signatures as usize;
    let tips: Vec<(Pubkey, u64)> = message
        .instructions()
        .iter()
        .filter_map(|ix| {
            let program = keys.get(ix.program_id_index as usize)?;
            if *program != solana_system_interface::program::ID {
                return None;
            }
            let lamports = transfer_lamports(&ix.data)?;
            let from = *ix.accounts.first()? as usize;
            let to = keys.get(*ix.accounts.get(1)? as usize)?;
            (from < signers && tip_accounts.contains(to)).then_some((*to, lamports))
        })
        .collect();

    match tips.as_slice() {
        [] => Err(format!("No Apex tip found. {}", TIP_RULE)),
        [(_, lamports)] if *lamports < MIN_TIP_LAMPORTS => Err(format!(
            "Tip of {} SOL is below the 0.001 SOL floor. {}",
            format_sol(*lamports),
            TIP_RULE
        )),
        [tip] => Ok(*tip),
        many => Err(format!(
            "Found {} Apex tips, exactly one is allowed. {}",
            many.len(),
            TIP_RULE
        )),
    }
}

fn transfer_lamports(data: &[u8]) -> Option<u64> {
    let (tag, amount) = data.split_first_chunk::<4>()?;
    let amount: [u8; 8] = amount.try_into().ok()?;
    (u32::from_le_bytes(*tag) == 2).then_some(u64::from_le_bytes(amount))
}

fn parse_sol(amount: &str) -> Result<u64, String> {
    let invalid = || format!("Invalid SOL amount '{}'", amount);
    let (whole, frac) = amount.split_once('.').unwrap_or((amount, ""));
    let mut digits = whole.chars().chain(frac.chars());
    if (whole.is_empty() && frac.is_empty())
        || frac.len() > 9
        || !digits.all(|c| c.is_ascii_digit())
    {
        return Err(invalid());
    }
    let whole: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| invalid())?
    };
    let frac: u64 = format!("{:0<9}", frac).parse().map_err(|_| invalid())?;
    whole
        .checked_mul(LAMPORTS_PER_SOL)
        .and_then(|w| w.checked_add(frac))
        .ok_or_else(invalid)
}

fn format_sol(lamports: u64) -> String {
    let frac = format!("{:09}", lamports % LAMPORTS_PER_SOL);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        format!("{}", lamports / LAMPORTS_PER_SOL)
    } else {
        format!("{}.{}", lamports / LAMPORTS_PER_SOL, frac)
    }
}

async fn send(
    tx: VersionedTransaction,
    region: Region,
    key: &str,
    mev_protect: bool,
    skip_tip_check: bool,
    rpc: Option<&RpcClient>,
) -> CliResult<()> {
    if !skip_tip_check {
        eprintln!("  {} Checking Apex tip...", "→".dimmed());
        let accounts = ApexRpc::new(region, key).get_tip_accounts().await?;
        let (account, lamports) = check_tip(&tx, &accounts)?;
        eprintln!(
            "  {} Tip: {} SOL to {}",
            "✓".green(),
            format_sol(lamports),
            account.to_string().cyan()
        );
    }

    let signature = submit(&tx, region, key, mev_protect).await?;
    let slot = match rpc {
        Some(rpc) => Some(confirm(rpc, &signature.to_string()).await?),
        None => None,
    };
    print_result(&signature, region, slot)
}

struct TransferRequest {
    to: Pubkey,
    lamports: u64,
    tip_lamports: u64,
    cu_price: u64,
}

async fn transfer(
    request: TransferRequest,
    wallet: &str,
    region: Region,
    key: &str,
    mev_protect: bool,
    rpc: &RpcClient,
) -> CliResult<()> {
    if request.tip_lamports < MIN_TIP_LAMPORTS {
        return Err(format!("Tip must be at least 0.001 SOL. {}", TIP_RULE).into());
    }

    eprintln!("  {} Loading keypair...", "→".dimmed());
    let payer = read_keypair_file(shellexpand::tilde(wallet).as_ref())
        .map_err(|e| format!("Cannot read keypair file '{}': {}", wallet, e))?;
    eprintln!(
        "  {} Wallet: {}",
        "✓".green(),
        payer.pubkey().to_string().cyan()
    );

    eprintln!("  {} Fetching tip accounts and blockhash...", "→".dimmed());
    let accounts = ApexRpc::new(region, key).get_tip_accounts().await?;
    let tip_account = tip::pick_tip_account(&accounts).ok_or("Apex returned no tip accounts")?;
    let (blockhash, _) = rpc.get_latest_blockhash().await?;
    let blockhash: Hash = blockhash
        .parse()
        .map_err(|_| format!("Invalid blockhash from RPC: {}", blockhash))?;

    let tx = build_transfer(&payer, &request, &tip_account, blockhash);
    check_tip(&tx, &accounts)?;

    if !output::is_json() {
        println!();
        println!("  To:      {}", request.to.to_string().cyan());
        println!(
            "  Amount:  {}",
            format!("{} SOL", format_sol(request.lamports)).bold()
        );
        println!(
            "  Tip:     {} SOL to {}",
            format_sol(request.tip_lamports),
            tip_account
        );
        println!("  Region:  {}", region.code().to_uppercase());
        println!();

        if !confirm_prompt("  Send via Apex? (y/N) ")? {
            eprintln!("  {}", "Cancelled.".dimmed());
            return Ok(());
        }
    }

    let signature = submit(&tx, region, key, mev_protect).await?;
    let slot = confirm(rpc, &signature.to_string()).await?;
    print_result(&signature, region, Some(slot))
}

fn build_transfer(
    payer: &Keypair,
    request: &TransferRequest,
    tip_account: &Pubkey,
    blockhash: Hash,
) -> VersionedTransaction {
    let payer_key = payer.pubkey();
    let instructions = [
        ComputeBudgetInstruction::set_compute_unit_limit(TRANSFER_CU_LIMIT),
        ComputeBudgetInstruction::set_compute_unit_price(request.cu_price),
        solana_system_interface::instruction::transfer(&payer_key, &request.to, request.lamports),
        tip_instruction(&payer_key, tip_account, request.tip_lamports),
    ];
    VersionedTransaction::from(Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer_key),
        &[payer],
        blockhash,
    ))
}

async fn submit(
    tx: &VersionedTransaction,
    region: Region,
    key: &str,
    mev_protect: bool,
) -> CliResult<Signature> {
    eprintln!(
        "  {} Sending via Apex QUIC ({})...",
        "→".dimmed(),
        region.quic_endpoint()
    );
    let options = ClientOptions {
        endpoint: Some(region.quic_endpoint()),
        mev_protect,
        proactive_reconnect: false,
        ..Default::default()
    };
    let client = ApexSenderClient::connect_with_options(options, key).await?;
    let result = client.send_transaction_with_response(tx).await;
    client.close();
    match result {
        Ok(signature) => Ok(signature),
        Err(ApexError::Rejected { code, message }) => {
            let hint = if matches!(code, AdmissionCode::NoTip | AdmissionCode::BelowFloor) {
                format!(" {}", TIP_RULE)
            } else {
                String::new()
            };
            Err(format!(
                "Apex rejected the transaction ({:?}): {}{}",
                code, message, hint
            )
            .into())
        }
        Err(e) => Err(e.into()),
    }
}

async fn confirm(rpc: &RpcClient, signature: &str) -> CliResult<Option<u64>> {
    eprintln!("  {} Waiting for confirmation...", "→".dimmed());
    let deadline = Instant::now() + CONFIRM_TIMEOUT;
    loop {
        let result = rpc
            .request(
                "getSignatureStatuses",
                json!([[signature], {"searchTransactionHistory": false}]),
            )
            .await?;
        let status = &result["value"][0];
        if !status.is_null() {
            if !status["err"].is_null() {
                return Err(format!("Transaction failed on chain: {}", status["err"]).into());
            }
            if matches!(
                status["confirmationStatus"].as_str(),
                Some("confirmed" | "finalized")
            ) {
                return Ok(status["slot"].as_u64());
            }
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn print_result(signature: &Signature, region: Region, slot: Option<Option<u64>>) -> CliResult<()> {
    let status = match slot {
        None => "sent",
        Some(None) => "unconfirmed",
        Some(Some(_)) => "confirmed",
    };
    let slot = slot.flatten();

    if output::is_json() {
        let json = json!({
            "status": status,
            "signature": signature.to_string(),
            "region": region.code(),
            "slot": slot,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }
    if output::is_raw() {
        println!("{}", signature);
        return Ok(());
    }

    match status {
        "confirmed" => eprintln!("  {} Confirmed", "✓".green().bold()),
        "unconfirmed" => eprintln!(
            "  {} Not confirmed within {}s",
            "!".yellow().bold(),
            CONFIRM_TIMEOUT.as_secs()
        ),
        _ => eprintln!("  {} Accepted by Apex", "✓".green().bold()),
    }
    println!();
    println!("  Signature: {}", signature.to_string().cyan());
    if let Some(slot) = slot {
        println!("  Slot:      {}", slot);
    }
    println!(
        "  TX:        {}",
        format!("https://solscan.io/tx/{}", signature).cyan()
    );
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tip_accounts() -> Vec<Pubkey> {
        vec![Pubkey::new_unique(), Pubkey::new_unique()]
    }

    fn transfer_tx(payer: &Keypair, transfers: &[(Pubkey, u64)]) -> VersionedTransaction {
        let instructions: Vec<Instruction> = transfers
            .iter()
            .map(|(to, lamports)| {
                solana_system_interface::instruction::transfer(&payer.pubkey(), to, *lamports)
            })
            .collect();
        VersionedTransaction::from(Transaction::new_signed_with_payer(
            &instructions,
            Some(&payer.pubkey()),
            &[payer],
            Hash::new_from_array([7u8; 32]),
        ))
    }

    #[test]
    fn parses_sol_amounts_exactly() {
        assert_eq!(parse_sol("1"), Ok(LAMPORTS_PER_SOL));
        assert_eq!(parse_sol("0.001"), Ok(MIN_TIP_LAMPORTS));
        assert_eq!(parse_sol("0.000000001"), Ok(1));
        assert_eq!(parse_sol(".5"), Ok(500_000_000));
        assert_eq!(parse_sol("2."), Ok(2 * LAMPORTS_PER_SOL));
        for bad in ["", ".", "-1", "1e3", "0.0000000001", "abc", "1.2.3"] {
            assert!(parse_sol(bad).is_err(), "{bad}");
        }
        assert!(parse_sol("99999999999999999999").is_err());
    }

    #[test]
    fn formats_sol_without_trailing_zeros() {
        assert_eq!(format_sol(1), "0.000000001");
        assert_eq!(format_sol(MIN_TIP_LAMPORTS), "0.001");
        assert_eq!(format_sol(3 * LAMPORTS_PER_SOL), "3");
    }

    #[test]
    fn parses_every_region_code() {
        for code in [
            "global", "fra", "ams", "dub", "lon", "nyc", "slc", "sgp", "tyo", "sqq",
        ] {
            let region = parse_region(code).unwrap();
            assert_eq!(region.host(), format!("{code}.apex.orbitflare.com"));
            assert_eq!(
                region.quic_endpoint(),
                format!("{code}.apex.orbitflare.com:7001")
            );
        }
        assert!(parse_region("mars").is_err());
    }

    #[test]
    fn accepts_exactly_one_tip_at_the_floor() {
        let payer = Keypair::new();
        let tips = tip_accounts();
        let tx = transfer_tx(
            &payer,
            &[(Pubkey::new_unique(), 1), (tips[1], MIN_TIP_LAMPORTS)],
        );
        assert_eq!(check_tip(&tx, &tips), Ok((tips[1], MIN_TIP_LAMPORTS)));
    }

    #[test]
    fn rejects_missing_low_and_duplicate_tips() {
        let payer = Keypair::new();
        let tips = tip_accounts();

        let none = transfer_tx(&payer, &[(Pubkey::new_unique(), MIN_TIP_LAMPORTS)]);
        assert!(
            check_tip(&none, &tips)
                .unwrap_err()
                .starts_with("No Apex tip")
        );

        let low = transfer_tx(&payer, &[(tips[0], MIN_TIP_LAMPORTS - 1)]);
        assert!(
            check_tip(&low, &tips)
                .unwrap_err()
                .contains("below the 0.001 SOL floor")
        );

        let two = transfer_tx(
            &payer,
            &[(tips[0], MIN_TIP_LAMPORTS), (tips[1], MIN_TIP_LAMPORTS)],
        );
        assert!(
            check_tip(&two, &tips)
                .unwrap_err()
                .starts_with("Found 2 Apex tips")
        );
    }

    #[test]
    fn ignores_non_transfer_system_instructions() {
        let payer = Keypair::new();
        let tips = tip_accounts();
        let assign = solana_system_interface::instruction::assign(&tips[0], &Pubkey::new_unique());
        let tx = VersionedTransaction::from(Transaction::new_with_payer(
            &[assign],
            Some(&payer.pubkey()),
        ));
        assert!(check_tip(&tx, &tips).is_err());
    }

    #[test]
    fn built_transfer_satisfies_the_tip_rule() {
        let payer = Keypair::new();
        let tips = tip_accounts();
        let request = TransferRequest {
            to: Pubkey::new_unique(),
            lamports: 1,
            tip_lamports: MIN_TIP_LAMPORTS,
            cu_price: 10_000,
        };
        let tx = build_transfer(&payer, &request, &tips[0], Hash::new_from_array([1u8; 32]));
        assert_eq!(check_tip(&tx, &tips), Ok((tips[0], MIN_TIP_LAMPORTS)));
        assert_eq!(tx.signatures.len(), 1);
        assert!(tx.signatures[0].verify(payer.pubkey().as_ref(), &tx.message.serialize()));
    }

    #[test]
    fn transfer_to_a_tip_account_counts_as_a_second_tip() {
        let payer = Keypair::new();
        let tips = tip_accounts();
        let request = TransferRequest {
            to: tips[1],
            lamports: MIN_TIP_LAMPORTS,
            tip_lamports: MIN_TIP_LAMPORTS,
            cu_price: 0,
        };
        let tx = build_transfer(&payer, &request, &tips[0], Hash::default());
        assert!(check_tip(&tx, &tips).is_err());
    }

    #[test]
    fn decodes_base64_base58_and_raw_transactions() {
        let payer = Keypair::new();
        let tx = transfer_tx(&payer, &[(Pubkey::new_unique(), 5)]);
        let wire = orbitflare_apex::serialize_transaction(&tx).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&wire);
        let b58 = bs58::encode(&wire).into_string();

        for input in [format!("{b64}\n").into_bytes(), b58.into_bytes(), wire] {
            assert_eq!(decode_transaction(&input), Some(tx.clone()));
        }
        assert_eq!(decode_transaction(b"not a transaction"), None);
    }

    #[test]
    fn only_transfer_and_confirmed_send_need_rpc() {
        let send = |confirm| ApexCommands::Send {
            transaction: Some(String::new()),
            file: None,
            region: "fra".into(),
            mev_protect: false,
            skip_tip_check: false,
            confirm,
        };
        assert!(!needs_rpc(&send(false)));
        assert!(needs_rpc(&send(true)));
        assert!(!needs_rpc(&ApexCommands::Ping { region: None }));
    }
}
