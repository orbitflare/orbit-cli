use crate::cli::DasCommands;
use crate::error::CliResult;
use crate::output;
use colored::Colorize;
use comfy_table::{
    Cell, Color as TColor, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};
use orbitflare_sdk::RpcClient;
use orbitflare_sdk::das::{
    Asset, AssetList, DisplayOptions, GetAssetsByGroup, GetAssetsByOwner, GetSignaturesForAsset,
    GetTokenAccounts, Interface, SearchAssets, TokenType,
};
use serde::Serialize;

pub async fn run(cmd: DasCommands, rpc: &RpcClient) -> CliResult<()> {
    match cmd {
        DasCommands::Asset { ids } => asset(&ids, rpc).await,
        DasCommands::Owner {
            address,
            tokens,
            nfts,
            limit,
            page,
        } => owner(&address, tokens, nfts, limit, page, rpc).await,
        DasCommands::Search {
            owner,
            creator,
            authority,
            collection,
            name,
            token_type,
            compressed,
            burnt,
            limit,
            page,
        } => {
            if owner.is_none() && creator.is_none() && authority.is_none() && collection.is_none() {
                return Err(
                    "Pass at least one of --owner, --creator, --authority or --collection".into(),
                );
            }
            let mut req = SearchAssets::new().limit(limit.min(1000)).page(page);
            if let Some(ref o) = owner {
                req = req.owner(o);
            }
            if let Some(ref c) = creator {
                req = req.creator(c, false);
            }
            if let Some(ref a) = authority {
                req = req.authority(a);
            }
            if let Some(ref c) = collection {
                req = req.collection(c);
            }
            if let Some(ref n) = name {
                req = req.name(n);
            }
            if let Some(ref t) = token_type {
                req = req.token_type(parse_token_type(t)?);
            }
            if let Some(c) = compressed {
                req = req.compressed(c);
            }
            if let Some(b) = burnt {
                req = req.burnt(b);
            }
            let list = rpc.search_assets(&req).await?;
            print_asset_list("Search Results", &list)
        }
        DasCommands::Collection {
            address,
            limit,
            page,
        } => {
            let req = GetAssetsByGroup::collection(&address)
                .limit(limit.min(1000))
                .page(page);
            let list = rpc.get_assets_by_group(&req).await?;
            print_asset_list(&format!("Collection {}", truncate_addr(&address)), &list)
        }
        DasCommands::Holders { mint, limit, page } => holders(&mint, limit, page, rpc).await,
        DasCommands::Proof { id } => proof(&id, rpc).await,
        DasCommands::Signatures { id, limit, page } => signatures(&id, limit, page, rpc).await,
    }
}

fn parse_token_type(s: &str) -> CliResult<TokenType> {
    match s.to_lowercase().as_str() {
        "fungible" => Ok(TokenType::Fungible),
        "nonfungible" | "non-fungible" => Ok(TokenType::NonFungible),
        "regularnft" | "nft" => Ok(TokenType::RegularNft),
        "compressednft" | "cnft" => Ok(TokenType::CompressedNft),
        "all" => Ok(TokenType::All),
        _ => Err(format!(
            "Unknown token type '{}'. Use fungible, nonFungible, regularNFT, compressedNFT or all",
            s
        )
        .into()),
    }
}

fn print_json<T: Serialize>(value: &T) -> CliResult<bool> {
    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(value)?);
        return Ok(true);
    }
    if output::is_raw() {
        println!("{}", serde_json::to_string(value)?);
        return Ok(true);
    }
    Ok(false)
}

fn new_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

fn interface_name(interface: Interface) -> String {
    serde_json::to_value(interface)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "Unknown".into())
}

fn is_fungible(asset: &Asset) -> bool {
    matches!(
        asset.interface,
        Interface::FungibleToken | Interface::FungibleAsset
    )
}

fn ui_amount(raw: u64, decimals: u8) -> f64 {
    raw as f64 / 10f64.powi(decimals as i32)
}

fn format_amount(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else if value >= 1.0 {
        let s = format!("{:.4}", value);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        let s = format!("{:.9}", value);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn format_usd(value: f64) -> String {
    if value >= 0.01 || value == 0.0 {
        format!("${:.2}", value)
    } else {
        format!("${:.6}", value)
    }
}

fn format_unit_price(price: f64) -> String {
    if price >= 1.0 {
        format!("${:.2}", price)
    } else if price >= 0.01 {
        format!("${:.4}", price)
    } else {
        format!("${:.8}", price)
    }
}

fn token_value(asset: &Asset) -> Option<f64> {
    let info = asset.token_info.as_ref()?;
    let price = info.price_info.as_ref()?;
    price.total_price.or_else(|| {
        let balance = ui_amount(info.balance?, info.decimals.unwrap_or(0));
        Some(balance * price.price_per_token)
    })
}

fn asset_label(asset: &Asset) -> String {
    match (asset.name(), asset.symbol()) {
        (Some(n), Some(s)) => format!("{} ({})", n, s),
        (Some(n), None) => n.to_string(),
        (None, Some(s)) => s.to_string(),
        (None, None) => "—".into(),
    }
}

async fn asset(ids: &[String], rpc: &RpcClient) -> CliResult<()> {
    if ids.len() > 1 {
        let refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
        let assets = rpc.get_asset_batch(&refs).await?;
        if print_json(&assets)? {
            return Ok(());
        }
        println!("\n  {} Assets\n", "●".cyan().bold());
        let mut table = new_table();
        table.set_header(vec!["ID", "Name", "Interface", "Owner"]);
        for (id, a) in ids.iter().zip(assets.iter()) {
            match a {
                Some(a) => table.add_row(vec![
                    Cell::new(&a.id).fg(TColor::Cyan),
                    Cell::new(asset_label(a)),
                    Cell::new(interface_name(a.interface)),
                    Cell::new(a.owner().map(truncate_addr).unwrap_or_else(|| "—".into())),
                ]),
                None => table.add_row(vec![
                    Cell::new(id),
                    Cell::new("not found").fg(TColor::Red),
                    Cell::new("—"),
                    Cell::new("—"),
                ]),
            };
        }
        println!("{table}\n");
        return Ok(());
    }

    let a = rpc.get_asset(&ids[0]).await?;
    if print_json(&a)? {
        return Ok(());
    }

    println!("\n  {} Asset\n", "●".cyan().bold());
    let mut table = new_table();
    table.set_header(vec!["Field", "Value"]);
    table.add_row(vec![Cell::new("ID"), Cell::new(&a.id).fg(TColor::Cyan)]);
    table.add_row(vec![Cell::new("Name"), Cell::new(asset_label(&a))]);
    table.add_row(vec![
        Cell::new("Interface"),
        Cell::new(interface_name(a.interface)),
    ]);
    if let Some(owner) = a.owner() {
        table.add_row(vec![Cell::new("Owner"), Cell::new(owner)]);
    }
    if let Some(collection) = a.collection() {
        table.add_row(vec![Cell::new("Collection"), Cell::new(collection)]);
    }
    if let Some(ref info) = a.token_info {
        let decimals = info.decimals.unwrap_or(0);
        if let Some(supply) = info.supply {
            table.add_row(vec![
                Cell::new("Supply"),
                Cell::new(format_amount(ui_amount(supply, decimals))),
            ]);
        }
        table.add_row(vec![Cell::new("Decimals"), Cell::new(decimals)]);
        if let Some(ref price) = info.price_info {
            table.add_row(vec![
                Cell::new("Price"),
                Cell::new(format!(
                    "{} {}",
                    format_unit_price(price.price_per_token),
                    price.currency
                ))
                .fg(TColor::Green),
            ]);
        }
        if let Some(ref program) = info.token_program {
            table.add_row(vec![Cell::new("Token Program"), Cell::new(program)]);
        }
        if let Some(ref auth) = info.mint_authority {
            table.add_row(vec![Cell::new("Mint Authority"), Cell::new(auth)]);
        }
        if let Some(ref auth) = info.freeze_authority {
            table.add_row(vec![Cell::new("Freeze Authority"), Cell::new(auth)]);
        }
    }
    if let Some(ref c) = a.compression
        && c.compressed
    {
        table.add_row(vec![Cell::new("Compressed"), Cell::new("yes")]);
        table.add_row(vec![Cell::new("Tree"), Cell::new(&c.tree)]);
        table.add_row(vec![Cell::new("Leaf"), Cell::new(c.leaf_id)]);
    }
    if let Some(ref r) = a.royalty
        && r.basis_points > 0
    {
        table.add_row(vec![
            Cell::new("Royalty"),
            Cell::new(format!("{:.2}%", r.basis_points as f64 / 100.0)),
        ]);
    }
    if !a.creators.is_empty() {
        let creators: Vec<String> = a
            .creators
            .iter()
            .map(|c| {
                format!(
                    "{} {}%{}",
                    c.address,
                    c.share,
                    if c.verified { " ✓" } else { "" }
                )
            })
            .collect();
        table.add_row(vec![Cell::new("Creators"), Cell::new(creators.join("\n"))]);
    }
    table.add_row(vec![
        Cell::new("Mutable"),
        Cell::new(if a.mutable { "yes" } else { "no" }),
    ]);
    if a.burnt {
        table.add_row(vec![Cell::new("Burnt"), Cell::new("yes").fg(TColor::Red)]);
    }
    if let Some(ref content) = a.content
        && !content.json_uri.is_empty()
    {
        table.add_row(vec![
            Cell::new("Metadata URI"),
            Cell::new(&content.json_uri),
        ]);
    }
    if let Some(image) = a.image() {
        table.add_row(vec![Cell::new("Image"), Cell::new(image)]);
    }
    println!("{table}\n");
    Ok(())
}

async fn owner(
    address: &str,
    tokens_only: bool,
    nfts_only: bool,
    limit: u32,
    page: u32,
    rpc: &RpcClient,
) -> CliResult<()> {
    let limit = limit.min(1000);
    let options = DisplayOptions::new()
        .show_fungible(!nfts_only)
        .show_native_balance(!nfts_only);
    let list = if tokens_only || nfts_only {
        let token_type = if tokens_only {
            TokenType::Fungible
        } else {
            TokenType::NonFungible
        };
        let mut req = SearchAssets::new()
            .owner(address)
            .token_type(token_type)
            .limit(limit)
            .page(page);
        if !options.is_empty() {
            req = req.options(options);
        }
        rpc.search_assets(&req).await?
    } else {
        let req = GetAssetsByOwner::new(address)
            .options(options)
            .limit(limit)
            .page(page);
        rpc.get_assets_by_owner(&req).await?
    };

    if print_json(&list)? {
        return Ok(());
    }

    println!(
        "\n  {} Portfolio of {}\n",
        "●".cyan().bold(),
        truncate_addr(address)
    );

    let mut total_usd = 0.0;
    if let Some(ref native) = list.native_balance {
        let sol = native.lamports as f64 / 1_000_000_000.0;
        let value = native.total_price;
        total_usd += value.unwrap_or(0.0);
        println!(
            "  SOL: {}{}\n",
            format_amount(sol).green().bold(),
            value
                .map(|v| format!(" ({})", format_usd(v)))
                .unwrap_or_default()
        );
    }

    let (fungible, other): (Vec<&Asset>, Vec<&Asset>) =
        list.items.iter().partition(|a| is_fungible(a));

    if !fungible.is_empty() {
        let mut rows: Vec<(&Asset, Option<f64>)> =
            fungible.iter().map(|a| (*a, token_value(a))).collect();
        rows.sort_by(|a, b| {
            b.1.unwrap_or(0.0)
                .partial_cmp(&a.1.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut table = new_table();
        table.set_header(vec!["Token", "Mint", "Balance", "Price", "Value"]);
        for (a, value) in rows {
            let info = a.token_info.as_ref();
            let decimals = info.and_then(|i| i.decimals).unwrap_or(0);
            let balance = info
                .and_then(|i| i.balance)
                .map(|b| format_amount(ui_amount(b, decimals)))
                .unwrap_or_else(|| "—".into());
            let price = info
                .and_then(|i| i.price_info.as_ref())
                .map(|p| format_unit_price(p.price_per_token))
                .unwrap_or_else(|| "—".into());
            total_usd += value.unwrap_or(0.0);
            let symbol = a
                .symbol()
                .or_else(|| info.and_then(|i| i.symbol.as_deref()))
                .or_else(|| a.name())
                .unwrap_or("—");
            table.add_row(vec![
                Cell::new(symbol).fg(TColor::Cyan),
                Cell::new(&a.id),
                Cell::new(balance),
                Cell::new(price),
                Cell::new(value.map(format_usd).unwrap_or_else(|| "—".into())).fg(TColor::Green),
            ]);
        }
        println!("  {} Tokens", "●".cyan().bold());
        println!("{table}\n");
    }

    if !other.is_empty() {
        let mut table = new_table();
        table.set_header(vec!["Name", "ID", "Collection", "Type"]);
        for a in &other {
            table.add_row(vec![
                Cell::new(asset_label(a)),
                Cell::new(&a.id),
                Cell::new(
                    a.collection()
                        .map(truncate_addr)
                        .unwrap_or_else(|| "—".into()),
                ),
                Cell::new(if a.is_compressed() {
                    format!("{} (compressed)", interface_name(a.interface))
                } else {
                    interface_name(a.interface)
                }),
            ]);
        }
        println!("  {} NFTs and other assets", "●".cyan().bold());
        println!("{table}\n");
    }

    if list.items.is_empty() {
        println!("  No assets on this page.\n");
    }
    if total_usd > 0.0 {
        println!(
            "  {} Priced value: {}",
            "✓".green().bold(),
            format_usd(total_usd).bold()
        );
    }
    print_page_hint(&list, limit, page);
    Ok(())
}

fn print_page_hint(list: &AssetList, limit: u32, page: u32) {
    println!(
        "  {} {} item(s) on page {}{}\n",
        "→".dimmed(),
        list.items.len(),
        page,
        if list.items.len() as u32 >= limit {
            format!(" (more with --page {})", page + 1)
        } else {
            String::new()
        }
    );
}

fn print_asset_list(title: &str, list: &AssetList) -> CliResult<()> {
    if print_json(list)? {
        return Ok(());
    }
    println!("\n  {} {}\n", "●".cyan().bold(), title);
    if list.items.is_empty() {
        println!("  No assets found.\n");
        return Ok(());
    }
    let mut table = new_table();
    table.set_header(vec!["Name", "ID", "Interface", "Owner"]);
    for a in &list.items {
        let interface = if a.is_compressed() {
            format!("{} (compressed)", interface_name(a.interface))
        } else {
            interface_name(a.interface)
        };
        table.add_row(vec![
            Cell::new(asset_label(a)),
            Cell::new(&a.id).fg(TColor::Cyan),
            Cell::new(interface),
            Cell::new(a.owner().map(truncate_addr).unwrap_or_else(|| "—".into())),
        ]);
    }
    println!("{table}");
    print_page_hint(list, list.limit, list.page.unwrap_or(1));
    Ok(())
}

async fn holders(mint: &str, limit: u32, page: u32, rpc: &RpcClient) -> CliResult<()> {
    let limit = limit.min(1000);
    let req = GetTokenAccounts::by_mint(mint).limit(limit).page(page);
    let list = rpc.get_token_accounts(&req).await?;
    if print_json(&list)? {
        return Ok(());
    }

    let token = rpc.get_asset(mint).await.ok();
    let decimals = token
        .as_ref()
        .and_then(|a| a.token_info.as_ref())
        .and_then(|i| i.decimals)
        .unwrap_or(0);
    let label = token
        .as_ref()
        .map(asset_label)
        .unwrap_or_else(|| truncate_addr(mint));

    println!("\n  {} Token accounts for {}\n", "●".cyan().bold(), label);
    if list.token_accounts.is_empty() {
        println!("  No token accounts found.\n");
        return Ok(());
    }

    let mut table = new_table();
    table.set_header(vec!["#", "Owner", "Token Account", "Amount"]);
    let offset = (page.saturating_sub(1) * limit) as usize;
    for (i, acct) in list.token_accounts.iter().enumerate() {
        table.add_row(vec![
            Cell::new(offset + i + 1),
            Cell::new(&acct.owner).fg(TColor::Cyan),
            Cell::new(&acct.address),
            Cell::new(format_amount(ui_amount(acct.amount, decimals))).fg(TColor::Green),
        ]);
    }
    println!("{table}");
    println!(
        "  {} {} account(s) on page {}{}\n",
        "→".dimmed(),
        list.token_accounts.len(),
        page,
        if list.token_accounts.len() as u32 >= limit {
            format!(" (more with --page {})", page + 1)
        } else {
            String::new()
        }
    );
    Ok(())
}

async fn proof(id: &str, rpc: &RpcClient) -> CliResult<()> {
    let p = rpc.get_asset_proof(id).await?;
    if print_json(&p)? {
        return Ok(());
    }
    println!("\n  {} Asset Proof\n", "●".cyan().bold());
    let mut table = new_table();
    table.set_header(vec!["Field", "Value"]);
    table.add_row(vec![Cell::new("Asset"), Cell::new(id).fg(TColor::Cyan)]);
    table.add_row(vec![Cell::new("Tree"), Cell::new(&p.tree_id)]);
    table.add_row(vec![Cell::new("Root"), Cell::new(&p.root)]);
    table.add_row(vec![Cell::new("Leaf"), Cell::new(&p.leaf)]);
    table.add_row(vec![Cell::new("Node Index"), Cell::new(p.node_index)]);
    table.add_row(vec![
        Cell::new("Proof"),
        Cell::new(format!("{} node(s)\n{}", p.proof.len(), p.proof.join("\n"))),
    ]);
    println!("{table}\n");
    Ok(())
}

async fn signatures(id: &str, limit: u32, page: u32, rpc: &RpcClient) -> CliResult<()> {
    let req = GetSignaturesForAsset::new(id)
        .limit(limit.min(1000))
        .page(page);
    let list = rpc.get_signatures_for_asset(&req).await?;
    if print_json(&list)? {
        return Ok(());
    }
    println!(
        "\n  {} Transactions for {}\n",
        "●".cyan().bold(),
        truncate_addr(id)
    );
    if list.items.is_empty() {
        println!("  No transactions found.\n");
        return Ok(());
    }
    let mut table = new_table();
    table.set_header(vec!["Instruction", "Signature"]);
    for s in &list.items {
        table.add_row(vec![
            Cell::new(&s.instruction).fg(TColor::Yellow),
            Cell::new(&s.signature),
        ]);
    }
    println!("{table}\n");
    Ok(())
}

fn truncate_addr(addr: &str) -> String {
    if addr.len() > 16 {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_token_types() {
        assert_eq!(parse_token_type("fungible").unwrap(), TokenType::Fungible);
        assert_eq!(
            parse_token_type("nonFungible").unwrap(),
            TokenType::NonFungible
        );
        assert_eq!(
            parse_token_type("compressedNFT").unwrap(),
            TokenType::CompressedNft
        );
        assert!(parse_token_type("coins").is_err());
    }

    #[test]
    fn formats_token_amounts() {
        assert_eq!(ui_amount(1_500_000, 6), 1.5);
        assert_eq!(format_amount(1.5), "1.5");
        assert_eq!(format_amount(0.000001), "0.000001");
        assert_eq!(format_amount(0.0), "0");
        assert_eq!(format_usd(12.345), "$12.35");
        assert_eq!(format_usd(0.0012346), "$0.001235");
        assert_eq!(format_unit_price(0.3375), "$0.3375");
        assert_eq!(format_unit_price(0.00000323), "$0.00000323");
    }

    #[test]
    fn values_tokens_from_price_info() {
        let asset: Asset = serde_json::from_value(serde_json::json!({
            "interface": "FungibleToken",
            "id": "mint",
            "token_info": {
                "balance": 2_500_000u64,
                "decimals": 6,
                "price_info": { "price_per_token": 2.0, "currency": "USDC" }
            }
        }))
        .unwrap();
        assert!(is_fungible(&asset));
        assert_eq!(token_value(&asset), Some(5.0));
    }
}
