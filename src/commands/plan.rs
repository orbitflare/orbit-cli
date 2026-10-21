use crate::api_client::{OrbitApi, RpcPlan};
use crate::cli::PlanCommands;
use crate::config::Config;
use crate::error::CliResult;
use crate::keyring_store;
use crate::output;
use colored::Colorize;
use comfy_table::{
    Cell, Color as TColor, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub async fn run(cmd: PlanCommands, profile: Option<&str>) -> CliResult<()> {
    match cmd {
        PlanCommands::List => list(profile).await,
        PlanCommands::View { slug } => view(&slug, profile).await,
        PlanCommands::Compare { plans, all } => compare(plans, all, profile).await,
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

fn val_display(value: &Option<serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::Bool(b)) => if *b { "Yes" } else { "No" }.to_string(),
        Some(v) => v.to_string(),
        None => "—".to_string(),
    }
}

fn titlecase(s: &str) -> String {
    s.replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn collect_object_keys<F>(plans: &[&RpcPlan], get_obj: F) -> Vec<String>
where
    F: Fn(&RpcPlan) -> Option<&serde_json::Value>,
{
    let mut keys: Vec<String> = Vec::new();
    for p in plans {
        if let Some(serde_json::Value::Object(map)) = get_obj(p) {
            for key in map.keys() {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
        }
    }
    keys.sort();
    keys
}

fn feature_key(feature: &str) -> String {
    feature
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == ',')
        .trim()
        .to_lowercase()
}

const CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);

fn cache_path() -> PathBuf {
    Config::dir().join("cache").join("rpc-plans.json")
}

fn read_cache() -> Option<Vec<RpcPlan>> {
    let path = cache_path();
    let metadata = std::fs::metadata(&path).ok()?;
    let modified = metadata.modified().ok()?;
    if SystemTime::now()
        .duration_since(modified)
        .unwrap_or(CACHE_TTL)
        >= CACHE_TTL
    {
        return None;
    }
    let contents = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn write_cache(plans: &[RpcPlan]) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(plans) {
        let _ = std::fs::write(&path, json);
    }
}

async fn fetch_plans(profile: Option<&str>) -> CliResult<Vec<RpcPlan>> {
    if let Some(cached) = read_cache() {
        return Ok(cached);
    }
    eprintln!("  {} Fetching plans...", "→".dimmed());
    let plans = match get_api(profile) {
        Ok(api) => api.rpc_plans().await?,
        Err(_) => crate::api_client::public_rpc_plans().await?,
    };
    write_cache(&plans);
    Ok(plans)
}

fn sort_plans(plans: &mut [RpcPlan]) {
    plans.sort_by(|a, b| {
        val_to_f64(&a.base_price)
            .partial_cmp(&val_to_f64(&b.base_price))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

async fn list(profile: Option<&str>) -> CliResult<()> {
    let mut plans = fetch_plans(profile).await?;
    sort_plans(&mut plans);

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&plans)?);
        return Ok(());
    }

    if plans.is_empty() {
        println!("  No plans available.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Name", "Slug", "Base Price", "Type"]);

    for p in &plans {
        table.add_row(vec![
            Cell::new(p.name.as_deref().unwrap_or("—")),
            Cell::new(p.slug.as_deref().unwrap_or("—")).fg(TColor::Cyan),
            Cell::new(format_price(&p.base_price)),
            Cell::new(p.price_type.as_deref().unwrap_or("—")),
        ]);
    }

    println!();
    println!("{table}");
    println!();
    println!(
        "  Use {} for details or {} to compare",
        "orbitflare plan view <slug>".cyan(),
        "orbitflare plan compare --all".cyan()
    );
    println!();

    Ok(())
}

async fn view(slug: &str, profile: Option<&str>) -> CliResult<()> {
    let plans = fetch_plans(profile).await?;
    let p = plans
        .into_iter()
        .find(|p| p.slug.as_deref() == Some(slug))
        .ok_or_else(|| format!("Plan '{}' not found", slug))?;

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&p)?);
        return Ok(());
    }

    println!();
    println!("  {}", p.name.as_deref().unwrap_or("—").bold());
    let desc = p.description.as_deref().unwrap_or("");
    if !desc.is_empty() && desc != p.name.as_deref().unwrap_or("") {
        println!("  {}", desc.dimmed());
    }
    println!();
    println!("  Slug:       {}", p.slug.as_deref().unwrap_or("—").cyan());
    println!("  Base Price: {}", format_price(&p.base_price).green());
    println!("  Type:       {}", p.price_type.as_deref().unwrap_or("—"));

    if !p.features.is_empty() {
        println!();
        println!("  Features:");
        for f in &p.features {
            println!("    - {}", f.trim());
        }
    }

    if let Some(serde_json::Value::Object(ref map)) = p.pricing
        && !map.is_empty()
    {
        println!();
        println!("  Pricing:");
        for (k, v) in map {
            let label = k.replace('_', " ");
            println!("    {:<20} {}", label, val_display(&Some(v.clone())));
        }
    }

    if let Some(serde_json::Value::Object(ref map)) = p.limits
        && !map.is_empty()
    {
        println!();
        println!("  Limits:");
        for (k, v) in map {
            let label = k.replace('_', " ");
            let display = val_display(&Some(v.clone()));
            let colored = match display.as_str() {
                "Yes" | "true" => display.green().to_string(),
                "No" | "false" => display.red().to_string(),
                _ => display,
            };
            println!("    {:<20} {}", label, colored);
        }
    }

    println!();

    Ok(())
}

async fn compare(plan_slugs: Vec<String>, all: bool, profile: Option<&str>) -> CliResult<()> {
    let mut all_plans = fetch_plans(profile).await?;
    sort_plans(&mut all_plans);

    let plans: Vec<&RpcPlan> = if all || plan_slugs.is_empty() {
        all_plans.iter().collect()
    } else {
        let mut selected = Vec::new();
        for slug in &plan_slugs {
            match all_plans
                .iter()
                .find(|p| p.slug.as_deref() == Some(slug.as_str()))
            {
                Some(p) => selected.push(p),
                None => return Err(format!("Plan '{}' not found", slug).into()),
            }
        }
        selected
    };

    if plans.is_empty() {
        println!("  No plans to compare.");
        return Ok(());
    }

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&plans)?);
        return Ok(());
    }

    let mut header: Vec<Cell> = vec![Cell::new("")];
    for p in &plans {
        header.push(Cell::new(p.name.as_deref().unwrap_or("—")).fg(TColor::Cyan));
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(header);

    let mut row = vec![Cell::new("Price/mo").fg(TColor::White)];
    for p in &plans {
        row.push(Cell::new(format_price(&p.base_price)).fg(TColor::Green));
    }
    table.add_row(row);

    let mut row = vec![Cell::new("Type").fg(TColor::White)];
    for p in &plans {
        row.push(Cell::new(p.price_type.as_deref().unwrap_or("—")));
    }
    table.add_row(row);

    let limit_keys = collect_object_keys(&plans, |p| p.limits.as_ref());
    for key in &limit_keys {
        let label = titlecase(key);
        let mut row = vec![Cell::new(&label).fg(TColor::White)];
        for p in &plans {
            let val = p.limits.as_ref().and_then(|l| l.get(key)).cloned();
            let display = val_display(&val);
            let cell = match display.as_str() {
                "true" => Cell::new("Yes").fg(TColor::Green),
                "false" => Cell::new("No").fg(TColor::Red),
                _ => Cell::new(&display),
            };
            row.push(cell);
        }
        table.add_row(row);
    }

    let pricing_keys = collect_object_keys(&plans, |p| p.pricing.as_ref());
    for key in &pricing_keys {
        let label = titlecase(key);
        let mut row = vec![Cell::new(&label).fg(TColor::White)];
        for p in &plans {
            let val = p.pricing.as_ref().and_then(|l| l.get(key)).cloned();
            row.push(Cell::new(val_display(&val)));
        }
        table.add_row(row);
    }

    let mut unique_features: Vec<String> = Vec::new();
    for p in &plans {
        for f in &p.features {
            let trimmed = f.trim().to_string();
            if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }
            let key = feature_key(&trimmed);
            if !unique_features.iter().any(|u| feature_key(u) == key) {
                unique_features.push(trimmed);
            }
        }
    }

    for feature in &unique_features {
        let key = feature_key(feature);
        let mut row = vec![Cell::new(feature).fg(TColor::White)];
        for p in &plans {
            let has = p.features.iter().any(|f| feature_key(f.trim()) == key);
            if has {
                row.push(Cell::new("Yes").fg(TColor::Green));
            } else {
                row.push(Cell::new("No").fg(TColor::Red));
            }
        }
        table.add_row(row);
    }

    println!();
    println!("{table}");
    println!();

    Ok(())
}
