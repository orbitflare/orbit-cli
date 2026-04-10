use crate::api_client::OrbitApi;
use crate::cli::PayCommands;
use crate::error::CliResult;
use crate::keyring_store;
use crate::output;
use colored::Colorize;
use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color as TColor,
    ContentArrangement, Table,
};
use ed25519_dalek::{Signer, SigningKey};
use std::io::{self, Write};

const DEFAULT_KEYPAIR_PATH: &str = "~/.config/solana/id.json";

pub async fn run(cmd: PayCommands, profile: Option<&str>) -> CliResult<()> {
    match cmd {
        PayCommands::CheckBalance => check_balance(profile).await,
        PayCommands::Topup { amount, wallet } => topup(amount, wallet.as_deref(), profile).await,
        PayCommands::History => history(profile).await,
        PayCommands::Purchase { slug, period, coupon } => {
            purchase(&slug, &period, coupon.as_deref(), profile).await
        }
        PayCommands::Order { reference, history } => {
            order(reference.as_deref(), history, profile).await
        }
        PayCommands::Invoice { reference, list } => {
            invoice(reference.as_deref(), list, profile).await
        }
        PayCommands::Renew { invoice } => renew(&invoice, profile).await,
    }
}


fn get_api(profile: Option<&str>) -> CliResult<OrbitApi> {
    let profile_name = profile.unwrap_or("default");
    if let Some(key) = keyring_store::get_secret(profile_name, "api_key") {
        Ok(OrbitApi::with_api_key(Some(key)))
    } else if let Some(token) = keyring_store::get_secret(profile_name, "token") {
        Ok(OrbitApi::with_token(&token))
    } else {
        Err("Not authenticated. Run: orbitflare auth login".into())
    }
}

fn load_signing_key(wallet_path: &str) -> CliResult<SigningKey> {
    let expanded = shellexpand::tilde(wallet_path);
    let file_contents = std::fs::read_to_string(expanded.as_ref())
        .map_err(|e| format!("Cannot read keypair file '{}': {}", wallet_path, e))?;

    let keypair_bytes: Vec<u8> = serde_json::from_str(&file_contents)
        .map_err(|e| format!("Invalid keypair format (expected JSON array of bytes): {}", e))?;

    if keypair_bytes.len() != 64 {
        return Err(format!(
            "Invalid keypair: expected 64 bytes, got {}",
            keypair_bytes.len()
        )
        .into());
    }

    let secret_bytes: [u8; 32] = keypair_bytes[..32]
        .try_into()
        .map_err(|_| "Failed to extract secret key bytes")?;

    Ok(SigningKey::from_bytes(&secret_bytes))
}

fn sign_transaction(unsigned_tx_base64: &str, signing_key: &SigningKey) -> CliResult<String> {
    use base64::Engine;
    let base64_engine = base64::engine::general_purpose::STANDARD;

    let mut tx_bytes = base64_engine
        .decode(unsigned_tx_base64)
        .map_err(|e| format!("Invalid transaction encoding: {}", e))?;

    if tx_bytes.len() < 66 {
        return Err("Transaction too short to contain a signature slot".into());
    }

    let num_sigs = tx_bytes[0] as usize;
    if num_sigs == 0 {
        return Err("Transaction has no signature slots".into());
    }

    let message_offset = 1 + (num_sigs * 64);
    if tx_bytes.len() <= message_offset {
        return Err("Transaction too short for declared signatures".into());
    }

    let message = &tx_bytes[message_offset..];
    let signature = signing_key.sign(message);
    tx_bytes[1..65].copy_from_slice(&signature.to_bytes());

    Ok(bs58::encode(tx_bytes).into_string())
}

fn val_to_f64(value: &Option<serde_json::Value>) -> f64 {
    match value {
        Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(serde_json::Value::String(s)) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn format_price(value: &Option<serde_json::Value>) -> String {
    format!("${:.2}", val_to_f64(value))
}

fn confirm_prompt(question: &str) -> CliResult<bool> {
    print!("{}", question);
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let answer = answer.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
}


async fn check_balance(profile: Option<&str>) -> CliResult<()> {
    let api = get_api(profile)?;

    eprintln!("  {} Fetching balance...", "→".dimmed());
    let balance = api.balance().await?;

    if output::is_json() {
        let json = serde_json::json!({
            "balance": balance.balance,
            "currency": balance.currency,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!();
        println!(
            "  Balance: {}",
            format!("${:.2}", balance.balance.unwrap_or(0.0))
                .green()
                .bold()
        );
        if let Some(ref currency) = balance.currency {
            println!("  Currency: {}", currency);
        }
        println!();
    }

    Ok(())
}


async fn topup(amount: f64, wallet_path: Option<&str>, profile: Option<&str>) -> CliResult<()> {
    if amount < 5.0 {
        return Err("Minimum top-up amount is $5 USDC".into());
    }

    let wallet_path = wallet_path.unwrap_or(DEFAULT_KEYPAIR_PATH);

    eprintln!("  {} Loading keypair...", "→".dimmed());
    let signing_key = load_signing_key(wallet_path)?;
    let public_key = signing_key.verifying_key();
    let wallet_address = bs58::encode(public_key.as_bytes()).into_string();
    eprintln!("  {} Wallet: {}", "✓".green(), wallet_address.cyan());

    let api = get_api(profile)?;

    eprintln!("  {} Preparing top-up transaction...", "→".dimmed());
    let prepared = api.topup_prepare(amount, &wallet_address).await?;

    let unsigned_tx = prepared
        .transaction
        .as_deref()
        .ok_or("Server did not return a transaction to sign")?;

    if !output::is_json() {
        println!();
        println!("  Amount:      {}", format!("${:.2} USDC", amount).bold());
        println!(
            "  Destination: {}",
            prepared.destination_wallet.as_deref().unwrap_or("—")
        );
        println!();

        if !confirm_prompt(&format!("  Send ${:.2} USDC? (y/N) ", amount))? {
            eprintln!("  {}", "Cancelled.".dimmed());
            return Ok(());
        }
    }

    eprintln!("  {} Signing transaction...", "→".dimmed());
    let signed_tx = sign_transaction(unsigned_tx, &signing_key)?;

    eprintln!("  {} Submitting transaction...", "→".dimmed());
    let result = api
        .topup_confirm(&signed_tx, &wallet_address, amount)
        .await?;

    if output::is_json() {
        let json = serde_json::json!({
            "status": "SUCCESS",
            "amount_credited": result.amount_credited,
            "balance": result.balance,
            "invoice_reference": result.invoice_reference,
            "transaction_signature": result.transaction_signature,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        eprintln!("  {} Top-up complete!", "✓".green().bold());
        println!();
        println!(
            "  Credited:  {}",
            format!("${:.2}", result.amount_credited.unwrap_or(0.0))
                .green()
                .bold()
        );
        println!(
            "  Balance:   ${:.2}",
            result.balance.unwrap_or(0.0)
        );
        if let Some(ref sig) = result.transaction_signature {
            println!(
                "  TX:        {}",
                format!("https://solscan.io/tx/{}", sig).cyan()
            );
        }
        println!();
    }

    Ok(())
}


async fn history(profile: Option<&str>) -> CliResult<()> {
    let api = get_api(profile)?;

    eprintln!("  {} Fetching top-up history...", "→".dimmed());
    let data = api.topup_history(1, 15).await?;

    let txns = data
        .transactions
        .as_ref()
        .map(|t| t.data.as_slice())
        .unwrap_or(&[]);

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&data.transactions)?);
        return Ok(());
    }

    if txns.is_empty() {
        println!("  No top-up history found.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Type", "Amount", "Before", "After", "Date"]);

    for tx in txns {
        let type_cell = match tx.tx_type.as_deref() {
            Some("deposit") => Cell::new("deposit").fg(TColor::Green),
            Some("deduction") => Cell::new("deduction").fg(TColor::Red),
            Some(t) => Cell::new(t),
            None => Cell::new("—"),
        };

        table.add_row(vec![
            type_cell,
            Cell::new(format!("${:.2}", tx.amount.unwrap_or(0.0))),
            Cell::new(format!("${:.2}", tx.balance_before.unwrap_or(0.0))),
            Cell::new(format!("${:.2}", tx.balance_after.unwrap_or(0.0))),
            Cell::new(tx.created_at.as_deref().unwrap_or("—")),
        ]);
    }

    println!();
    println!("{table}");
    println!();

    Ok(())
}


async fn purchase(
    slug: &str,
    period: &str,
    coupon: Option<&str>,
    profile: Option<&str>,
) -> CliResult<()> {
    let valid_periods = ["monthly", "quarterly", "semi-annual", "annual"];
    if !valid_periods.contains(&period) {
        return Err(format!(
            "Invalid billing period '{}'. Must be one of: {}",
            period,
            valid_periods.join(", ")
        )
        .into());
    }

    let api = get_api(profile)?;

    eprintln!("  {} Validating order...", "→".dimmed());
    let validation = api.orders_validate(slug, period, coupon).await?;

    let can_afford = validation.can_afford.unwrap_or(false);
    let user_balance = val_to_f64(&validation.user_balance);
    let pricing = validation.pricing.as_ref();
    let final_price = pricing.map(|p| val_to_f64(&p.final_price)).unwrap_or(0.0);

    if output::is_json() && !can_afford {
        let json = serde_json::json!({
            "error": "INSUFFICIENT_BALANCE",
            "user_balance": user_balance,
            "final_price": final_price,
            "shortfall": final_price - user_balance,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Err("Insufficient balance. Top up first with: orbitflare pay topup <amount>".into());
    }

    if !output::is_json() {
        println!();
        println!("  Plan:           {}", slug.cyan());
        println!("  Period:         {}", period);
        if let Some(p) = pricing {
            println!("  Base price:     {}", format_price(&p.base_price));
            if p.period_months.is_some() {
                println!("  Period months:  {}", val_to_f64(&p.period_months) as u64);
            }
            println!("  Period total:   {}", format_price(&p.period_total));
            let disc = val_to_f64(&p.discount_amount);
            if disc > 0.0 {
                println!(
                    "  Discount:       {}",
                    format!("-${:.2}", disc).green()
                );
            }
            println!(
                "  {}",
                format!("Final price:    ${:.2}", final_price).bold()
            );
        }
        println!(
            "  Your balance:   ${:.2}",
            user_balance
        );
        println!();

        if !can_afford {
            println!(
                "  {} Insufficient balance (need ${:.2} more). Run: {}",
                "!".red().bold(),
                final_price - user_balance,
                "orbitflare pay topup <amount>".cyan()
            );
            println!();
            return Ok(());
        }

        if !confirm_prompt("  Confirm purchase? (y/N) ")? {
            eprintln!("  {}", "Cancelled.".dimmed());
            return Ok(());
        }
    }

    eprintln!("  {} Processing purchase...", "→".dimmed());
    let result = api.orders_purchase(slug, period, coupon).await?;

    if output::is_json() {
        let json = serde_json::json!({
            "status": "SUCCESS",
            "order": result.order,
            "license": result.license,
            "invoice": result.invoice,
            "new_balance": result.new_balance,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        eprintln!("  {} Purchase complete!", "✓".green().bold());
        println!();
        if let Some(ref order) = result.order {
            println!(
                "  Order:     {}",
                order.reference.as_deref().unwrap_or("—").cyan()
            );
        }
        if let Some(ref license) = result.license {
            println!(
                "  License:   {}",
                license.key.as_deref().unwrap_or("—").green().bold()
            );
            if let Some(ref renewal) = license.renewal_date {
                println!("  Renews:    {}", renewal);
            }
        }
        println!(
            "  Balance:   ${:.2}",
            result.new_balance.unwrap_or(0.0)
        );
        println!();
    }

    Ok(())
}


async fn order(
    reference: Option<&str>,
    history: bool,
    profile: Option<&str>,
) -> CliResult<()> {
    let api = get_api(profile)?;

    if history || reference.is_none() {
        eprintln!("  {} Fetching orders...", "→".dimmed());
        let data = api.orders_list(1, 15).await?;

        let orders = data
            .orders
            .as_ref()
            .map(|o| o.data.as_slice())
            .unwrap_or(&[]);

        if output::is_json() {
            println!("{}", serde_json::to_string_pretty(&data.orders)?);
            return Ok(());
        }

        if orders.is_empty() {
            println!("  No orders found.");
            return Ok(());
        }

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec!["Reference", "Product", "Amount", "Period", "Status", "Date"]);

        for o in orders {
            let status = o.status.as_deref().unwrap_or("—");
            let status_cell = match status {
                "fulfilled" => Cell::new(status).fg(TColor::Green),
                "pending" => Cell::new(status).fg(TColor::Yellow),
                _ => Cell::new(status),
            };

            table.add_row(vec![
                Cell::new(o.reference.as_deref().unwrap_or("—")),
                Cell::new(o.product_name.as_deref().unwrap_or("—")),
                Cell::new(format!("${:.2}", o.total_amount.unwrap_or(0.0))),
                Cell::new(o.billing_period.as_deref().unwrap_or("—")),
                status_cell,
                Cell::new(o.placed_at.as_deref().unwrap_or("—")),
            ]);
        }

        println!();
        println!("{table}");
        println!();
    } else {
        let reference = reference.unwrap();
        eprintln!("  {} Fetching order '{}'...", "→".dimmed(), reference);
        let o = api.order_detail(reference).await?;

        if output::is_json() {
            println!("{}", serde_json::to_string_pretty(&o)?);
            return Ok(());
        }

        println!();
        println!(
            "  Order: {}",
            o.reference.as_deref().unwrap_or("—").cyan().bold()
        );
        println!("  Status:          {}", o.status.as_deref().unwrap_or("—"));
        println!(
            "  Payment:         {}",
            o.payment_status.as_deref().unwrap_or("—")
        );
        println!(
            "  Amount:          {}",
            format!("${:.2}", o.total_amount.unwrap_or(0.0)).bold()
        );
        println!(
            "  Period:          {}",
            o.billing_period.as_deref().unwrap_or("—")
        );
        if let Some(ref placed) = o.placed_at {
            println!("  Placed:          {}", placed);
        }
        if let Some(ref paid) = o.paid_at {
            println!("  Paid:            {}", paid);
        }
        if let Some(ref invoice) = o.invoice {
            println!();
            println!(
                "  Invoice:         {}",
                invoice.reference.as_deref().unwrap_or("—")
            );
            println!(
                "  Invoice status:  {}",
                invoice.status.as_deref().unwrap_or("—")
            );
        }
        if let Some(ref license) = o.license {
            println!();
            println!(
                "  License key:     {}",
                license.key.as_deref().unwrap_or("—").green()
            );
            println!(
                "  License status:  {}",
                license.status.as_deref().unwrap_or("—")
            );
            if let Some(ref renewal) = license.renewal_date {
                println!("  Renewal date:    {}", renewal);
            }
        }
        println!();
    }

    Ok(())
}


async fn invoice(
    reference: Option<&str>,
    list: bool,
    profile: Option<&str>,
) -> CliResult<()> {
    let api = get_api(profile)?;

    if list || reference.is_none() {
        eprintln!("  {} Fetching invoices...", "→".dimmed());
        let invoices = api.invoices().await?;

        if output::is_json() {
            let json: Vec<_> = invoices
                .iter()
                .map(|inv| {
                    serde_json::json!({
                        "id": inv.id,
                        "reference": inv.reference,
                        "amount": inv.amount,
                        "status": inv.status,
                        "created_at": inv.created_at,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
            return Ok(());
        }

        if invoices.is_empty() {
            println!("  No invoices found.");
            return Ok(());
        }

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec!["Reference", "Amount", "Status", "Created"]);

        for inv in &invoices {
            let status = inv.status.as_deref().unwrap_or("—");
            let status_cell = match status {
                "paid" => Cell::new(status).fg(TColor::Green),
                "pending" | "unpaid" => Cell::new(status).fg(TColor::Yellow),
                _ => Cell::new(status),
            };

            table.add_row(vec![
                Cell::new(inv.reference.as_deref().unwrap_or("—")),
                Cell::new(format!("${:.2}", inv.amount.unwrap_or(0.0))),
                status_cell,
                Cell::new(inv.created_at.as_deref().unwrap_or("—")),
            ]);
        }

        println!();
        println!("{table}");
        println!();
    } else {
        let reference = reference.unwrap();
        eprintln!("  {} Fetching invoice '{}'...", "→".dimmed(), reference);
        let invoices = api.invoices().await?;
        let inv = invoices
            .iter()
            .find(|i| i.reference.as_deref() == Some(reference))
            .ok_or_else(|| format!("Invoice '{}' not found", reference))?;

        if output::is_json() {
            let json = serde_json::json!({
                "id": inv.id,
                "reference": inv.reference,
                "amount": inv.amount,
                "status": inv.status,
                "created_at": inv.created_at,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
            return Ok(());
        }

        println!();
        println!(
            "  Reference: {}",
            inv.reference.as_deref().unwrap_or("—").cyan()
        );
        println!(
            "  Amount:    {}",
            format!("${:.2}", inv.amount.unwrap_or(0.0)).bold()
        );
        println!("  Status:    {}", inv.status.as_deref().unwrap_or("—"));
        println!("  Created:   {}", inv.created_at.as_deref().unwrap_or("—"));
        println!();
    }

    Ok(())
}

async fn renew(invoice_ref: &str, profile: Option<&str>) -> CliResult<()> {
    let api = get_api(profile)?;

    eprintln!(
        "  {} Paying invoice '{}'...",
        "→".dimmed(),
        invoice_ref
    );
    let result = api.pay_invoice_with_balance(invoice_ref).await?;

    if output::is_json() {
        let json = serde_json::json!({
            "status": "SUCCESS",
            "balance_used": result.balance_used,
            "remainder": result.remainder,
            "invoice_status": result.invoice_status,
            "new_user_balance": result.new_user_balance,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        eprintln!("  {} Invoice paid!", "✓".green().bold());
        println!();
        println!(
            "  Balance used:  {}",
            format!("${:.2}", result.balance_used.unwrap_or(0.0)).bold()
        );
        println!(
            "  Invoice:       {}",
            result
                .invoice_status
                .as_deref()
                .unwrap_or("—")
        );
        println!(
            "  New balance:   ${:.2}",
            result.new_user_balance.unwrap_or(0.0)
        );
        if let Some(remainder) = result.remainder {
            if remainder > 0.0 {
                println!(
                    "  Remainder:     {}",
                    format!("${:.2} still due", remainder).yellow()
                );
            }
        }
        println!();
    }

    Ok(())
}
