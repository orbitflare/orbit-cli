mod app;
mod event;
mod models;
pub mod theme;
mod ui;

use std::time::Duration;

use ratatui::{
    crossterm::event::KeyEventKind,
    DefaultTerminal,
};

use crate::api_client::OrbitApi;
use crate::config::Config;
use crate::error::CliResult;
use crate::keyring_store;
use app::{App, DashboardData};
use event::{Event, EventHandler};
use models::{ApiKeyItem, ConnectedAccountItem, Endpoint, IpEntry, Invoice, InvoiceStatus, Location, Service, ServiceInfo, ServiceStatus};
pub async fn run() -> CliResult<()> {
    eprintln!("  Loading dashboard data...");

    let config = Config::load().unwrap_or_default();
    let profile_name = config.auth.default_profile.as_deref().unwrap_or("default");
    let api_key = keyring_store::get_secret(profile_name, "api_key");
    let token = keyring_store::get_secret(profile_name, "token");
    let api = if let Some(ref key) = api_key {
        OrbitApi::with_api_key(Some(key.clone()))
    } else if let Some(ref tok) = token {
        OrbitApi::with_token(tok)
    } else {
        OrbitApi::new()
    };
    let initial_theme = theme::ThemeName::from_str(&config.preferences.theme);

    let data = fetch_dashboard_data(&api, api_key.as_deref(), token.as_deref(), initial_theme).await;

    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, data, &api).await;
    ratatui::restore();

    if let Ok(mut cfg) = Config::load() {
        cfg.preferences.theme = result.as_ref().map_or_else(|_| "ocean".into(), |t| t.label().to_lowercase());
        let _ = cfg.save();
    }

    result.map(|_| ())
}

async fn fetch_dashboard_data(api: &OrbitApi, api_key: Option<&str>, token: Option<&str>, initial_theme: theme::ThemeName) -> DashboardData {
    let user = if let Some(key) = api_key {
        api.me_with_api_key(key).await.ok()
    } else if let Some(tok) = token {
        api.me_with_token(tok).await.ok()
    } else {
        None
    };

    let user_name = user.as_ref().and_then(|u| u.name.clone());
    let user_email = user.as_ref().and_then(|u| u.email.clone());
    let user_wallet = user.as_ref().and_then(|u| u.wallet_address.clone());
    let user_id = user.as_ref().and_then(|u| u.id);
    let payment_wallet = user.as_ref().and_then(|u| u.payment_wallet_address.clone());
    let user_created_at = user.as_ref().and_then(|u| u.created_at.clone());
    let fallback_balance = user.as_ref().and_then(|u| u.balance).unwrap_or(0.0);

    let profile = api.profile().await.ok();

    let mut connected_accounts: Vec<ConnectedAccountItem> = Vec::new();
    if let Some(ref ca) = profile.as_ref().and_then(|p| p.connected_accounts.clone()) {
        connected_accounts.push(ConnectedAccountItem {
            provider: "Discord".into(),
            connected: ca.discord.unwrap_or(false),
        });
        connected_accounts.push(ConnectedAccountItem {
            provider: "GitHub".into(),
            connected: ca.github.unwrap_or(false),
        });
    }

    let user_name = profile.as_ref().and_then(|p| p.name.clone()).or(user_name);
    let user_email = profile.as_ref().and_then(|p| p.email.clone()).or(user_email);
    let user_wallet = profile.as_ref().and_then(|p| p.wallet_address.clone()).or(user_wallet);
    let user_id = profile.as_ref().and_then(|p| p.id).or(user_id);
    let payment_wallet = profile.as_ref().and_then(|p| p.payment_wallet_address.clone()).or(payment_wallet);
    let user_created_at = profile.as_ref().and_then(|p| p.created_at.clone()).or(user_created_at);
    let profile_balance = profile.as_ref().and_then(|p| p.balance);

    let balance_data = api.balance().await.ok();
    let balance = balance_data.as_ref()
        .and_then(|b| b.balance)
        .or(profile_balance)
        .unwrap_or(fallback_balance);
    let currency = balance_data.as_ref()
        .and_then(|b| b.currency.clone())
        .unwrap_or_else(|| "USD".into());

    let licenses = api.licenses().await.unwrap_or_default();

    let service_info = licenses.first().map(|lic| ServiceInfo {
        key_name: lic.product_name.clone().unwrap_or_else(|| "License".into()),
        license_key: lic.license_key.clone().unwrap_or_else(|| "—".into()),
        issue_date: format_date(lic.issued_date.as_deref()),
        renewal_date: format_date(lic.renewal_date.as_deref()),
    });

    let mut services: Vec<Service> = Vec::new();
    for lic in &licenses {
        let license_key = lic.license_key.clone().unwrap_or_default();
        let detail = api.license_detail(&license_key).await.ok().flatten();

        let status = match lic.status.as_deref() {
            Some("active") => ServiceStatus::Active,
            Some("expired") => ServiceStatus::Expired,
            Some("suspended") => ServiceStatus::Suspended,
            _ => ServiceStatus::Active,
        };

        if let Some(d) = detail {
            let locations: Vec<Location> = d.allowed_locations.iter().map(|loc| {
                build_location(loc, &d.whitelisted_ips)
            }).collect();

            let detail_key = d.license_key.clone().unwrap_or(license_key.clone());
            let detail_status = match d.status.as_deref() {
                Some("active") => ServiceStatus::Active,
                Some("expired") => ServiceStatus::Expired,
                Some("suspended") => ServiceStatus::Suspended,
                _ => status,
            };

            services.push(Service {
                id: lic.id.unwrap_or(0),
                name: d.product_name.clone().unwrap_or_else(|| lic.product_name.clone().unwrap_or_else(|| "License".into())),
                license_key: detail_key,
                status: detail_status,
                auth_mode: d.auth_mode.clone().unwrap_or_else(|| "api_key".into()),
                auto_renew: d.auto_renew_enabled.unwrap_or(false),
                issue_date: format_date(d.issued_date.as_deref()),
                renewal_date: format_date(d.renewal_date.as_deref()),
                allowed_domains: d.allowed_domains.clone(),
                devnet_allowed_domains: d.devnet_allowed_domains.clone(),
                locations,
            });
        } else {
            let empty_ips = std::collections::HashMap::new();
            let locations: Vec<Location> = lic.allowed_locations.iter().map(|loc| {
                build_location(loc, &empty_ips)
            }).collect();

            services.push(Service {
                id: lic.id.unwrap_or(0),
                name: lic.product_name.clone().unwrap_or_else(|| "License".into()),
                license_key: license_key.clone(),
                status,
                auth_mode: lic.auth_mode.clone().unwrap_or_else(|| "api_key".into()),
                auto_renew: false,
                issue_date: format_date(lic.issued_date.as_deref()),
                renewal_date: format_date(lic.renewal_date.as_deref()),
                allowed_domains: Vec::new(),
                devnet_allowed_domains: Vec::new(),
                locations,
            });
        }
    }

    let api_invoices = api.invoices().await.unwrap_or_default();
    let invoices: Vec<Invoice> = api_invoices
        .iter()
        .map(|inv| {
            let status = match inv.status.as_deref() {
                Some("paid") => InvoiceStatus::Paid,
                _ => InvoiceStatus::Pending,
            };
            Invoice {
                id: inv.reference.clone().unwrap_or_else(|| format!("#{}", inv.id.unwrap_or(0))),
                amount: inv.amount.map(|a| format!("${:.2}", a)).unwrap_or_else(|| "—".into()),
                date: inv.created_at.clone().unwrap_or_else(|| "—".into()),
                status,
            }
        })
        .collect();

    let api_key_entries = api.api_keys().await.unwrap_or_default();
    let api_keys: Vec<ApiKeyItem> = api_key_entries
        .iter()
        .map(|k| ApiKeyItem {
            id: k.id.unwrap_or(0),
            name: k.name.clone().unwrap_or_else(|| "Unnamed".into()),
            key_masked: k.key_masked.clone().unwrap_or_else(|| "••••••••".into()),
            enabled: k.enabled.unwrap_or(true),
            created_at: format_date(k.created_at.as_deref()),
            ip_whitelist: k.ip_whitelist.clone(),
            ip_count: k.ip_count.unwrap_or(0),
        })
        .collect();

    DashboardData {
        user_id: user_id.unwrap_or(0),
        user_name,
        email: user_email,
        wallet_address: user_wallet,
        payment_wallet_address: payment_wallet,
        created_at: format_date(user_created_at.as_deref()),
        balance,
        currency,
        service_info,
        services,
        invoices,
        api_keys,
        connected_accounts,
        theme: initial_theme,
    }
}

fn build_location(
    loc: &crate::api_client::LocationData,
    whitelisted_ips: &std::collections::HashMap<String, Vec<crate::api_client::WhitelistedIp>>,
) -> Location {
    let mut endpoints = Vec::new();
    if let Some(ref urls) = loc.urls {
        if let Some(ref u) = urls.rpc { endpoints.push(Endpoint { label: "RPC".into(), url: u.clone() }); }
        if let Some(ref u) = urls.websocket { endpoints.push(Endpoint { label: "WebSocket".into(), url: u.clone() }); }
        if let Some(ref u) = urls.yellowstone { endpoints.push(Endpoint { label: "gRPC (Yellowstone)".into(), url: u.clone() }); }
        if let Some(ref u) = urls.jetstream { endpoints.push(Endpoint { label: "Jetstream".into(), url: u.clone() }); }
        if let Some(ref u) = urls.tx_sender { endpoints.push(Endpoint { label: "TX Sender".into(), url: u.clone() }); }
    }

    let loc_id_num = loc.id.unwrap_or(0);
    let loc_id = loc_id_num.to_string();
    let ips: Vec<IpEntry> = whitelisted_ips
        .get(&loc_id)
        .map(|ip_list| {
            ip_list.iter().map(|ip| IpEntry {
                ip_address: ip.ip_address.clone().unwrap_or_default(),
                is_rented: ip.is_rented.unwrap_or(false),
                expires_at: ip.expires_at.clone().unwrap_or_default(),
            }).collect()
        })
        .unwrap_or_default();

    Location {
        id: loc_id_num,
        name: loc.name.clone().unwrap_or_else(|| "Unknown".into()),
        slots: loc.slots.unwrap_or(0),
        endpoints,
        ips,
    }
}

fn format_date(iso: Option<&str>) -> String {
    iso.and_then(|s| {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%:z"))
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S+00:00"))
            .ok()
            .map(|dt| dt.format("%d/%m/%Y").to_string())
    })
    .unwrap_or_else(|| "—".into())
}

async fn run_loop(
    terminal: &mut DefaultTerminal,
    data: DashboardData,
    api: &OrbitApi,
) -> CliResult<theme::ThemeName> {
    let mut app = App::new(data);
    let mut events = EventHandler::new(Duration::from_millis(250));

    terminal.draw(|frame| ui::render(frame, &app))?;

    while app.running {
        match events.next().await {
            Some(Event::Tick) => {
                app.tick();
                terminal.draw(|frame| ui::render(frame, &app))?;
            }
            Some(Event::Key(key)) => {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
            None => break,
        }

        if let Some(action) = app.pending_action.take() {
            app.loading = true;
            terminal.draw(|frame| ui::render(frame, &app))?;
            execute_action(&mut app, api, action).await;
            app.loading = false;
        }
    }

    Ok(app.theme)
}

fn format_created_key(k: &crate::api_client::CreatedApiKey) -> String {
    let key = k.key.as_deref()
        .or(k.key_masked.as_deref())
        .unwrap_or("(hidden)");
    let name = k.name.as_deref().unwrap_or("Unnamed");
    let id = k.id.map(|i| format!(" #{}", i)).unwrap_or_default();
    let enabled = if k.enabled.unwrap_or(true) { "enabled" } else { "disabled" };
    let ips = if k.ip_whitelist.is_empty() {
        format!("{} IPs", k.ip_count.unwrap_or(0))
    } else {
        k.ip_whitelist.join(", ")
    };
    let date = k.created_at.as_deref().unwrap_or("—");
    format!("{}{} — {} ({}, {}, created {})", name, id, key, enabled, ips, date)
}

async fn refresh_api_keys(app: &mut App, api: &OrbitApi) {
    let entries = api.api_keys().await.unwrap_or_default();
    app.api_keys = entries
        .iter()
        .map(|k| ApiKeyItem {
            id: k.id.unwrap_or(0),
            name: k.name.clone().unwrap_or_else(|| "Unnamed".into()),
            key_masked: k.key_masked.clone().unwrap_or_else(|| "••••••••".into()),
            enabled: k.enabled.unwrap_or(true),
            created_at: format_date(k.created_at.as_deref()),
            ip_whitelist: k.ip_whitelist.clone(),
            ip_count: k.ip_count.unwrap_or(0),
        })
        .collect();

    if !app.api_keys.is_empty() {
        if app.api_keys_selected >= app.api_keys.len() {
            app.api_keys_selected = app.api_keys.len() - 1;
        }
    } else {
        app.api_keys_selected = 0;
        app.api_keys_expanded = None;
    }
}

async fn refresh_service(app: &mut App, api: &OrbitApi) {
    let idx = app.services_selected;
    if let Some(svc) = app.services.get(idx) {
        let license_key = svc.license_key.clone();
        if let Ok(Some(d)) = api.license_detail(&license_key).await {
            let locations: Vec<Location> = d.allowed_locations.iter().map(|loc| {
                build_location(loc, &d.whitelisted_ips)
            }).collect();

            let status = match d.status.as_deref() {
                Some("active") => ServiceStatus::Active,
                Some("expired") => ServiceStatus::Expired,
                Some("suspended") => ServiceStatus::Suspended,
                _ => svc.status,
            };

            let updated = Service {
                id: svc.id,
                name: d.product_name.clone().unwrap_or_else(|| svc.name.clone()),
                license_key: d.license_key.clone().unwrap_or(license_key),
                status,
                auth_mode: d.auth_mode.clone().unwrap_or_else(|| svc.auth_mode.clone()),
                auto_renew: d.auto_renew_enabled.unwrap_or(svc.auto_renew),
                issue_date: format_date(d.issued_date.as_deref()),
                renewal_date: format_date(d.renewal_date.as_deref()),
                allowed_domains: d.allowed_domains.clone(),
                devnet_allowed_domains: d.devnet_allowed_domains.clone(),
                locations,
            };
            app.services[idx] = updated;
        }
    }
}

async fn execute_action(app: &mut App, api: &OrbitApi, action: app::Action) {
    match action {
        app::Action::RefreshApiKeys => {
            refresh_api_keys(app, api).await;
        }
        app::Action::CreateApiKey { name } => {
            match api.create_api_key(&name).await {
                Ok(created) => {
                    let key_str = format_created_key(&created);
                    app.set_status(format!("Created: {}", key_str), false);
                    refresh_api_keys(app, api).await;
                }
                Err(e) => {
                    app.set_status(format!("Failed to create: {}", e), true);
                }
            }
        }
        app::Action::ToggleApiKeyEnabled { id, currently_enabled } => {
            let new_enabled = !currently_enabled;
            match api.update_api_key(id, None, Some(new_enabled)).await {
                Ok(_) => {
                    let label = if new_enabled { "enabled" } else { "disabled" };
                    app.set_status(format!("API key {}", label), false);
                    refresh_api_keys(app, api).await;
                }
                Err(e) => {
                    app.set_status(format!("Failed to toggle: {}", e), true);
                }
            }
        }
        app::Action::RegenerateApiKey { id } => {
            match api.regenerate_api_key(id).await {
                Ok(created) => {
                    let key_str = format_created_key(&created);
                    app.set_status(format!("Regenerated: {}", key_str), false);
                    refresh_api_keys(app, api).await;
                }
                Err(e) => {
                    app.set_status(format!("Failed to regenerate: {}", e), true);
                }
            }
        }
        app::Action::DeleteApiKey { id } => {
            match api.delete_api_key(id).await {
                Ok(_) => {
                    app.api_keys_expanded = None;
                    app.set_status("API key deleted".into(), false);
                    refresh_api_keys(app, api).await;
                }
                Err(e) => {
                    app.set_status(format!("Failed to delete: {}", e), true);
                }
            }
        }
        app::Action::AddApiKeyIp { id, ip } => {
            match api.api_key_add_ip(id, &ip).await {
                Ok(_) => {
                    app.set_status(format!("IP {} added", ip), false);
                    refresh_api_keys(app, api).await;
                }
                Err(e) => {
                    app.set_status(format!("Failed to add IP: {}", e), true);
                }
            }
        }
        app::Action::RemoveApiKeyIp { id, ip } => {
            match api.api_key_remove_ip(id, &ip).await {
                Ok(_) => {
                    app.set_status(format!("IP {} removed", ip), false);
                    refresh_api_keys(app, api).await;
                    if let Some(exp_idx) = app.api_keys_expanded
                        && let Some(key_item) = app.api_keys.get(exp_idx)
                        && app.api_keys_ip_selected >= key_item.ip_whitelist.len()
                        && !key_item.ip_whitelist.is_empty()
                    {
                        app.api_keys_ip_selected = key_item.ip_whitelist.len() - 1;
                    }
                }
                Err(e) => {
                    app.set_status(format!("Failed to remove IP: {}", e), true);
                }
            }
        }
        app::Action::RefreshService => {
            refresh_service(app, api).await;
            app.set_status("Service refreshed".into(), false);
        }
        app::Action::ToggleLicenseAuthMode { license_key } => {
            let current = app.services.get(app.services_selected)
                .map(|s| s.auth_mode.as_str())
                .unwrap_or("ip_whitelist");
            let new_mode = if current == "api_key" { "ip_whitelist" } else { "api_key" };
            match api.license_toggle_auth_mode(&license_key, new_mode).await {
                Ok(_) => {
                    refresh_service(app, api).await;
                    let mode = app.services.get(app.services_selected)
                        .map(|s| s.auth_mode.as_str())
                        .unwrap_or("unknown");
                    app.set_status(format!("Auth mode: {}", mode), false);
                }
                Err(e) => {
                    app.set_status(format!("Failed to toggle auth mode: {}", e), true);
                }
            }
        }
        app::Action::RegenerateLicenseKey { license_key } => {
            match api.license_regenerate_key(&license_key).await {
                Ok(_) => {
                    refresh_service(app, api).await;
                    app.set_status("License key regenerated".into(), false);
                }
                Err(e) => {
                    app.set_status(format!("Failed to regenerate: {}", e), true);
                }
            }
        }
        app::Action::AddLicenseIp { license_key, ip, location_id } => {
            match api.license_add_ip(&license_key, &ip, location_id).await {
                Ok(_) => {
                    app.set_status(format!("IP {} added", ip), false);
                    refresh_service(app, api).await;
                }
                Err(e) => {
                    app.set_status(format!("Failed to add IP: {}", e), true);
                }
            }
        }
        app::Action::RemoveLicenseIp { license_key, ip, location_id } => {
            match api.license_remove_ip(&license_key, &ip, location_id).await {
                Ok(_) => {
                    app.set_status(format!("IP {} removed", ip), false);
                    refresh_service(app, api).await;
                    if let Some(loc_idx) = app.detail_location_expanded
                        && let Some(svc) = app.services.get(app.services_selected)
                        && let Some(loc) = svc.locations.get(loc_idx)
                        && app.detail_ip_selected >= loc.ips.len()
                        && !loc.ips.is_empty()
                    {
                        app.detail_ip_selected = loc.ips.len() - 1;
                    }
                }
                Err(e) => {
                    app.set_status(format!("Failed to remove IP: {}", e), true);
                }
            }
        }
        app::Action::ToggleAutoRenewal { license_key } => {
            match api.license_toggle_auto_renewal(&license_key).await {
                Ok(_) => {
                    refresh_service(app, api).await;
                    let enabled = app.services.get(app.services_selected)
                        .map(|s| s.auto_renew)
                        .unwrap_or(false);
                    let label = if enabled { "enabled" } else { "disabled" };
                    app.set_status(format!("Auto-renewal {}", label), false);
                }
                Err(e) => {
                    app.set_status(format!("Failed to toggle auto-renewal: {}", e), true);
                }
            }
        }
        app::Action::AddDomain { license_key, domain } => {
            match api.license_add_domain(&license_key, &domain).await {
                Ok(_) => {
                    app.set_status(format!("Domain {} added", domain), false);
                    refresh_service(app, api).await;
                }
                Err(e) => {
                    app.set_status(format!("Failed to add domain: {}", e), true);
                }
            }
        }
        app::Action::RemoveDomain { license_key, domain } => {
            match api.license_remove_domain(&license_key, &domain).await {
                Ok(_) => {
                    app.set_status(format!("Domain {} removed", domain), false);
                    refresh_service(app, api).await;
                    if let Some(svc) = app.services.get(app.services_selected)
                        && app.detail_domain_selected >= svc.allowed_domains.len()
                        && !svc.allowed_domains.is_empty()
                    {
                        app.detail_domain_selected = svc.allowed_domains.len() - 1;
                    }
                }
                Err(e) => {
                    app.set_status(format!("Failed to remove domain: {}", e), true);
                }
            }
        }
        app::Action::AddDevnetDomain { license_key, domain } => {
            match api.license_add_devnet_domain(&license_key, &domain).await {
                Ok(_) => {
                    app.set_status(format!("Devnet domain {} added", domain), false);
                    refresh_service(app, api).await;
                }
                Err(e) => {
                    app.set_status(format!("Failed to add devnet domain: {}", e), true);
                }
            }
        }
        app::Action::RemoveDevnetDomain { license_key, domain } => {
            match api.license_remove_devnet_domain(&license_key, &domain).await {
                Ok(_) => {
                    app.set_status(format!("Devnet domain {} removed", domain), false);
                    refresh_service(app, api).await;
                    if let Some(svc) = app.services.get(app.services_selected)
                        && app.detail_domain_selected >= svc.devnet_allowed_domains.len()
                        && !svc.devnet_allowed_domains.is_empty()
                    {
                        app.detail_domain_selected = svc.devnet_allowed_domains.len() - 1;
                    }
                }
                Err(e) => {
                    app.set_status(format!("Failed to remove devnet domain: {}", e), true);
                }
            }
        }
        app::Action::RefreshInvoices => {
            let api_invoices = api.invoices().await.unwrap_or_default();
            app.invoices = api_invoices
                .iter()
                .map(|inv| {
                    let status = match inv.status.as_deref() {
                        Some("paid") => InvoiceStatus::Paid,
                        _ => InvoiceStatus::Pending,
                    };
                    Invoice {
                        id: inv.reference.clone().unwrap_or_else(|| format!("#{}", inv.id.unwrap_or(0))),
                        amount: inv.amount.map(|a| format!("${:.2}", a)).unwrap_or_else(|| "—".into()),
                        date: inv.created_at.clone().unwrap_or_else(|| "—".into()),
                        status,
                    }
                })
                .collect();
            if app.invoices_selected >= app.invoices.len() && !app.invoices.is_empty() {
                app.invoices_selected = app.invoices.len() - 1;
            }
            app.set_status("Invoices refreshed".into(), false);
        }
        app::Action::PayInvoice { reference } => {
            match api.pay_invoice_with_balance(&reference).await {
                Ok(_) => {
                    app.set_status(format!("Invoice {} paid", reference), false);
                }
                Err(e) => {
                    app.set_status(format!("Payment failed: {}", e), true);
                }
            }
        }
        app::Action::UpdateProfile { name, wallet } => {
            match api.update_profile(name.as_deref(), wallet.as_deref()).await {
                Ok(profile) => {
                    if let Some(ref n) = profile.name {
                        app.user_display = n.clone();
                    }
                    if let Some(ref w) = profile.payment_wallet_address {
                        app.payment_wallet_address = w.clone();
                    }
                    app.set_status("Profile updated".into(), false);
                }
                Err(e) => {
                    app.set_status(format!("Failed to update profile: {}", e), true);
                }
            }
        }
    }
}
