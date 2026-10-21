use crate::api_client::{DevicePollResult, OrbitApi, UserData};
use crate::cli::AuthCommands;
use crate::config::{AuthProfile, Config};
use crate::error::{CliError, CliResult};
use crate::keyring_store;
use crate::output;
use colored::Colorize;
use comfy_table::{
    Cell, Color as TColor, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};
use std::io::{self, Write};

pub async fn run(cmd: AuthCommands) -> CliResult<()> {
    match cmd {
        AuthCommands::Login {
            api_key,
            wallet,
            profile,
        } => login(api_key, wallet, profile).await,
        AuthCommands::Logout => logout().await,
        AuthCommands::Status => status().await,
        AuthCommands::Switch { profile } => switch(profile),
        AuthCommands::SetLicenseKey { key } => set_license_key(key),
        AuthCommands::SetApexKey { key } => set_apex_key(key),
    }
}

async fn login(
    api_key: Option<String>,
    wallet: Option<String>,
    profile_name: String,
) -> CliResult<()> {
    if api_key.is_some() && wallet.is_some() {
        return Err("Cannot use both --x-orbit-key and --wallet. Choose one.".into());
    }

    if let Some(key) = api_key {
        login_api_key(&key, &profile_name).await
    } else if let Some(keypair_path) = wallet {
        login_wallet(&keypair_path, &profile_name).await
    } else {
        login_device(&profile_name).await
    }
}

async fn login_device(profile_name: &str) -> CliResult<()> {
    let api = OrbitApi::new();

    eprintln!("  {} Requesting device authorization...", "→".dimmed());
    let device = api.device_code().await?;

    println!();
    println!("  {} Open this URL in your browser:", "→".cyan().bold());
    println!("    {}", device.verification_uri.bold());
    println!();
    println!("  {} Enter this code when prompted:", "→".cyan().bold());
    println!("    {}", device.user_code.green().bold());
    println!();

    if open::that(&device.verification_uri).is_ok() {
        eprintln!("  {} Browser opened automatically", "✓".green());
    } else {
        eprintln!(
            "  {} Could not open browser — please copy the URL above",
            "!".yellow()
        );
    }

    eprintln!();
    eprintln!(
        "  Waiting for authorization (expires in {}s)...",
        device.expires_in
    );

    let mut interval = std::time::Duration::from_secs(device.interval.max(2));

    loop {
        tokio::time::sleep(interval).await;

        match api.device_token(&device.device_code).await? {
            DevicePollResult::Success(token_data) => {
                eprintln!();

                let api_with_token = OrbitApi::new();
                let user = api_with_token
                    .me_with_token(&token_data.access_token)
                    .await
                    .ok()
                    .or(token_data.user)
                    .unwrap_or_default();

                save_profile(
                    profile_name,
                    AuthProfile {
                        auth_method: "device".into(),
                        email: user.email.clone().unwrap_or_default(),
                        name: user.name.clone(),
                        wallet_address: None,
                    },
                    None,
                    &token_data.access_token,
                )?;

                let token_api = OrbitApi::with_token(&token_data.access_token);
                fetch_and_store_license_key(&token_api, profile_name).await;

                print_login_success(&user, profile_name);
                return Ok(());
            }
            DevicePollResult::Pending => {
                eprint!(".");
                io::stderr().flush()?;
            }
            DevicePollResult::SlowDown => {
                interval += std::time::Duration::from_secs(2);
            }
            DevicePollResult::Expired => {
                eprintln!();
                return Err(
                    "Device authorization expired. Run `orbitflare auth login` again.".into(),
                );
            }
        }
    }
}

async fn login_api_key(key: &str, profile_name: &str) -> CliResult<()> {
    let api = OrbitApi::with_api_key(Some(key.to_string()));

    eprintln!("  {} Validating API key...", "→".dimmed());

    let user = api.me_with_api_key(key).await?;

    save_profile(
        profile_name,
        AuthProfile {
            auth_method: "api_key".into(),
            email: user.email.clone().unwrap_or_default(),
            name: user.name.clone(),
            wallet_address: None,
        },
        Some(key),
        "",
    )?;

    fetch_and_store_license_key(&api, profile_name).await;

    print_login_success(&user, profile_name);
    Ok(())
}

async fn login_wallet(keypair_path: &str, profile_name: &str) -> CliResult<()> {
    use ed25519_dalek::{Signer, SigningKey};

    let api = OrbitApi::new();

    let expanded = shellexpand::tilde(keypair_path);
    let file_contents = std::fs::read_to_string(expanded.as_ref())
        .map_err(|e| format!("Cannot read keypair file '{}': {}", keypair_path, e))?;

    let keypair_bytes: Vec<u8> = serde_json::from_str(&file_contents).map_err(|e| {
        format!(
            "Invalid keypair format (expected JSON array of bytes): {}",
            e
        )
    })?;

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
    let signing_key = SigningKey::from_bytes(&secret_bytes);
    let public_key = signing_key.verifying_key();
    let wallet_address = bs58::encode(public_key.as_bytes()).into_string();

    eprintln!("  {} Wallet: {}", "→".dimmed(), wallet_address.cyan());

    eprintln!("  {} Requesting challenge...", "→".dimmed());
    let challenge_data = api.wallet_challenge(&wallet_address).await?;

    let signature = signing_key.sign(challenge_data.challenge.as_bytes());
    let sig_bs58 = bs58::encode(signature.to_bytes()).into_string();

    eprintln!("  {} Verifying signature...", "→".dimmed());
    match api.wallet_verify(&wallet_address, &sig_bs58).await {
        Ok(auth_data) => {
            let user = if let Some(u) = auth_data.user {
                u
            } else {
                api.me_with_token(&auth_data.access_token)
                    .await
                    .unwrap_or_default()
            };

            let returned_api_key = auth_data.api_key.and_then(|k| k.key);

            save_profile(
                profile_name,
                AuthProfile {
                    auth_method: "wallet".into(),
                    email: user.email.clone().unwrap_or_default(),
                    name: user.name.clone(),
                    wallet_address: Some(wallet_address),
                },
                returned_api_key.as_deref(),
                &auth_data.access_token,
            )?;

            let authed_api = if let Some(ref key) = returned_api_key {
                OrbitApi::with_api_key(Some(key.clone()))
            } else {
                OrbitApi::with_token(&auth_data.access_token)
            };
            fetch_and_store_license_key(&authed_api, profile_name).await;

            print_login_success(&user, profile_name);
            Ok(())
        }
        Err(CliError::WalletNotLinked) => {
            eprintln!(
                "  {} Wallet not linked to any account. Let's register.",
                "!".yellow()
            );
            register_wallet(
                &api,
                &signing_key,
                &wallet_address,
                &challenge_data.challenge,
                profile_name,
            )
            .await
        }
        Err(e) => Err(e),
    }
}

async fn register_wallet(
    api: &OrbitApi,
    signing_key: &ed25519_dalek::SigningKey,
    wallet_address: &str,
    challenge: &str,
    profile_name: &str,
) -> CliResult<()> {
    use ed25519_dalek::Signer;

    print!("  Name: ");
    io::stdout().flush()?;
    let mut name = String::new();
    io::stdin().read_line(&mut name)?;
    let name = name.trim().to_string();

    print!("  Email: ");
    io::stdout().flush()?;
    let mut email = String::new();
    io::stdin().read_line(&mut email)?;
    let email = email.trim().to_string();

    if name.is_empty() || email.is_empty() || !email.contains('@') {
        return Err("Name and a valid email are required.".into());
    }

    let signature = signing_key.sign(challenge.as_bytes());
    let sig_bs58 = bs58::encode(signature.to_bytes()).into_string();

    eprintln!("  {} Registering wallet...", "→".dimmed());
    let auth_data = api
        .wallet_register(wallet_address, &sig_bs58, &name, &email)
        .await?;

    let stored_api_key = auth_data.api_key.and_then(|k| k.key);

    save_profile(
        profile_name,
        AuthProfile {
            auth_method: "wallet".into(),
            email: email.clone(),
            name: Some(name),
            wallet_address: Some(wallet_address.to_string()),
        },
        stored_api_key.as_deref(),
        &auth_data.access_token,
    )?;

    println!();
    println!(
        "  {} Wallet registered and authenticated!",
        "✓".green().bold()
    );
    Ok(())
}

async fn status() -> CliResult<()> {
    let config = Config::load()?;
    let profile_name = match config.auth.default_profile {
        Some(ref p) => p.clone(),
        None => {
            println!(
                "{}",
                "Not authenticated. Run: orbitflare auth login".yellow()
            );
            return Ok(());
        }
    };

    let profile = match config.auth.profiles.get(&profile_name) {
        Some(p) => p.clone(),
        None => {
            println!(
                "{}",
                "Not authenticated. Run: orbitflare auth login".yellow()
            );
            return Ok(());
        }
    };

    let stored_api_key = keyring_store::get_secret(&profile_name, "api_key");
    let stored_token = keyring_store::get_secret(&profile_name, "token");
    let stored_license_key = keyring_store::get_secret(&profile_name, "license_key");
    let stored_apex_key = keyring_store::get_secret(&profile_name, "apex_key");

    let api = OrbitApi::with_api_key(stored_api_key.clone());

    let user_result = if let Some(ref key) = stored_api_key {
        api.me_with_api_key(key).await
    } else if let Some(ref tok) = stored_token {
        api.me_with_token(tok).await
    } else {
        Err("No stored credentials".into())
    };

    println!();

    match user_result {
        Ok(user) => {
            if output::is_json() {
                let json = serde_json::json!({
                    "authenticated": true,
                    "profile": profile_name,
                    "auth_method": profile.auth_method,
                    "name": user.name,
                    "email": user.email,
                    "balance": user.balance,
                    "wallet": profile.wallet_address,
                    "api_key": stored_api_key.as_deref().map(mask_token),
                    "token": stored_token.as_deref().map(mask_token),
                    "license_key": stored_license_key.as_deref().map(mask_token),
                    "apex_key": stored_apex_key.as_deref().map(mask_token),
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!("  {} Authenticated\n", "✓".green().bold());

                let mut table = Table::new();
                table
                    .load_preset(UTF8_FULL)
                    .apply_modifier(UTF8_ROUND_CORNERS)
                    .set_content_arrangement(ContentArrangement::Dynamic)
                    .set_header(vec!["Field", "Value"]);

                table.add_row(vec![Cell::new("Profile"), Cell::new(&profile_name)]);
                table.add_row(vec![
                    Cell::new("Auth Method"),
                    Cell::new(&profile.auth_method),
                ]);
                if let Some(ref name) = user.name {
                    table.add_row(vec![Cell::new("Name"), Cell::new(name)]);
                }
                if let Some(ref email) = user.email {
                    table.add_row(vec![Cell::new("Email"), Cell::new(email)]);
                }
                if let Some(balance) = user.balance {
                    table.add_row(vec![
                        Cell::new("Balance"),
                        Cell::new(format!("${:.2}", balance)).fg(TColor::Green),
                    ]);
                }
                if let Some(ref addr) = profile.wallet_address {
                    table.add_row(vec![Cell::new("Wallet"), Cell::new(addr)]);
                }
                if let Some(ref key) = stored_api_key {
                    table.add_row(vec![Cell::new("API Key"), Cell::new(mask_token(key))]);
                }
                if let Some(ref tok) = stored_token {
                    table.add_row(vec![Cell::new("Token"), Cell::new(mask_token(tok))]);
                }
                if let Some(ref lk) = stored_license_key {
                    table.add_row(vec![Cell::new("License Key"), Cell::new(mask_token(lk))]);
                }
                if let Some(ref ak) = stored_apex_key {
                    table.add_row(vec![Cell::new("Apex Key"), Cell::new(mask_token(ak))]);
                }

                println!("{table}\n");
            }
        }
        Err(e) => {
            if output::is_json() {
                let json = serde_json::json!({
                    "authenticated": false,
                    "error": e.to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!(
                    "  {} Session expired or invalid: {}",
                    "!".yellow().bold(),
                    e
                );
                println!("  Run: orbitflare auth login\n");
            }
        }
    }

    Ok(())
}

async fn logout() -> CliResult<()> {
    let mut config = Config::load()?;

    for profile_name in config.auth.profiles.keys() {
        if let Some(token) =
            keyring_store::get_secret(profile_name, "token").filter(|t| !t.is_empty())
            && let Err(e) = OrbitApi::with_token(&token).logout().await
        {
            eprintln!(
                "  {} Could not revoke session for '{}': {}",
                "!".yellow(),
                profile_name,
                e
            );
        }
        let _ = keyring_store::delete_profile_secrets(profile_name);
    }

    config.auth.profiles.clear();
    config.auth.default_profile = None;
    config.save()?;
    println!("  {} Credentials removed", "✓".green().bold());
    Ok(())
}

fn switch(profile: String) -> CliResult<()> {
    let mut config = Config::load()?;
    if !config.auth.profiles.contains_key(&profile) {
        let available: Vec<_> = config.auth.profiles.keys().collect();
        return Err(format!(
            "Profile '{}' not found. Available: {:?}",
            profile, available
        )
        .into());
    }
    config.auth.default_profile = Some(profile.clone());
    config.save()?;
    println!("  {} Switched to profile: {}", "✓".green().bold(), profile);
    Ok(())
}

fn set_license_key(key: String) -> CliResult<()> {
    let config = Config::load()?;
    let profile_name = config.auth.default_profile.as_deref().unwrap_or("default");

    keyring_store::store_secret(profile_name, "license_key", &key)?;
    println!(
        "  {} License key stored for profile '{}'",
        "✓".green().bold(),
        profile_name
    );
    Ok(())
}

fn set_apex_key(key: String) -> CliResult<()> {
    let config = Config::load()?;
    let profile_name = config.auth.default_profile.as_deref().unwrap_or("default");

    keyring_store::store_secret(profile_name, "apex_key", &key)?;
    println!(
        "  {} Apex key stored for profile '{}'",
        "✓".green().bold(),
        profile_name
    );
    Ok(())
}

fn save_profile(
    name: &str,
    profile: AuthProfile,
    api_key: Option<&str>,
    token: &str,
) -> CliResult<()> {
    keyring_store::store_profile_secrets(name, api_key, token)?;

    let mut config = Config::load()?;
    config.auth.profiles.insert(name.to_string(), profile);
    config.auth.default_profile = Some(name.to_string());
    config.save()?;
    Ok(())
}

async fn fetch_and_store_license_key(api: &OrbitApi, profile_name: &str) {
    match api.licenses().await {
        Ok(licenses) => {
            let active = licenses
                .iter()
                .find(|l| l.status.as_deref() == Some("active"))
                .or_else(|| licenses.first());
            if let Some(license) = active {
                if let Some(ref key) = license.license_key {
                    if let Err(e) = keyring_store::store_secret(profile_name, "license_key", key) {
                        eprintln!("  {} Could not store license key: {}", "!".yellow(), e);
                    } else {
                        eprintln!(
                            "  {} License key stored ({})",
                            "✓".green(),
                            license.product_name.as_deref().unwrap_or("default")
                        );
                    }
                }
            } else {
                eprintln!(
                    "  {} No licenses found. RPC calls will use fallback endpoints.",
                    "!".yellow()
                );
            }
        }
        Err(e) => {
            eprintln!("  {} Could not fetch licenses: {}", "!".yellow(), e);
        }
    }
}

fn print_login_success(user: &UserData, profile_name: &str) {
    println!();
    println!("  {} Authenticated", "✓".green().bold());
    if let Some(ref name) = user.name {
        println!("  Name:     {}", name);
    }
    if let Some(ref email) = user.email {
        println!("  Email:    {}", email);
    }
    println!("  Profile:  {}", profile_name);
    println!();
}

fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        return "****".to_string();
    }
    let prefix = &token[..std::cmp::min(7, token.len())];
    let suffix = &token[token.len().saturating_sub(4)..];
    format!("{}...{}", prefix, suffix)
}
