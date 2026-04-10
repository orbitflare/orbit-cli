use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::models::{self, ApiKeyItem, CoinPrice, ConnectedAccountItem, Invoice, Service, ServiceInfo};
use super::theme::ThemeName;
#[derive(Clone, Debug)]
pub enum Action {
    RefreshApiKeys,
    CreateApiKey { name: String },
    ToggleApiKeyEnabled { id: u64, currently_enabled: bool },
    RegenerateApiKey { id: u64 },
    DeleteApiKey { id: u64 },
    AddApiKeyIp { id: u64, ip: String },
    RemoveApiKeyIp { id: u64, ip: String },
    RefreshService,
    ToggleLicenseAuthMode { license_key: String },
    ToggleAutoRenewal { license_key: String },
    RegenerateLicenseKey { license_key: String },
    AddLicenseIp { license_key: String, ip: String, location_id: u64 },
    RemoveLicenseIp { license_key: String, ip: String, location_id: u64 },
    AddDomain { license_key: String, domain: String },
    RemoveDomain { license_key: String, domain: String },
    AddDevnetDomain { license_key: String, domain: String },
    RemoveDevnetDomain { license_key: String, domain: String },
    RefreshInvoices,
    PayInvoice { reference: String },
    UpdateProfile { name: Option<String>, wallet: Option<String> },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputTarget {
    AddIp,
    CreateApiKey,
    AddLicenseIp,
    AddDomain,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProfileField {
    Name,
    Wallet,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConfirmKind {
    Regenerate,
    Delete,
    RemoveIp,
    RegenerateLicenseKey,
    RemoveLicenseIp,
    RemoveDomain,
    PayInvoice,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Balance,
    Service,
    Invoices,
}

impl Panel {
    pub fn next(self) -> Panel {
        match self {
            Panel::Balance => Panel::Service,
            Panel::Service => Panel::Invoices,
            Panel::Invoices => Panel::Balance,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Home,
    Services,
    ServiceDetail,
    Invoices,
    ApiKeys,
    Profile,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Grid,
    List,
    Compact,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Details,
    WhitelistedIps,
    Domains,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DomainNetwork {
    Mainnet,
    Devnet,
}

pub struct SidebarItem {
    pub label: &'static str,
    pub icon: &'static str,
}

pub struct DashboardData {
    pub user_name: Option<String>,
    pub email: Option<String>,
    pub wallet_address: Option<String>,
    pub payment_wallet_address: Option<String>,
    pub created_at: String,
    pub balance: f64,
    pub currency: String,
    pub service_info: Option<ServiceInfo>,
    pub services: Vec<Service>,
    pub invoices: Vec<Invoice>,
    pub api_keys: Vec<ApiKeyItem>,
    pub connected_accounts: Vec<ConnectedAccountItem>,
    pub theme: ThemeName,
}

pub struct App {
    pub running: bool,
    pub tick_count: u64,

    pub prices: Vec<CoinPrice>,
    pub ticker_offset: usize,

    pub greeting: String,
    pub user_display: String,

    pub email: String,
    pub wallet_address: String,
    pub payment_wallet_address: String,
    pub created_at: String,
    pub currency: String,

    pub balance: f64,
    pub service: ServiceInfo,
    pub invoices: Vec<Invoice>,
    pub connected_accounts: Vec<ConnectedAccountItem>,

    pub theme: ThemeName,

    pub focus: Panel,
    pub sidebar_items: Vec<SidebarItem>,
    pub selected_sidebar: usize,
    pub sidebar_collapsed: bool,

    pub section: Section,

    pub services: Vec<Service>,
    pub services_selected: usize,
    pub services_view: ViewMode,

    pub invoices_view: ViewMode,
    pub invoices_selected: usize,

    pub detail_scroll: u16,
    pub detail_tab: DetailTab,
    pub detail_ep_expanded: std::collections::HashSet<usize>,
    pub detail_ep_selected: usize,
    pub detail_location_selected: usize,
    pub detail_location_expanded: Option<usize>,
    pub detail_ip_selected: usize,
    pub detail_domain_network: DomainNetwork,
    pub detail_domain_selected: usize,

    pub api_keys: Vec<ApiKeyItem>,
    pub api_keys_selected: usize,
    pub api_keys_scroll: u16,
    pub api_keys_expanded: Option<usize>,
    pub api_keys_ip_selected: usize,

    pub profile_editing: bool,
    pub profile_field: ProfileField,
    pub profile_name_buf: String,
    pub profile_wallet_buf: String,
    pub profile_cursor: usize,

    pub pending_action: Option<Action>,

    pub input_mode: Option<InputTarget>,
    pub input_buffer: String,

    pub confirm: Option<ConfirmKind>,
    pub confirm_context: Option<String>,

    pub status_message: Option<String>,
    pub status_is_error: bool,
    pub status_ticks_remaining: u16,

    pub loading: bool,
}

impl App {
    pub fn new(data: DashboardData) -> Self {
        let name = data.user_name.as_deref().unwrap_or("User");
        let first = name.split_whitespace().next().unwrap_or(name);

        let hour = chrono::Local::now().hour();
        let period = match hour {
            0..=11 => "morning",
            12..=17 => "afternoon",
            _ => "evening",
        };
        let greeting = format!("Good {}, {}", period, first);
        let user_display = name.to_string();

        let sidebar_items = vec![
            SidebarItem { label: "Dashboard", icon: "\u{f0e4}" },
            SidebarItem { label: "Services", icon: "\u{f013}" },
            SidebarItem { label: "Invoices", icon: "\u{f0f6}" },
            SidebarItem { label: "API Keys", icon: "\u{f084}" },
            SidebarItem { label: "Profile", icon: "\u{f2bb}" },
        ];

        Self {
            running: true,
            tick_count: 0,

            prices: models::ticker_prices(),
            ticker_offset: 0,

            greeting,
            user_display,

            email: data.email.unwrap_or_default(),
            wallet_address: data.wallet_address.unwrap_or_default(),
            payment_wallet_address: data.payment_wallet_address.clone().unwrap_or_default(),
            created_at: data.created_at,
            currency: data.currency,
            theme: data.theme,

            balance: data.balance,
            service: data.service_info.unwrap_or_default(),
            invoices: data.invoices,
            connected_accounts: data.connected_accounts,

            focus: Panel::Balance,
            sidebar_items,
            selected_sidebar: 0,
            sidebar_collapsed: false,

            section: Section::Home,

            services: data.services,
            services_selected: 0,
            services_view: ViewMode::List,

            invoices_view: ViewMode::Grid,
            invoices_selected: 0,

            detail_scroll: 0,
            detail_tab: DetailTab::Details,
            detail_ep_expanded: std::collections::HashSet::new(),
            detail_ep_selected: 0,
            detail_location_selected: 0,
            detail_location_expanded: None,
            detail_ip_selected: 0,
            detail_domain_network: DomainNetwork::Mainnet,
            detail_domain_selected: 0,

            api_keys: data.api_keys,
            api_keys_selected: 0,
            api_keys_scroll: 0,
            api_keys_expanded: None,
            api_keys_ip_selected: 0,

            profile_editing: false,
            profile_field: ProfileField::Name,
            profile_name_buf: name.to_string(),
            profile_wallet_buf: data.payment_wallet_address.as_deref().unwrap_or("").to_string(),
            profile_cursor: 0,

            pending_action: None,
            input_mode: None,
            input_buffer: String::new(),
            confirm: None,
            confirm_context: None,
            status_message: None,
            status_is_error: false,
            status_ticks_remaining: 0,
            loading: false,
        }
    }

    pub fn seconds_since_update(&self) -> u64 {
        0
    }

    pub fn tick(&mut self) {
        self.tick_count += 1;

        if self.tick_count.is_multiple_of(2) {
            self.ticker_offset += 1;
        }

        if self.tick_count.is_multiple_of(4) {
            self.prices = models::ticker_prices();
        }

        if self.status_ticks_remaining > 0 {
            self.status_ticks_remaining -= 1;
            if self.status_ticks_remaining == 0 {
                self.status_message = None;
            }
        }
    }

    pub fn set_status(&mut self, msg: String, is_error: bool) {
        self.status_message = Some(msg);
        self.status_is_error = is_error;
        self.status_ticks_remaining = 16;
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.input_mode.is_some() {
            self.handle_input_key(key);
            return;
        }

        if self.confirm.is_some() {
            self.handle_confirm_key(key);
            return;
        }

        if self.section == Section::Profile && self.profile_editing {
            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.running = false;
                }
                KeyCode::Esc => {
                    self.profile_editing = false;
                }
                _ => self.handle_profile_key(key),
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            KeyCode::Char('[') => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
            }
            KeyCode::Char('t') => {
                self.theme = self.theme.next();
            }
            KeyCode::Char('T') => {
                self.theme = self.theme.prev();
            }
            KeyCode::Esc => {
                match self.section {
                    Section::ServiceDetail => self.section = Section::Services,
                    Section::Home => {}
                    _ => {
                        self.section = Section::Home;
                        self.selected_sidebar = 0;
                    }
                }
            }
            _ => {
                match self.section {
                    Section::Home => self.handle_home_key(key),
                    Section::Services => self.handle_services_key(key),
                    Section::ServiceDetail => self.handle_detail_key(key),
                    Section::Invoices => self.handle_invoices_key(key),
                    Section::ApiKeys => self.handle_api_keys_key(key),
                    Section::Profile => {
                        if key.code == KeyCode::Char('e') {
                            self.profile_name_buf = self.user_display.clone();
                            self.profile_wallet_buf = self.payment_wallet_address.clone();
                            self.profile_field = ProfileField::Name;
                            self.profile_cursor = self.user_display.len();
                            self.profile_editing = true;
                        }
                    }
                }
            }
        }
    }

    fn handle_home_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => {
                self.focus = self.focus.next();
            }
            KeyCode::Enter => {
                let label = self.sidebar_items[self.selected_sidebar].label;
                match label {
                    "Dashboard" => self.section = Section::Home,
                    "Services" => self.section = Section::Services,
                    "Invoices" => self.section = Section::Invoices,
                    "API Keys" => {
                        self.section = Section::ApiKeys;
                        self.pending_action = Some(Action::RefreshApiKeys);
                    }
                    "Profile" => {
                        self.section = Section::Profile;
                    }
                    _ => {}
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_sidebar > 0 {
                    self.selected_sidebar -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.sidebar_items.len().saturating_sub(1);
                if self.selected_sidebar < max {
                    self.selected_sidebar += 1;
                }
            }
            _ => {}
        }
    }

    fn active_buf_len(&self) -> usize {
        match self.profile_field {
            ProfileField::Name => self.profile_name_buf.len(),
            ProfileField::Wallet => self.profile_wallet_buf.len(),
        }
    }

    fn handle_profile_key(&mut self, key: KeyEvent) {
        if self.loading { return; }
        match key.code {
            KeyCode::Tab => {
                self.profile_field = match self.profile_field {
                    ProfileField::Name => ProfileField::Wallet,
                    ProfileField::Wallet => ProfileField::Name,
                };
                self.profile_cursor = self.active_buf_len();
            }
            KeyCode::Left => {
                self.profile_cursor = self.profile_cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                let len = self.active_buf_len();
                if self.profile_cursor < len {
                    self.profile_cursor += 1;
                }
            }
            KeyCode::Home => {
                self.profile_cursor = 0;
            }
            KeyCode::End => {
                self.profile_cursor = self.active_buf_len();
            }
            KeyCode::Backspace => {
                if self.profile_cursor > 0 {
                    let pos = self.profile_cursor - 1;
                    match self.profile_field {
                        ProfileField::Name => { self.profile_name_buf.remove(pos); }
                        ProfileField::Wallet => { self.profile_wallet_buf.remove(pos); }
                    }
                    self.profile_cursor -= 1;
                }
            }
            KeyCode::Delete => {
                let len = self.active_buf_len();
                if self.profile_cursor < len {
                    let pos = self.profile_cursor;
                    match self.profile_field {
                        ProfileField::Name => { self.profile_name_buf.remove(pos); }
                        ProfileField::Wallet => { self.profile_wallet_buf.remove(pos); }
                    }
                }
            }
            KeyCode::Char(c) => {
                let pos = self.profile_cursor;
                match self.profile_field {
                    ProfileField::Name => self.profile_name_buf.insert(pos, c),
                    ProfileField::Wallet => self.profile_wallet_buf.insert(pos, c),
                }
                self.profile_cursor += 1;
            }
            KeyCode::Enter => {
                let name = self.profile_name_buf.trim().to_string();
                let wallet = self.profile_wallet_buf.trim().to_string();
                self.pending_action = Some(Action::UpdateProfile {
                    name: if name.is_empty() { None } else { Some(name) },
                    wallet: if wallet.is_empty() { None } else { Some(wallet) },
                });
            }
            _ => {}
        }
    }

    fn handle_invoices_key(&mut self, key: KeyEvent) {
        if self.loading { return; }
        match key.code {
            KeyCode::Tab => {
                self.invoices_view = match self.invoices_view {
                    ViewMode::Grid => ViewMode::List,
                    ViewMode::List => ViewMode::Compact,
                    ViewMode::Compact => ViewMode::Grid,
                };
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.invoices_selected > 0 {
                    self.invoices_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.invoices.len().saturating_sub(1);
                if self.invoices_selected < max {
                    self.invoices_selected += 1;
                }
            }
            KeyCode::Char('p') => {
                if let Some(inv) = self.invoices.get(self.invoices_selected)
                    && inv.status.label() == "Pending"
                {
                    self.confirm = Some(ConfirmKind::PayInvoice);
                    self.confirm_context = Some(inv.id.clone());
                }
            }
            KeyCode::Char('R') => {
                self.pending_action = Some(Action::RefreshInvoices);
            }
            _ => {}
        }
    }

    fn handle_services_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.services_selected > 0 {
                    self.services_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.services.len().saturating_sub(1);
                if self.services_selected < max {
                    self.services_selected += 1;
                }
            }
            KeyCode::Enter => {
                if !self.services.is_empty() {
                    self.detail_scroll = 0;
                    self.detail_tab = DetailTab::Details;
                    self.detail_location_expanded = None;
                    self.section = Section::ServiceDetail;
                }
            }
            KeyCode::Tab => {
                self.services_view = match self.services_view {
                    ViewMode::Grid => ViewMode::List,
                    ViewMode::List => ViewMode::Compact,
                    ViewMode::Compact => ViewMode::Grid,
                };
            }
            _ => {}
        }
    }

    fn handle_api_keys_key(&mut self, key: KeyEvent) {
        if self.loading {
            return;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.api_keys_expanded.is_some() {
                    self.api_keys_ip_selected = self.api_keys_ip_selected.saturating_sub(1);
                } else if self.api_keys_selected > 0 {
                    self.api_keys_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(exp_idx) = self.api_keys_expanded {
                    if let Some(key_item) = self.api_keys.get(exp_idx) {
                        let max = key_item.ip_whitelist.len().saturating_sub(1);
                        if self.api_keys_ip_selected < max {
                            self.api_keys_ip_selected += 1;
                        }
                    }
                } else {
                    let max = self.api_keys.len().saturating_sub(1);
                    if self.api_keys_selected < max {
                        self.api_keys_selected += 1;
                    }
                }
            }
            KeyCode::Enter => {
                if self.api_keys_expanded == Some(self.api_keys_selected) {
                    self.api_keys_expanded = None;
                } else {
                    self.api_keys_expanded = Some(self.api_keys_selected);
                    self.api_keys_ip_selected = 0;
                }
            }
            KeyCode::Char('e') => {
                if let Some(key_item) = self.api_keys.get(self.api_keys_selected) {
                    self.pending_action = Some(Action::ToggleApiKeyEnabled {
                        id: key_item.id,
                        currently_enabled: key_item.enabled,
                    });
                }
            }
            KeyCode::Char('r') => {
                if self.api_keys.get(self.api_keys_selected).is_some() {
                    self.confirm = Some(ConfirmKind::Regenerate);
                }
            }
            KeyCode::Char('x') => {
                if self.api_keys.get(self.api_keys_selected).is_some() {
                    self.confirm = Some(ConfirmKind::Delete);
                }
            }
            KeyCode::Char('a') => {
                if self.api_keys_expanded.is_some() {
                    self.input_mode = Some(InputTarget::AddIp);
                    self.input_buffer.clear();
                }
            }
            KeyCode::Char('d') => {
                if let Some(exp_idx) = self.api_keys_expanded
                    && let Some(key_item) = self.api_keys.get(exp_idx)
                    && let Some(ip) = key_item.ip_whitelist.get(self.api_keys_ip_selected)
                {
                    self.confirm = Some(ConfirmKind::RemoveIp);
                    self.confirm_context = Some(ip.clone());
                }
            }
            KeyCode::Char('c') => {
                self.input_mode = Some(InputTarget::CreateApiKey);
                self.input_buffer.clear();
            }
            KeyCode::Char('R') => {
                self.pending_action = Some(Action::RefreshApiKeys);
            }
            _ => {}
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = None;
                self.input_buffer.clear();
            }
            KeyCode::Enter => {
                let value = self.input_buffer.trim().to_string();
                if !value.is_empty() {
                    match self.input_mode {
                        Some(InputTarget::AddIp) => {
                            if let Some(exp_idx) = self.api_keys_expanded
                                && let Some(key_item) = self.api_keys.get(exp_idx)
                            {
                                self.pending_action = Some(Action::AddApiKeyIp {
                                    id: key_item.id,
                                    ip: value,
                                });
                            }
                        }
                        Some(InputTarget::CreateApiKey) => {
                            self.pending_action = Some(Action::CreateApiKey { name: value });
                        }
                        Some(InputTarget::AddLicenseIp) => {
                            if let Some(svc) = self.services.get(self.services_selected)
                                && self.detail_location_expanded.is_some()
                            {
                                let is_mainnet = self.detail_domain_network == DomainNetwork::Mainnet;
                                if let Some(loc) = svc.locations.iter()
                                    .filter(|l| if is_mainnet { !l.name.eq_ignore_ascii_case("devnet") } else { l.name.eq_ignore_ascii_case("devnet") })
                                    .nth(self.detail_location_selected)
                                {
                                    self.pending_action = Some(Action::AddLicenseIp {
                                        license_key: svc.license_key.clone(),
                                        ip: value,
                                        location_id: loc.id,
                                    });
                                }
                            }
                        }
                        Some(InputTarget::AddDomain) => {
                            if let Some(svc) = self.services.get(self.services_selected) {
                                let lk = svc.license_key.clone();
                                self.pending_action = Some(match self.detail_domain_network {
                                    DomainNetwork::Mainnet => Action::AddDomain { license_key: lk, domain: value },
                                    DomainNetwork::Devnet => Action::AddDevnetDomain { license_key: lk, domain: value },
                                });
                            }
                        }
                        None => {}
                    }
                }
                self.input_mode = None;
                self.input_buffer.clear();
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let kind = self.confirm.take();
                match kind {
                    Some(ConfirmKind::Regenerate) => {
                        if let Some(key_item) = self.api_keys.get(self.api_keys_selected) {
                            self.pending_action = Some(Action::RegenerateApiKey { id: key_item.id });
                        }
                    }
                    Some(ConfirmKind::Delete) => {
                        if let Some(key_item) = self.api_keys.get(self.api_keys_selected) {
                            self.pending_action = Some(Action::DeleteApiKey { id: key_item.id });
                        }
                    }
                    Some(ConfirmKind::RemoveIp) => {
                        if let Some(ip) = self.confirm_context.take()
                            && let Some(exp_idx) = self.api_keys_expanded
                            && let Some(key_item) = self.api_keys.get(exp_idx)
                        {
                            self.pending_action = Some(Action::RemoveApiKeyIp {
                                id: key_item.id,
                                ip,
                            });
                        }
                    }
                    Some(ConfirmKind::RegenerateLicenseKey) => {
                        if let Some(svc) = self.services.get(self.services_selected) {
                            self.pending_action = Some(Action::RegenerateLicenseKey {
                                license_key: svc.license_key.clone(),
                            });
                        }
                    }
                    Some(ConfirmKind::RemoveLicenseIp) => {
                        if let Some(ip) = self.confirm_context.take()
                            && let Some(svc) = self.services.get(self.services_selected)
                            && self.detail_location_expanded.is_some()
                        {
                            let is_mainnet = self.detail_domain_network == DomainNetwork::Mainnet;
                            if let Some(loc) = svc.locations.iter()
                                .filter(|l| if is_mainnet { !l.name.eq_ignore_ascii_case("devnet") } else { l.name.eq_ignore_ascii_case("devnet") })
                                .nth(self.detail_location_selected)
                            {
                                self.pending_action = Some(Action::RemoveLicenseIp {
                                    license_key: svc.license_key.clone(),
                                    ip,
                                    location_id: loc.id,
                                });
                            }
                        }
                    }
                    Some(ConfirmKind::RemoveDomain) => {
                        if let Some(domain) = self.confirm_context.take()
                            && let Some(svc) = self.services.get(self.services_selected)
                        {
                            let lk = svc.license_key.clone();
                            self.pending_action = Some(match self.detail_domain_network {
                                DomainNetwork::Mainnet => Action::RemoveDomain { license_key: lk, domain },
                                DomainNetwork::Devnet => Action::RemoveDevnetDomain { license_key: lk, domain },
                            });
                        }
                    }
                    Some(ConfirmKind::PayInvoice) => {
                        if let Some(reference) = self.confirm_context.take() {
                            self.pending_action = Some(Action::PayInvoice { reference });
                        }
                    }
                    None => {}
                }
            }
            _ => {
                self.confirm = None;
                self.confirm_context = None;
            }
        }
    }

    fn handle_detail_key(&mut self, key: KeyEvent) {
        if self.loading {
            return;
        }
        let svc = match self.services.get(self.services_selected) {
            Some(s) => s,
            None => return,
        };
        let license_key = svc.license_key.clone();

        match key.code {
            KeyCode::Tab => {
                self.detail_scroll = 0;
                self.detail_tab = match self.detail_tab {
                    DetailTab::Details => DetailTab::WhitelistedIps,
                    DetailTab::WhitelistedIps => DetailTab::Domains,
                    DetailTab::Domains => DetailTab::Details,
                };
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.detail_tab == DetailTab::Details {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                } else if self.detail_tab == DetailTab::WhitelistedIps {
                    if self.detail_location_expanded.is_some() {
                        self.detail_ip_selected = self.detail_ip_selected.saturating_sub(1);
                    } else {
                        self.detail_location_selected = self.detail_location_selected.saturating_sub(1);
                    }
                } else if self.detail_tab == DetailTab::Domains {
                    self.detail_domain_selected = self.detail_domain_selected.saturating_sub(1);
                } else {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.detail_tab == DetailTab::WhitelistedIps {
                    let is_mainnet = self.detail_domain_network == DomainNetwork::Mainnet;
                    let filtered_count = svc.locations.iter()
                        .filter(|l| if is_mainnet { !l.name.eq_ignore_ascii_case("devnet") } else { l.name.eq_ignore_ascii_case("devnet") })
                        .count();
                    if self.detail_location_expanded.is_some() {
                        let filtered_loc = svc.locations.iter()
                            .filter(|l| if is_mainnet { !l.name.eq_ignore_ascii_case("devnet") } else { l.name.eq_ignore_ascii_case("devnet") })
                            .nth(self.detail_location_selected);
                        if let Some(loc) = filtered_loc {
                            let max = loc.ips.len().saturating_sub(1);
                            if self.detail_ip_selected < max {
                                self.detail_ip_selected += 1;
                            }
                        }
                    } else {
                        let max = filtered_count.saturating_sub(1);
                        if self.detail_location_selected < max {
                            self.detail_location_selected += 1;
                        }
                    }
                } else if self.detail_tab == DetailTab::Details {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                } else if self.detail_tab == DetailTab::Domains {
                    let domains = match self.detail_domain_network {
                        DomainNetwork::Mainnet => &svc.allowed_domains,
                        DomainNetwork::Devnet => &svc.devnet_allowed_domains,
                    };
                    let max = domains.len().saturating_sub(1);
                    if self.detail_domain_selected < max {
                        self.detail_domain_selected += 1;
                    }
                } else {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                }
            }
            KeyCode::PageUp => {
                if self.detail_tab == DetailTab::Details {
                    self.detail_ep_selected = self.detail_ep_selected.saturating_sub(1);
                }
            }
            KeyCode::PageDown => {
                if self.detail_tab == DetailTab::Details {
                    let max = svc.locations.len().saturating_sub(1);
                    if self.detail_ep_selected < max {
                        self.detail_ep_selected += 1;
                    }
                }
            }
            KeyCode::Enter => {
                if self.detail_tab == DetailTab::Details {
                    let idx = self.detail_ep_selected;
                    if self.detail_ep_expanded.contains(&idx) {
                        self.detail_ep_expanded.remove(&idx);
                    } else {
                        self.detail_ep_expanded.insert(idx);
                    }
                } else if self.detail_tab == DetailTab::WhitelistedIps {
                    if self.detail_location_expanded == Some(self.detail_location_selected) {
                        self.detail_location_expanded = None;
                    } else {
                        self.detail_location_expanded = Some(self.detail_location_selected);
                        self.detail_ip_selected = 0;
                    }
                }
            }
            KeyCode::Char('m') => {
                if self.detail_tab == DetailTab::Details {
                    self.pending_action = Some(Action::ToggleLicenseAuthMode { license_key });
                }
            }
            KeyCode::Char('w') => {
                if self.detail_tab == DetailTab::Details {
                    self.pending_action = Some(Action::ToggleAutoRenewal { license_key });
                }
            }
            KeyCode::Char('r') => {
                if self.detail_tab == DetailTab::Details {
                    self.confirm = Some(ConfirmKind::RegenerateLicenseKey);
                }
            }
            KeyCode::Char('n') => {
                if self.detail_tab == DetailTab::WhitelistedIps || self.detail_tab == DetailTab::Domains {
                    self.detail_domain_network = match self.detail_domain_network {
                        DomainNetwork::Mainnet => DomainNetwork::Devnet,
                        DomainNetwork::Devnet => DomainNetwork::Mainnet,
                    };
                    self.detail_domain_selected = 0;
                    self.detail_location_selected = 0;
                    self.detail_location_expanded = None;
                    self.detail_ip_selected = 0;
                }
            }
            KeyCode::Char('a') => {
                if self.detail_tab == DetailTab::WhitelistedIps && self.detail_location_expanded.is_some() {
                    let is_mainnet = self.detail_domain_network == DomainNetwork::Mainnet;
                    let has_loc = svc.locations.iter()
                        .filter(|l| if is_mainnet { !l.name.eq_ignore_ascii_case("devnet") } else { l.name.eq_ignore_ascii_case("devnet") })
                        .nth(self.detail_location_selected)
                        .is_some();
                    if has_loc {
                        self.input_mode = Some(InputTarget::AddLicenseIp);
                        self.input_buffer.clear();
                    }
                } else if self.detail_tab == DetailTab::Domains {
                    self.input_mode = Some(InputTarget::AddDomain);
                    self.input_buffer.clear();
                }
            }
            KeyCode::Char('d') => {
                if self.detail_tab == DetailTab::WhitelistedIps && self.detail_location_expanded.is_some() {
                    let is_mainnet = self.detail_domain_network == DomainNetwork::Mainnet;
                    let filtered_loc = svc.locations.iter()
                        .filter(|l| if is_mainnet { !l.name.eq_ignore_ascii_case("devnet") } else { l.name.eq_ignore_ascii_case("devnet") })
                        .nth(self.detail_location_selected);
                    if let Some(loc) = filtered_loc
                        && let Some(ip_entry) = loc.ips.get(self.detail_ip_selected)
                    {
                        self.confirm = Some(ConfirmKind::RemoveLicenseIp);
                        self.confirm_context = Some(ip_entry.ip_address.clone());
                    }
                } else if self.detail_tab == DetailTab::Domains {
                    let domains = match self.detail_domain_network {
                        DomainNetwork::Mainnet => &svc.allowed_domains,
                        DomainNetwork::Devnet => &svc.devnet_allowed_domains,
                    };
                    if let Some(domain) = domains.get(self.detail_domain_selected) {
                        self.confirm = Some(ConfirmKind::RemoveDomain);
                        self.confirm_context = Some(domain.clone());
                    }
                }
            }
            KeyCode::Char('R') => {
                self.pending_action = Some(Action::RefreshService);
            }
            _ => {}
        }
    }
}

use chrono::Timelike;
