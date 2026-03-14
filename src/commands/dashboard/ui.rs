use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::app::{App, DetailTab, Panel, Section, ViewMode};
use super::models::{PriceHighlight, ServiceStatus};
use super::theme::Theme;
macro_rules! theme {
    ($app:expr => $($name:ident),+ $(,)?) => {
        let _t = $app.theme.palette();
        $( let $name = _t.$name; )+
    };
}
pub fn render(frame: &mut Frame, app: &App) {
    theme!(app => bg);
    let size = frame.area();

    frame.render_widget(
        Block::default().style(Style::default().bg(bg)),
        size,
    );

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(10),
        ])
        .split(size);

    render_ticker(frame, app, rows[0]);

    let sidebar_width = if app.sidebar_collapsed { 5 } else { 28 };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(sidebar_width),
            Constraint::Min(40),
        ])
        .split(rows[1]);

    render_sidebar(frame, app, cols[0]);
    render_main(frame, app, cols[1]);
}
fn render_ticker(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => green_dot, label, success, yellow, value, ticker_bg);
    let mut spans: Vec<Span> = Vec::new();

    spans.push(Span::styled(" \u{25cf} ", Style::default().fg(green_dot)));
    spans.push(Span::styled(
        format!("Updated {}s ago  ", app.seconds_since_update()),
        Style::default().fg(label),
    ));

    let prices = &app.prices;
    let len = prices.len();
    for i in 0..len {
        let idx = (i + app.ticker_offset) % len;
        let coin = &prices[idx];
        let color = match coin.highlight {
            PriceHighlight::Green => success,
            PriceHighlight::Yellow => yellow,
            PriceHighlight::Normal => label,
        };
        let style = Style::default().fg(color);
        let bold_style = style.add_modifier(Modifier::BOLD);

        if coin.highlight != PriceHighlight::Normal {
            let icon = match coin.highlight {
                PriceHighlight::Green => "\u{25ce} ",
                PriceHighlight::Yellow => "\u{25c8} ",
                _ => "",
            };
            spans.push(Span::styled(icon, style));
            spans.push(Span::styled(format!("{} ", coin.symbol), bold_style));
            spans.push(Span::styled(format!("${:.2}  ", coin.price), bold_style));
        } else {
            spans.push(Span::styled(format!("{}:", coin.symbol), Style::default().fg(label)));
            spans.push(Span::styled(format!(" ${:.2}  ", coin.price), Style::default().fg(value)));
        }
    }

    let ticker = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(ticker_bg));
    frame.render_widget(ticker, area);
}
fn render_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => bg_sidebar, border_dim, accent, label, value, bg_nav_active);
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(border_dim))
        .style(Style::default().bg(bg_sidebar));
    frame.render_widget(block, area);

    if app.sidebar_collapsed {
        let inner = shrink(area, 0, 1, 0, 0);
        let mut lines: Vec<Line> = vec![
            Line::from(""),
            Line::from(Span::styled("  \u{25c8}", Style::default().fg(accent).add_modifier(Modifier::BOLD))),
            Line::from(""),
        ];
        for (i, item) in app.sidebar_items.iter().enumerate() {
            let style = if i == app.selected_sidebar {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(label)
            };
            lines.push(Line::from(Span::styled(format!("  {}", item.icon), style)));
        }
        let p = Paragraph::new(lines);
        frame.render_widget(p, inner);
        return;
    }

    let inner = shrink(area, 0, 1, 0, 0);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(10),
        ])
        .split(inner);

    let top_lines = vec![
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("  \u{25c8} ", Style::default().fg(accent)),
            Span::styled("Orbit", Style::default().fg(value).add_modifier(Modifier::BOLD)),
            Span::styled("Fla", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
            Span::styled("re", Style::default().fg(value).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("  {}", app.user_display), Style::default().fg(value)),
        ]),
        Line::from(vec![
            Span::styled(
                if app.email.is_empty() { "".into() } else { format!("  {}", app.email) },
                Style::default().fg(label),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("  ${:.2} {}", app.balance, app.currency),
                Style::default().fg(accent),
            ),
        ]),
    ];
    let top = Paragraph::new(top_lines);
    frame.render_widget(top, sections[0]);

    let sep = Paragraph::new(Line::from(Span::styled(
        " \u{2500}".to_string() + &"\u{2500}".repeat(sections[1].width.saturating_sub(2) as usize),
        Style::default().fg(border_dim),
    )));
    frame.render_widget(sep, sections[1]);

    let nav_area = sections[2];
    for (i, item) in app.sidebar_items.iter().enumerate() {
        let y = nav_area.y + (i as u16) * 2;
        if y >= nav_area.y + nav_area.height {
            break;
        }
        let item_rect = Rect {
            x: nav_area.x,
            y,
            width: nav_area.width,
            height: 1,
        };

        if i == app.selected_sidebar {
            frame.render_widget(
                Block::default().style(Style::default().bg(bg_nav_active)),
                item_rect,
            );
            let line = Line::from(vec![
                Span::styled(
                    format!("  {} ", item.icon),
                    Style::default().fg(accent).bg(bg_nav_active).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    item.label.to_string(),
                    Style::default().fg(accent).bg(bg_nav_active).add_modifier(Modifier::BOLD),
                ),
            ]);
            frame.render_widget(Paragraph::new(line), item_rect);
        } else {
            let line = Line::from(vec![
                Span::styled(format!("  {} ", item.icon), Style::default().fg(label)),
                Span::styled(item.label, Style::default().fg(label)),
            ]);
            frame.render_widget(Paragraph::new(line), item_rect);
        }
    }

    let bottom = sections[3];
    let bottom_parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(bottom);

    let sep_line = Paragraph::new(Line::from(Span::styled(
        " \u{2500}".to_string() + &"\u{2500}".repeat(bottom_parts[0].width.saturating_sub(2) as usize),
        Style::default().fg(border_dim),
    )));
    frame.render_widget(sep_line, bottom_parts[0]);

    let theme_widget = Paragraph::new(Line::from(vec![
        Span::styled("  \u{f53f} ", Style::default().fg(label)),
        Span::styled("Theme", Style::default().fg(label)),
        Span::styled(
            format!("{:>w$}", app.theme.label(), w = (bottom_parts[1].width as usize).saturating_sub(11)),
            Style::default().fg(app.theme.dot_color()),
        ),
    ]));
    frame.render_widget(theme_widget, bottom_parts[1]);

}
fn render_main(frame: &mut Frame, app: &App, area: Rect) {
    match app.section {
        Section::Home => render_main_home(frame, app, area),
        Section::Services => render_main_services(frame, app, area),
        Section::ServiceDetail => render_main_service_detail(frame, app, area),
        Section::Invoices => render_main_invoices(frame, app, area),
        Section::ApiKeys => render_main_api_keys(frame, app, area),
        Section::Profile => render_main_profile(frame, app, area),
    }
}

fn render_main_home(frame: &mut Frame, app: &App, area: Rect) {
    let has_profile = !app.wallet_address.is_empty() || !app.payment_wallet_address.is_empty();
    let profile_height = if has_profile { 4 } else { 0 };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(5),
            Constraint::Length(profile_height),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(frame, app, layout[0]);
    render_balance_bar(frame, app, layout[1]);
    if has_profile {
        render_profile_bar(frame, app, layout[2]);
    }
    render_cards_row(frame, app, layout[3]);
    render_footer(frame, app, layout[4]);
}
fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => bg, label, border_dim, value, accent);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(30),
            Constraint::Length(32),
        ])
        .split(area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  \u{25a3}\u{25a3} ", Style::default().fg(label)),
            Span::styled("Dashboard ", Style::default().fg(label)),
            Span::styled("> ", Style::default().fg(border_dim)),
            Span::styled("Home", Style::default().fg(value)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  \u{2728} ", Style::default().fg(accent)),
            Span::styled(
                &app.greeting,
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Here's what's happening with your services.",
                Style::default().fg(label),
            ),
        ]),
    ];
    let header = Paragraph::new(lines).style(Style::default().bg(bg));
    frame.render_widget(header, cols[0]);

    let btn_lines = vec![
        Line::from(""),
        Line::from(""),
        Line::from(""),
    ];
    let btns = Paragraph::new(btn_lines)
        .style(Style::default().bg(bg))
        .alignment(Alignment::Right);
    frame.render_widget(btns, cols[1]);
}
fn render_balance_bar(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => accent, label, value, bg_card, border_dim);
    let block = card_block("", app.focus == Panel::Balance, accent, border_dim, bg_card);
    let inner = block.inner(area);

    frame.render_widget(block, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(30)])
        .split(inner);

    let left_lines = vec![
        Line::from(vec![
            Span::styled("  \u{25a3} ", Style::default().fg(accent)),
            Span::styled("ACCOUNT BALANCE", Style::default().fg(label)),
        ]),
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                format!("${:.2}", app.balance),
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", app.currency),
                Style::default().fg(label),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(left_lines), cols[0]);
}
fn render_profile_bar(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => accent, label, value, bg_card, border_dim);
    let block = card_block("", false, accent, border_dim, bg_card);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut spans_line1: Vec<Span> = vec![
        Span::styled("  \u{f2bb} ", Style::default().fg(accent)),
    ];
    if !app.wallet_address.is_empty() {
        spans_line1.push(Span::styled("Wallet: ", Style::default().fg(label)));
        spans_line1.push(Span::styled(&app.wallet_address, Style::default().fg(value)));
    }
    if !app.payment_wallet_address.is_empty() {
        spans_line1.push(Span::styled("    Payment: ", Style::default().fg(label)));
        spans_line1.push(Span::styled(&app.payment_wallet_address, Style::default().fg(value)));
    }

    let mut spans_line2: Vec<Span> = vec![
        Span::styled("    ", Style::default()),
    ];
    if app.user_id > 0 {
        spans_line2.push(Span::styled(format!("ID: {}  ", app.user_id), Style::default().fg(label)));
    }
    if !app.created_at.is_empty() && app.created_at != "—" {
        spans_line2.push(Span::styled(format!("Member since: {}", app.created_at), Style::default().fg(label)));
    }

    let lines = vec![
        Line::from(spans_line1),
        Line::from(spans_line2),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}
fn render_cards_row(frame: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55),
            Constraint::Percentage(45),
        ])
        .split(area);
    render_service_card(frame, app, cols[0]);
    render_invoices_card(frame, app, cols[1]);
}

fn card_block(title: &str, focused: bool, accent: Color, border_dim: Color, bg_card: Color) -> Block<'_> {
    let border_color = if focused { accent } else { border_dim };
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(bg_card));
    if !title.is_empty() {
        block = block
            .title(format!(" {title} "))
            .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD));
    }
    block
}

fn render_service_card(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => accent, label, value, bg_card, border_dim);
    let s = &app.service;
    let block = card_block("", app.focus == Panel::Service, accent, border_dim, bg_card);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let card_height = inner.height;
    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("  {}", s.key_name),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled("  License Key", Style::default().fg(label))),
        Line::from(vec![
            Span::styled(
                format!("  {}", s.license_key),
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Issue Date", Style::default().fg(label))),
        Line::from(vec![
            Span::styled(
                format!("  {}", s.issue_date),
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Next Renewal", Style::default().fg(label))),
        Line::from(vec![
            Span::styled(
                format!("  {}", s.renewal_date),
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let used = lines.len() as u16;
    if card_height > used + 2 {
        for _ in 0..(card_height - used - 2) {
            lines.push(Line::from(""));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  [ View Details ] ", Style::default().fg(accent)),
    ]));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_invoices_card(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => accent, label, value, bg_card, border_dim, success, yellow);
    let focused = app.focus == Panel::Invoices;
    let border_color = if focused { accent } else { border_dim };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(bg_card));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Recent Invoices",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))),
        Rect { height: 1, ..inner },
    );

    let content_area = Rect {
        y: inner.y + 2,
        height: inner.height.saturating_sub(2),
        ..inner
    };

    if app.invoices.is_empty() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("  INVOICE", Style::default().fg(label)),
                Span::styled("     AMOUNT", Style::default().fg(label)),
                Span::styled("     DATE", Style::default().fg(label)),
                Span::styled("       STATUS", Style::default().fg(label)),
            ]),
            Line::from(""),
        ];
        let remaining = content_area.height.saturating_sub(3);
        for _ in 0..remaining / 2 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            "         No invoices found",
            Style::default().fg(label),
        )));

        frame.render_widget(Paragraph::new(lines), content_area);
    } else {
        let mut lines = vec![Line::from(vec![
            Span::styled("  INVOICE", Style::default().fg(label)),
            Span::styled("     AMOUNT", Style::default().fg(label)),
            Span::styled("     DATE", Style::default().fg(label)),
            Span::styled("       STATUS", Style::default().fg(label)),
        ])];
        for inv in &app.invoices {
            let status_color = match inv.status {
                super::models::InvoiceStatus::Paid => success,
                super::models::InvoiceStatus::Pending => yellow,
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<10}", inv.id), Style::default().fg(value)),
                Span::styled(format!("{:<10}", inv.amount), Style::default().fg(value)),
                Span::styled(format!("{:<14}", inv.date), Style::default().fg(label)),
                Span::styled(inv.status.label(), Style::default().fg(status_color)),
            ]));
        }
        frame.render_widget(Paragraph::new(lines), content_area);
    }
}
fn render_main_profile(frame: &mut Frame, app: &App, area: Rect) {
    if app.profile_editing {
        render_profile_edit(frame, app, area);
    } else {
        render_profile_view(frame, app, area);
    }
}

fn render_profile_view(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => bg, accent, border_dim, label, value, success);
    let cw = (area.width as usize).saturating_sub(6);

    let has_status = app.status_message.is_some();
    let extra = if has_status { 1 } else { 0 };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(extra as u16),
            Constraint::Length(1),
        ])
        .split(area);

    let header = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  \u{25a3}\u{25a3} ", Style::default().fg(label)),
            Span::styled("Dashboard", Style::default().fg(label)),
            Span::styled(" > ", Style::default().fg(border_dim)),
            Span::styled("Profile", Style::default().fg(value).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Profile", Style::default().fg(value).add_modifier(Modifier::BOLD)),
        ]),
    ];
    frame.render_widget(Paragraph::new(header).style(Style::default().bg(bg)), layout[0]);

    let mut lines: Vec<Line> = Vec::new();

    card_top(&mut lines, cw, border_dim);
    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled("Personal Details", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
    ]));
    card_empty(&mut lines, border_dim);

    let fields: Vec<(&str, &str)> = vec![
        ("Full Name", &app.user_display),
        ("Email", &app.email),
    ];
    for (lbl, val) in &fields {
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
            Span::styled(format!("{:<20}", lbl), Style::default().fg(label)),
            Span::styled(*val, Style::default().fg(value)),
        ]));
    }
    if !app.wallet_address.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
            Span::styled(format!("{:<20}", "Wallet"), Style::default().fg(label)),
            Span::styled(&app.wallet_address, Style::default().fg(value)),
        ]));
    }
    if !app.payment_wallet_address.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
            Span::styled(format!("{:<20}", "Payment Wallet"), Style::default().fg(label)),
            Span::styled(&app.payment_wallet_address, Style::default().fg(value)),
        ]));
    }
    if app.user_id > 0 {
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
            Span::styled(format!("{:<20}", "User ID"), Style::default().fg(label)),
            Span::styled(format!("{}", app.user_id), Style::default().fg(value)),
        ]));
    }
    if !app.created_at.is_empty() && app.created_at != "\u{2014}" {
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
            Span::styled(format!("{:<20}", "Member Since"), Style::default().fg(label)),
            Span::styled(&app.created_at, Style::default().fg(value)),
        ]));
    }
    card_bottom(&mut lines, cw, border_dim);
    lines.push(Line::from(""));

    card_top(&mut lines, cw, border_dim);
    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled("Connected Accounts", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
    ]));
    card_empty(&mut lines, border_dim);

    if app.connected_accounts.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
            Span::styled("No connected accounts.", Style::default().fg(label)),
        ]));
    } else {
        for ca in &app.connected_accounts {
            let icon = match ca.provider.as_str() {
                "Discord" => "\u{f392}",
                "GitHub" => "\u{f09b}",
                _ => "\u{f0c1}",
            };
            let status_color = if ca.connected { success } else { label };
            let status_label = if ca.connected { "\u{2713} Connected" } else { "\u{2717} Not connected" };

            lines.push(Line::from(vec![
                Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
                Span::styled(format!("{} ", icon), Style::default().fg(accent)),
                Span::styled(format!("{:<12}", ca.provider), Style::default().fg(value).add_modifier(Modifier::BOLD)),
                Span::styled(status_label, Style::default().fg(status_color)),
            ]));
            card_empty(&mut lines, border_dim);
        }
    }
    card_bottom(&mut lines, cw, border_dim);

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(bg)),
        layout[1],
    );

    if has_status {
        let success_color = app.theme.palette().success;
        if let Some(ref msg) = app.status_message {
            let color = if app.status_is_error { Color::Rgb(200, 80, 80) } else { success_color };
            let icon = if app.status_is_error { "\u{2717} " } else { "\u{2713} " };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(icon, Style::default().fg(color)),
                    Span::styled(msg.as_str(), Style::default().fg(color)),
                ])).style(Style::default().bg(bg)),
                layout[2],
            );
        }
    }

    let keys = vec![
        ("[q]", "Quit"),
        ("[Esc]", "Back"),
        ("[e]", "Edit"),
        ("[t]", "Theme"),
    ];
    let mut spans: Vec<Span> = vec![Span::styled(" ", Style::default())];
    for (key, desc) in keys {
        spans.push(Span::styled(format!(" {key} "), Style::default().fg(accent).add_modifier(Modifier::BOLD)));
        spans.push(Span::styled(format!("{desc}  "), Style::default().fg(label)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(app.theme.palette().bg_sidebar)),
        layout[3],
    );
}

fn render_profile_edit(frame: &mut Frame, app: &App, area: Rect) {
    use super::app::ProfileField;
    theme!(app => bg, accent, border_dim, label, value);

    let has_status = app.status_message.is_some();
    let extra = if has_status { 1 } else { 0 };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(12),
            Constraint::Length(extra as u16),
            Constraint::Length(1),
        ])
        .split(area);

    let header = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  \u{25a3}\u{25a3} ", Style::default().fg(label)),
            Span::styled("Dashboard", Style::default().fg(label)),
            Span::styled(" > ", Style::default().fg(border_dim)),
            Span::styled("Profile", Style::default().fg(label)),
            Span::styled(" > ", Style::default().fg(border_dim)),
            Span::styled("Edit", Style::default().fg(value).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Edit Profile", Style::default().fg(value).add_modifier(Modifier::BOLD)),
        ]),
    ];
    frame.render_widget(Paragraph::new(header).style(Style::default().bg(bg)), layout[0]);

    let form_area = shrink(layout[1], 2, 2, 0, 0);
    let cw = (form_area.width as usize).saturating_sub(4);

    let name_active = app.profile_field == ProfileField::Name;
    let wallet_active = app.profile_field == ProfileField::Wallet;
    let name_border = if name_active { accent } else { border_dim };
    let wallet_border = if wallet_active { accent } else { border_dim };

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("  Full Name", Style::default().fg(if name_active { accent } else { label }).add_modifier(if name_active { Modifier::BOLD } else { Modifier::empty() })),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  \u{256d}", Style::default().fg(name_border)),
        Span::styled("\u{2500}".repeat(cw), Style::default().fg(name_border)),
        Span::styled("\u{256e}", Style::default().fg(name_border)),
    ]));
    {
        let buf = &app.profile_name_buf;
        let cursor = if name_active { app.profile_cursor } else { buf.len() };
        let (before, after) = buf.split_at(cursor.min(buf.len()));
        let mut spans = vec![
            Span::styled("  \u{2502} ", Style::default().fg(name_border)),
            Span::styled(before, Style::default().fg(value)),
        ];
        if name_active {
            spans.push(Span::styled("\u{2588}", Style::default().fg(accent)));
        }
        spans.push(Span::styled(after, Style::default().fg(value)));
        lines.push(Line::from(spans));
    }
    lines.push(Line::from(vec![
        Span::styled("  \u{2570}", Style::default().fg(name_border)),
        Span::styled("\u{2500}".repeat(cw), Style::default().fg(name_border)),
        Span::styled("\u{256f}", Style::default().fg(name_border)),
    ]));
    lines.push(Line::from(""));

    lines.push(Line::from(vec![
        Span::styled("  Payment Wallet Address", Style::default().fg(if wallet_active { accent } else { label }).add_modifier(if wallet_active { Modifier::BOLD } else { Modifier::empty() })),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  \u{256d}", Style::default().fg(wallet_border)),
        Span::styled("\u{2500}".repeat(cw), Style::default().fg(wallet_border)),
        Span::styled("\u{256e}", Style::default().fg(wallet_border)),
    ]));
    {
        let buf = &app.profile_wallet_buf;
        let cursor = if wallet_active { app.profile_cursor } else { buf.len() };
        let (before, after) = buf.split_at(cursor.min(buf.len()));
        let mut spans = vec![
            Span::styled("  \u{2502} ", Style::default().fg(wallet_border)),
            Span::styled(before, Style::default().fg(value)),
        ];
        if wallet_active {
            spans.push(Span::styled("\u{2588}", Style::default().fg(accent)));
        }
        spans.push(Span::styled(after, Style::default().fg(value)));
        lines.push(Line::from(spans));
    }
    lines.push(Line::from(vec![
        Span::styled("  \u{2570}", Style::default().fg(wallet_border)),
        Span::styled("\u{2500}".repeat(cw), Style::default().fg(wallet_border)),
        Span::styled("\u{256f}", Style::default().fg(wallet_border)),
    ]));

    frame.render_widget(Paragraph::new(lines).style(Style::default().bg(bg)), form_area);

    if has_status {
        let success_color = app.theme.palette().success;
        if let Some(ref msg) = app.status_message {
            let color = if app.status_is_error { Color::Rgb(200, 80, 80) } else { success_color };
            let icon = if app.status_is_error { "\u{2717} " } else { "\u{2713} " };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(icon, Style::default().fg(color)),
                    Span::styled(msg.as_str(), Style::default().fg(color)),
                ])).style(Style::default().bg(bg)),
                layout[2],
            );
        }
    }

    let keys = vec![
        ("[Esc]", "Cancel"),
        ("[Tab]", "Switch Field"),
        ("[Enter]", "Save"),
    ];
    let mut spans: Vec<Span> = vec![Span::styled(" ", Style::default())];
    for (key, desc) in keys {
        spans.push(Span::styled(format!(" {key} "), Style::default().fg(accent).add_modifier(Modifier::BOLD)));
        spans.push(Span::styled(format!("{desc}  "), Style::default().fg(label)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(app.theme.palette().bg_sidebar)),
        layout[3],
    );
}

fn render_main_invoices(frame: &mut Frame, app: &App, area: Rect) {
    let has_status = app.status_message.is_some() && app.section == Section::Invoices;
    let has_confirm = app.confirm.is_some() && app.section == Section::Invoices;
    let extra_rows = if has_status { 1 } else { 0 }
        + if has_confirm { 3 } else { 0 };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(6),
            Constraint::Length(extra_rows as u16),
            Constraint::Length(1),
        ])
        .split(area);

    render_invoices_header_section(frame, app, layout[0]);

    if app.invoices.is_empty() {
        render_invoices_empty_card(frame, app, layout[1]);
    } else {
        render_invoices_list(frame, app, layout[1]);
    }

    if extra_rows > 0 {
        theme!(app => label, yellow, bg);
        let success_color = app.theme.palette().success;
        let overlay = layout[2];
        let mut overlay_lines: Vec<Line> = Vec::new();

        if let Some(ref msg) = app.status_message {
            let color = if app.status_is_error { Color::Rgb(200, 80, 80) } else { success_color };
            let icon = if app.status_is_error { "\u{2717} " } else { "\u{2713} " };
            overlay_lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(icon, Style::default().fg(color)),
                Span::styled(msg.as_str(), Style::default().fg(color)),
            ]));
        }

        if let Some(ref kind) = app.confirm {
            let prompt = match kind {
                super::app::ConfirmKind::PayInvoice => "Pay this invoice with your account balance?",
                _ => "Are you sure?",
            };
            overlay_lines.push(Line::from(""));
            overlay_lines.push(Line::from(vec![
                Span::styled("  \u{26a0} ", Style::default().fg(yellow)),
                Span::styled(prompt, Style::default().fg(yellow)),
                Span::styled("  [y] Yes  [any] Cancel", Style::default().fg(label)),
            ]));
            overlay_lines.push(Line::from(""));
        }

        frame.render_widget(
            Paragraph::new(overlay_lines).style(Style::default().bg(bg)),
            overlay,
        );
    }

    render_footer_invoices(frame, app, layout[3]);
}

fn render_invoices_header_section(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => bg, label, border_dim, value, accent);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(30),
            Constraint::Length(20),
        ])
        .split(area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  \u{25a3}\u{25a3} ", Style::default().fg(label)),
            Span::styled("Dashboard ", Style::default().fg(label)),
            Span::styled("> ", Style::default().fg(border_dim)),
            Span::styled("Invoices ", Style::default().fg(label)),
            Span::styled("> ", Style::default().fg(border_dim)),
            Span::styled("Invoices", Style::default().fg(value).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Invoices",
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  View and manage all your invoices and billing history.",
                Style::default().fg(label),
            ),
        ]),
    ];
    let header = Paragraph::new(lines).style(Style::default().bg(bg));
    frame.render_widget(header, cols[0]);

    let view_lines = vec![
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " \u{25a3}\u{25a3} ",
                if app.invoices_view == ViewMode::Grid {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(label)
                },
            ),
            Span::styled(
                " \u{2261} ",
                if app.invoices_view == ViewMode::List {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(label)
                },
            ),
            Span::styled(
                " \u{25a1} ",
                if app.invoices_view == ViewMode::Compact {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(label)
                },
            ),
            Span::styled(" ", Style::default()),
        ]),
    ];
    let view_btns = Paragraph::new(view_lines)
        .style(Style::default().bg(bg))
        .alignment(Alignment::Right);
    frame.render_widget(view_btns, cols[1]);
}

fn render_invoices_empty_card(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => accent, label, bg_card, border_dim);
    let padded = shrink(area, 1, 1, 0, 1);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_dim))
        .style(Style::default().bg(bg_card));
    let inner = block.inner(padded);
    frame.render_widget(block, padded);

    if inner.height < 5 {
        return;
    }

    let content_height: u16 = 5;
    let top_pad = inner.height.saturating_sub(content_height) / 2;

    let mut lines: Vec<Line> = Vec::new();
    for _ in 0..top_pad {
        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled(
            "[$]",
            Style::default().fg(label),
        ),
    ]));
    lines.push(Line::from(""));

    lines.push(Line::from(vec![
        Span::styled(
            "No Invoices Found",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "You don't have any invoices yet. Missing an invoice? Contact us.",
            Style::default().fg(label),
        ),
    ]));

    let content = Paragraph::new(lines)
        .alignment(Alignment::Center);
    frame.render_widget(content, inner);
}

fn render_invoices_list(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => bg, accent, border_dim, label, value, success, yellow);
    let padded = shrink(area, 1, 1, 0, 1);
    let cw = (padded.width as usize).saturating_sub(4);

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(
            format!("  {:<20} {:<12} {:<14} {}",
                "REFERENCE", "AMOUNT", "STATUS", "DATE"
            ),
            Style::default().fg(label),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(format!("  {}", "\u{2500}".repeat(cw)), Style::default().fg(border_dim)),
    ]));

    for (i, inv) in app.invoices.iter().enumerate() {
        let is_selected = i == app.invoices_selected;
        let marker = if is_selected { "\u{25b6} " } else { "  " };
        let row_fg = if is_selected { accent } else { value };

        let (status_label, status_color) = match inv.status {
            super::models::InvoiceStatus::Paid => ("\u{25c9} Paid", success),
            super::models::InvoiceStatus::Pending => ("\u{25cb} Pending", yellow),
        };

        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(accent)),
            Span::styled(format!("{:<20}", inv.id), Style::default().fg(row_fg)),
            Span::styled(format!("{:<12}", inv.amount), Style::default().fg(row_fg)),
            Span::styled(format!("{:<14}", status_label), Style::default().fg(status_color)),
            Span::styled(&inv.date, Style::default().fg(label)),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(bg)),
        padded,
    );
}

fn render_footer_invoices(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => accent, label, bg_sidebar);
    let keys: Vec<(&str, &str)> = if app.confirm.is_some() {
        vec![
            ("[y]", "Confirm"),
            ("[any]", "Cancel"),
        ]
    } else {
        let mut k = vec![
            ("[q]", "Quit"),
            ("[Esc]", "Back"),
            ("[↑↓]", "Select"),
            ("[Tab]", "View"),
        ];
        if let Some(inv) = app.invoices.get(app.invoices_selected)
            && inv.status.label() == "Pending"
        {
            k.push(("[p]", "Pay"));
        }
        k.push(("[R]", "Refresh"));
        k.push(("[t]", "Theme"));
        k
    };

    let mut spans: Vec<Span> = vec![Span::styled(" ", Style::default())];
    for (key, desc) in keys {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{desc}  "),
            Style::default().fg(label),
        ));
    }

    let footer = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(bg_sidebar));
    frame.render_widget(footer, area);
}
fn render_main_services(frame: &mut Frame, app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(1),
        ])
        .split(area);

    render_services_header(frame, app, layout[0]);
    render_services_filter_bar(frame, app, layout[1]);
    match app.services_view {
        ViewMode::Grid => render_services_grid(frame, app, layout[2]),
        ViewMode::List => render_services_table(frame, app, layout[2]),
        ViewMode::Compact => render_services_compact(frame, app, layout[2]),
    }
    render_footer_services(frame, app, layout[3]);
}

fn render_services_header(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => bg, label, border_dim, value, accent);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(30),
            Constraint::Length(20),
        ])
        .split(area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  \u{25a3}\u{25a3} ", Style::default().fg(label)),
            Span::styled("Dashboard ", Style::default().fg(label)),
            Span::styled("> ", Style::default().fg(border_dim)),
            Span::styled("Services ", Style::default().fg(label)),
            Span::styled("> ", Style::default().fg(border_dim)),
            Span::styled("Your Services", Style::default().fg(value).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Your Services",
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Manage your active services and renewals.",
                Style::default().fg(label),
            ),
        ]),
    ];
    let header = Paragraph::new(lines).style(Style::default().bg(bg));
    frame.render_widget(header, cols[0]);

    let view_lines = vec![
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " \u{25a3}\u{25a3} ",
                if app.services_view == ViewMode::Grid {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(label)
                },
            ),
            Span::styled(
                " \u{2261} ",
                if app.services_view == ViewMode::List {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(label)
                },
            ),
            Span::styled(
                " \u{25a1} ",
                if app.services_view == ViewMode::Compact {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(label)
                },
            ),
            Span::styled(" ", Style::default()),
        ]),
    ];
    let view_btns = Paragraph::new(view_lines)
        .style(Style::default().bg(bg))
        .alignment(Alignment::Right);
    frame.render_widget(view_btns, cols[1]);
}

fn render_services_filter_bar(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => bg, green_dot, value, label);
    let active_count = app.services.iter()
        .filter(|s| s.status == ServiceStatus::Active)
        .count();
    let total = app.services.len();

    let filter_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(" \u{25cf} ", Style::default().fg(green_dot)),
            Span::styled(
                format!("Active: {}", active_count),
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("   {} total", total),
                Style::default().fg(label),
            ),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(filter_lines).style(Style::default().bg(bg)),
        area,
    );
}

fn render_services_table(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => bg, bg_card, bg_nav_active, accent, border_dim, label, value, success, yellow);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_dim))
        .style(Style::default().bg(bg_card));
    let inner = block.inner(area);
    frame.render_widget(block, shrink(area, 0, 1, 0, 1));

    if inner.width < 10 || inner.height < 3 {
        return;
    }

    let col_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(28),
            Constraint::Percentage(12),
            Constraint::Percentage(14),
            Constraint::Percentage(14),
            Constraint::Percentage(12),
        ])
        .split(inner);

    let header_y = inner.y;
    let header_labels = ["SERVICE", "LICENSE KEY", "STATUS", "ISSUE DATE", "RENEWAL", "ACTIONS"];
    for (i, lbl) in header_labels.iter().enumerate() {
        let rect = Rect {
            x: col_layout[i].x,
            y: header_y,
            width: col_layout[i].width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  {}", lbl),
                Style::default().fg(label),
            ))),
            rect,
        );
    }

    let sep_y = header_y + 1;
    if sep_y < inner.y + inner.height {
        let sep_rect = Rect {
            x: inner.x,
            y: sep_y,
            width: inner.width,
            height: 1,
        };
        let sep = Paragraph::new(Line::from(Span::styled(
            "\u{2500}".repeat(inner.width as usize),
            Style::default().fg(border_dim),
        )));
        frame.render_widget(sep, sep_rect);
    }

    let data_start_y = sep_y + 1;
    for (row_idx, svc) in app.services.iter().enumerate() {
        let row_y = data_start_y + (row_idx as u16) * 2;
        if row_y >= inner.y + inner.height {
            break;
        }

        let is_selected = row_idx == app.services_selected;

        if is_selected {
            let row_rect = Rect {
                x: inner.x,
                y: row_y,
                width: inner.width,
                height: 1,
            };
            frame.render_widget(
                Block::default().style(Style::default().bg(bg_nav_active)),
                row_rect,
            );
        }

        let row_bg = if is_selected { bg_nav_active } else { bg_card };

        let svc_rect = Rect {
            x: col_layout[0].x,
            y: row_y,
            width: col_layout[0].width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  ", Style::default().bg(row_bg)),
                Span::styled(
                    "\u{25c8} ",
                    Style::default().fg(accent).bg(row_bg),
                ),
                Span::styled(
                    svc.name.clone(),
                    Style::default().fg(value).bg(row_bg).add_modifier(Modifier::BOLD),
                ),
            ])),
            svc_rect,
        );

        let key_rect = Rect {
            x: col_layout[1].x,
            y: row_y,
            width: col_layout[1].width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("  {}", svc.license_key),
                    Style::default().fg(value).bg(row_bg),
                ),
                Span::styled(
                    " \u{f0c5}",
                    Style::default().fg(label).bg(row_bg),
                ),
            ])),
            key_rect,
        );

        let status_rect = Rect {
            x: col_layout[2].x,
            y: row_y,
            width: col_layout[2].width,
            height: 1,
        };
        let status_color = match svc.status {
            ServiceStatus::Active => success,
            ServiceStatus::Expired => Color::Rgb(200, 80, 80),
            ServiceStatus::Suspended => yellow,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  ", Style::default().bg(row_bg)),
                Span::styled(
                    svc.status.label(),
                    Style::default().fg(status_color).bg(row_bg),
                ),
            ])),
            status_rect,
        );

        let issue_rect = Rect {
            x: col_layout[3].x,
            y: row_y,
            width: col_layout[3].width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  {}", svc.issue_date),
                Style::default().fg(value).bg(row_bg),
            ))),
            issue_rect,
        );

        let renewal_rect = Rect {
            x: col_layout[4].x,
            y: row_y,
            width: col_layout[4].width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  {}", svc.renewal_date),
                Style::default().fg(value).bg(row_bg),
            ))),
            renewal_rect,
        );

        let actions_rect = Rect {
            x: col_layout[5].x,
            y: row_y,
            width: col_layout[5].width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  ", Style::default().bg(row_bg)),
                Span::styled(
                    " View ",
                    Style::default().fg(bg).bg(accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "  \u{22ee}",
                    Style::default().fg(label).bg(row_bg),
                ),
            ])),
            actions_rect,
        );
    }
}
fn render_services_grid(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => bg, bg_card, accent, border_dim, label, value, success, yellow);
    if app.services.is_empty() {
        let empty = Paragraph::new("  No services found.")
            .style(Style::default().fg(label).bg(bg));
        frame.render_widget(empty, area);
        return;
    }

    let card_width = area.width.min(50);
    let cards_per_row = (area.width / card_width).max(1) as usize;
    let card_height: u16 = 10;

    for (i, svc) in app.services.iter().enumerate() {
        let row = i / cards_per_row;
        let col = i % cards_per_row;
        let x = area.x + (col as u16) * card_width;
        let y = area.y + (row as u16) * (card_height + 1);

        if y + card_height > area.y + area.height {
            break;
        }

        let card_area = Rect {
            x: x + 1,
            y,
            width: card_width.min(area.x + area.width - x) - 2,
            height: card_height,
        };

        let is_selected = i == app.services_selected;
        let border_color = if is_selected { accent } else { border_dim };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(bg_card));
        let inner = block.inner(card_area);
        frame.render_widget(block, card_area);

        if inner.height < 6 || inner.width < 20 {
            continue;
        }

        let status_color = match svc.status {
            ServiceStatus::Active => success,
            ServiceStatus::Expired => Color::Rgb(200, 80, 80),
            ServiceStatus::Suspended => yellow,
        };

        let lines = vec![
            Line::from(vec![
                Span::styled(
                    format!("  {}", svc.name),
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!(" {} ", svc.status.label()),
                    Style::default().fg(status_color),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  License Key", Style::default().fg(label)),
            ]),
            Line::from(vec![
                Span::styled(
                    format!("  {}", svc.license_key),
                    Style::default().fg(value).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Issue Date       ", Style::default().fg(label)),
            ]),
            Line::from(vec![
                Span::styled(
                    format!("  {:<16}   Next: {}", svc.issue_date, svc.renewal_date),
                    Style::default().fg(value).add_modifier(Modifier::BOLD),
                ),
            ]),
        ];

        let content = Paragraph::new(lines).style(Style::default().bg(bg_card));
        frame.render_widget(content, inner);
    }
}
fn render_services_compact(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => bg, bg_card, accent, border_dim, label, value, success, yellow);
    if app.services.is_empty() {
        let empty = Paragraph::new("  No services found.")
            .style(Style::default().fg(label).bg(bg));
        frame.render_widget(empty, area);
        return;
    }

    let row_height: u16 = 4;

    for (i, svc) in app.services.iter().enumerate() {
        let y = area.y + (i as u16) * (row_height + 1);
        if y + row_height > area.y + area.height {
            break;
        }

        let card_area = Rect {
            x: area.x + 1,
            y,
            width: area.width.saturating_sub(2),
            height: row_height,
        };

        let is_selected = i == app.services_selected;
        let border_color = if is_selected { accent } else { border_dim };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(bg_card));
        let inner = block.inner(card_area);
        frame.render_widget(block, card_area);

        if inner.height < 2 || inner.width < 20 {
            continue;
        }

        let status_color = match svc.status {
            ServiceStatus::Active => success,
            ServiceStatus::Expired => Color::Rgb(200, 80, 80),
            ServiceStatus::Suspended => yellow,
        };

        let line1 = Line::from(vec![
            Span::styled(
                format!("  {}", svc.name),
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
            Span::styled("    ", Style::default()),
            Span::styled(
                format!(" {} ", svc.status.label()),
                Style::default().fg(status_color),
            ),
        ]);

        let line2 = Line::from(vec![
            Span::styled(
                format!("  {}", svc.license_key),
                Style::default().fg(label),
            ),
        ]);

        let line3 = Line::from(vec![
            Span::styled(
                format!("  Issued: {}    Renewal: {}", svc.issue_date, svc.renewal_date),
                Style::default().fg(label),
            ),
        ]);

        let lines = if inner.height >= 3 {
            vec![line1, line2, line3]
        } else {
            vec![line1, line2]
        };

        let content = Paragraph::new(lines).style(Style::default().bg(bg_card));
        frame.render_widget(content, inner);
    }
}
fn render_main_api_keys(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => bg, bg_card, accent, border_dim, label, value, success, yellow);
    let has_status = app.status_message.is_some() && app.section == Section::ApiKeys;
    let has_confirm = app.confirm.is_some();
    let has_input = app.input_mode.is_some();
    let extra_rows = if has_status { 1 } else { 0 }
        + if has_confirm { 3 } else { 0 }
        + if has_input { 1 } else { 0 };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(6),
            Constraint::Length(extra_rows as u16),
            Constraint::Length(1),
        ])
        .split(area);

    let header_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  \u{25a3}\u{25a3} ", Style::default().fg(label)),
            Span::styled("Dashboard", Style::default().fg(label)),
            Span::styled(" > ", Style::default().fg(border_dim)),
            Span::styled("API Keys", Style::default().fg(value).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  API Keys",
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Manage your API keys for accessing OrbitFlare services.",
                Style::default().fg(label),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(header_lines).style(Style::default().bg(bg)), layout[0]);

    let content_area = layout[1];

    if app.api_keys.is_empty() {
        let cw = (content_area.width as usize).saturating_sub(6);
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(""));
        card_top(&mut lines, cw, border_dim);
        lines.push(Line::from(vec![
            Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
            Span::styled("\u{f084} ", Style::default().fg(accent)),
            Span::styled(
                "No API Keys",
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
            Span::styled(
                "You haven't created any API keys yet.",
                Style::default().fg(label),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
            Span::styled(
                "Create keys at: orbitflare.com/dashboard/settings/api-keys",
                Style::default().fg(accent),
            ),
        ]));
        card_bottom(&mut lines, cw, border_dim);

        frame.render_widget(
            Paragraph::new(lines).style(Style::default().bg(bg)),
            content_area,
        );
    } else {
        let cw = (content_area.width as usize).saturating_sub(4);
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(""));

        let enabled_count = app.api_keys.iter().filter(|k| k.enabled).count();
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{} key{}", app.api_keys.len(), if app.api_keys.len() != 1 { "s" } else { "" }),
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ({} enabled)", enabled_count),
                Style::default().fg(success),
            ),
        ]));
        lines.push(Line::from(""));

        for (i, key) in app.api_keys.iter().enumerate() {
            let is_selected = i == app.api_keys_selected;
            let is_expanded = app.api_keys_expanded == Some(i);
            let key_border_color = if is_selected { accent } else { border_dim };

            lines.push(Line::from(vec![
                Span::styled("  \u{256d}", Style::default().fg(key_border_color)),
                Span::styled(
                    "\u{2500}".repeat(cw),
                    Style::default().fg(key_border_color),
                ),
                Span::styled("\u{256e}", Style::default().fg(key_border_color)),
            ]));

            let pointer_color = if is_selected { accent } else { label };

            let status_color = if key.enabled { success } else { Color::Rgb(200, 80, 80) };
            let status_label = if key.enabled { " \u{23fb} Enabled " } else { " \u{23fb} Disabled " };
            let ips_label = format!(" \u{2616} IPs({}) ", key.ip_count);

            lines.push(Line::from(vec![
                Span::styled("  \u{2502} ", Style::default().fg(key_border_color)),
                Span::styled("\u{f084} ", Style::default().fg(pointer_color)),
                Span::styled(
                    key.name.clone(),
                    Style::default().fg(value).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  #{}", key.id),
                    Style::default().fg(label),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("  \u{2502}   ", Style::default().fg(key_border_color)),
                Span::styled(
                    key.key_masked.clone(),
                    Style::default().fg(label),
                ),
                Span::styled(
                    " \u{f0c5}",
                    Style::default().fg(border_dim),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("  \u{2502}   ", Style::default().fg(key_border_color)),
                Span::styled(
                    format!("Created {}", key.created_at),
                    Style::default().fg(label),
                ),
            ]));

            let btn_pad = cw.saturating_sub(40);
            lines.push(Line::from(vec![
                Span::styled("  \u{2502} ", Style::default().fg(key_border_color)),
                Span::styled(
                    " ".repeat(btn_pad),
                    Style::default(),
                ),
                Span::styled(
                    status_label,
                    Style::default().fg(status_color),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(
                    ips_label,
                    Style::default().fg(accent).bg(if is_expanded { Color::Rgb(20, 35, 45) } else { bg_card }),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(
                    " \u{21bb} ",
                    Style::default().fg(label),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(
                    " \u{f1f8} ",
                    Style::default().fg(Color::Rgb(200, 80, 80)),
                ),
            ]));

            if is_expanded {
                lines.push(Line::from(vec![
                    Span::styled("  \u{251c}", Style::default().fg(key_border_color)),
                    Span::styled(
                        "\u{2500}".repeat(cw),
                        Style::default().fg(key_border_color),
                    ),
                    Span::styled("\u{2524}", Style::default().fg(key_border_color)),
                ]));

                lines.push(Line::from(vec![
                    Span::styled("  \u{2502} ", Style::default().fg(key_border_color)),
                    Span::styled(
                        "IP Whitelist",
                        Style::default().fg(accent).add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("  \u{2502} ", Style::default().fg(key_border_color)),
                    Span::styled(
                        "When IPs are whitelisted, only requests from those addresses will be accepted.",
                        Style::default().fg(label),
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("  \u{2502}", Style::default().fg(key_border_color)),
                ]));

                if key.ip_whitelist.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("  \u{2502}   ", Style::default().fg(key_border_color)),
                        Span::styled(
                            "No IPs whitelisted  ",
                            Style::default().fg(label),
                        ),
                        Span::styled(
                            "[a] Add IP",
                            Style::default().fg(accent),
                        ),
                    ]));
                } else {
                    for (ip_idx, ip) in key.ip_whitelist.iter().enumerate() {
                        let is_ip_selected = is_expanded && ip_idx == app.api_keys_ip_selected;
                        let chip_bg = if is_ip_selected {
                            Color::Rgb(40, 60, 80)
                        } else {
                            Color::Rgb(30, 40, 55)
                        };
                        let chip_border = if is_ip_selected { accent } else { border_dim };
                        lines.push(Line::from(vec![
                            Span::styled("  \u{2502}   ", Style::default().fg(key_border_color)),
                            if is_ip_selected {
                                Span::styled("\u{25b8} ", Style::default().fg(accent))
                            } else {
                                Span::styled("  ", Style::default())
                            },
                            Span::styled(
                                format!(" {} ", ip),
                                Style::default().fg(value).bg(chip_bg),
                            ),
                            Span::styled(
                                " \u{00d7} ",
                                Style::default().fg(chip_border).bg(chip_bg),
                            ),
                        ]));
                    }
                }

                lines.push(Line::from(vec![
                    Span::styled("  \u{2502}", Style::default().fg(key_border_color)),
                ]));
            }

            lines.push(Line::from(vec![
                Span::styled("  \u{2570}", Style::default().fg(key_border_color)),
                Span::styled(
                    "\u{2500}".repeat(cw),
                    Style::default().fg(key_border_color),
                ),
                Span::styled("\u{256f}", Style::default().fg(key_border_color)),
            ]));
            lines.push(Line::from(""));
        }

        frame.render_widget(
            Paragraph::new(lines).style(Style::default().bg(bg)).scroll((app.api_keys_scroll, 0)),
            content_area,
        );
    }

    if extra_rows > 0 {
        let overlay = layout[2];
        let mut overlay_lines: Vec<Line> = Vec::new();

        if let Some(ref msg) = app.status_message {
            let color = if app.status_is_error { Color::Rgb(200, 80, 80) } else { success };
            let icon = if app.status_is_error { "\u{2717} " } else { "\u{2713} " };
            overlay_lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(icon, Style::default().fg(color)),
                Span::styled(msg.as_str(), Style::default().fg(color)),
            ]));
        }

        if let Some(ref kind) = app.confirm {
            let prompt = match kind {
                super::app::ConfirmKind::Regenerate => "Regenerate this API key? This cannot be undone.",
                super::app::ConfirmKind::Delete => "Delete this API key? This cannot be undone.",
                super::app::ConfirmKind::RemoveIp => "Remove this IP from the whitelist?",
                super::app::ConfirmKind::RegenerateLicenseKey => "Regenerate this license key? This cannot be undone.",
                super::app::ConfirmKind::RemoveLicenseIp => "Remove this IP from the whitelist?",
                super::app::ConfirmKind::RemoveDomain => "Remove this domain?",
                super::app::ConfirmKind::PayInvoice => "Pay this invoice with your account balance?",
            };
            overlay_lines.push(Line::from(""));
            overlay_lines.push(Line::from(vec![
                Span::styled("  \u{26a0} ", Style::default().fg(yellow)),
                Span::styled(prompt, Style::default().fg(yellow)),
                Span::styled("  [y] Yes  [any] Cancel", Style::default().fg(label)),
            ]));
            overlay_lines.push(Line::from(""));
        }

        if let Some(ref target) = app.input_mode {
            let prompt = match target {
                super::app::InputTarget::AddIp => "  IP: ",
                super::app::InputTarget::CreateApiKey => "  Name: ",
                super::app::InputTarget::AddLicenseIp => "  IP: ",
                super::app::InputTarget::AddDomain => "  Domain: ",
            };
            overlay_lines.push(Line::from(vec![
                Span::styled(prompt, Style::default().fg(accent)),
                Span::styled(
                    app.input_buffer.as_str(),
                    Style::default().fg(value).add_modifier(Modifier::BOLD),
                ),
                Span::styled("\u{2588}", Style::default().fg(accent)),
            ]));
        }

        frame.render_widget(
            Paragraph::new(overlay_lines).style(Style::default().bg(bg)),
            overlay,
        );
    }

    if app.loading {
        let loading_area = Rect {
            x: area.x + area.width.saturating_sub(18),
            y: area.y + 3,
            width: 16,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" \u{21bb} Loading... ", Style::default().fg(accent).bg(bg_card)),
            ])),
            loading_area,
        );
    }

    render_footer_api_keys(frame, app, layout[3]);
}

fn render_footer_api_keys(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => accent, label, bg_sidebar);
    let keys: Vec<(&str, &str)> = if app.input_mode.is_some() {
        vec![
            ("[Esc]", "Cancel"),
            ("[Enter]", "Submit"),
        ]
    } else if app.confirm.is_some() {
        vec![
            ("[y]", "Confirm"),
            ("[any]", "Cancel"),
        ]
    } else if app.api_keys_expanded.is_some() {
        vec![
            ("[q]", "Quit"),
            ("[Esc]", "Back"),
            ("[↑↓]", "Select IP"),
            ("[Enter]", "Collapse"),
            ("[a]", "Add IP"),
            ("[d]", "Remove IP"),
            ("[e]", "Toggle"),
            ("[R]", "Refresh"),
            ("[t]", "Theme"),
        ]
    } else {
        vec![
            ("[q]", "Quit"),
            ("[Esc]", "Back"),
            ("[↑↓]", "Select"),
            ("[Enter]", "IPs"),
            ("[c]", "Create"),
            ("[e]", "Toggle"),
            ("[r]", "Regen"),
            ("[x]", "Delete"),
            ("[R]", "Refresh"),
            ("[t]", "Theme"),
        ]
    };

    let mut spans: Vec<Span> = vec![Span::styled(" ", Style::default())];
    for (key, desc) in keys {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{desc}  "),
            Style::default().fg(label),
        ));
    }

    let footer = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(bg_sidebar));
    frame.render_widget(footer, area);
}
const PURPLE: Color = Color::Rgb(180, 130, 255);
const BLUE: Color = Color::Rgb(100, 180, 255);
const RED_WARN: Color = Color::Rgb(220, 100, 80);

fn badge_color(label_text: &str, theme: &Theme) -> Color {
    match label_text {
        "RPC" => theme.success,
        "WebSocket" | "WS" => BLUE,
        "gRPC (Yellowstone)" | "GRPC" => PURPLE,
        "Jetstream" => theme.accent,
        "TX Sender" => theme.yellow,
        _ => theme.label,
    }
}

fn render_main_service_detail(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => bg, accent, border_dim, label, value, yellow);
    let palette = app.theme.palette();
    let svc = match app.services.get(app.services_selected) {
        Some(s) => s,
        None => return,
    };

    let has_status = app.status_message.is_some() && app.section == Section::ServiceDetail;
    let has_confirm = app.confirm.is_some();
    let has_input = app.input_mode.is_some();
    let extra_rows = if has_status { 1 } else { 0 }
        + if has_confirm { 3 } else { 0 }
        + if has_input { 1 } else { 0 };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(extra_rows as u16),
            Constraint::Length(1),
        ])
        .split(area);

    let header_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  \u{25a3}\u{25a3} ", Style::default().fg(label)),
            Span::styled("Dashboard", Style::default().fg(label)),
            Span::styled(" > ", Style::default().fg(border_dim)),
            Span::styled("Services", Style::default().fg(label)),
            Span::styled(" > ", Style::default().fg(border_dim)),
            Span::styled(svc.name.as_str(), Style::default().fg(value).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("  {}", svc.name),
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  #{}", svc.id),
                Style::default().fg(label),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(header_lines).style(Style::default().bg(bg)), layout[0]);

    let tab_line = Line::from(vec![
        Span::styled("  ", Style::default()),
        if app.detail_tab == DetailTab::Details {
            Span::styled(" Details ", Style::default().fg(accent).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" Details ", Style::default().fg(label))
        },
        Span::styled("  ", Style::default()),
        if app.detail_tab == DetailTab::WhitelistedIps {
            Span::styled(" Whitelisted IPs ", Style::default().fg(accent).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" Whitelisted IPs ", Style::default().fg(label))
        },
        Span::styled("  ", Style::default()),
        if app.detail_tab == super::app::DetailTab::Domains {
            Span::styled(" Domains ", Style::default().fg(accent).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" Domains ", Style::default().fg(label))
        },
    ]);
    let underline = Line::from(vec![
        Span::styled("  ", Style::default()),
        if app.detail_tab == DetailTab::Details {
            Span::styled("\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}", Style::default().fg(accent))
        } else {
            Span::styled("         ", Style::default().fg(border_dim))
        },
        Span::styled("  ", Style::default()),
        if app.detail_tab == DetailTab::WhitelistedIps {
            Span::styled("\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}", Style::default().fg(accent))
        } else {
            Span::styled("                 ", Style::default().fg(border_dim))
        },
        Span::styled("  ", Style::default()),
        if app.detail_tab == super::app::DetailTab::Domains {
            Span::styled("\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}", Style::default().fg(accent))
        } else {
            Span::styled("         ", Style::default().fg(border_dim))
        },
    ]);
    frame.render_widget(
        Paragraph::new(vec![tab_line, underline]).style(Style::default().bg(bg)),
        layout[1],
    );

    let content_area = layout[2];
    match app.detail_tab {
        DetailTab::Details => render_detail_tab(frame, app, svc, content_area, &palette),
        DetailTab::WhitelistedIps => render_ips_tab(frame, app, svc, content_area, &palette),
        super::app::DetailTab::Domains => render_domains_tab(frame, app, svc, content_area),
    }

    if extra_rows > 0 {
        let overlay = layout[3];
        let mut overlay_lines: Vec<Line> = Vec::new();

        if let Some(ref msg) = app.status_message {
            let success_color = app.theme.palette().success;
            let color = if app.status_is_error { Color::Rgb(200, 80, 80) } else { success_color };
            let icon = if app.status_is_error { "\u{2717} " } else { "\u{2713} " };
            overlay_lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(icon, Style::default().fg(color)),
                Span::styled(msg.as_str(), Style::default().fg(color)),
            ]));
        }

        if let Some(ref kind) = app.confirm {
            let prompt = match kind {
                super::app::ConfirmKind::RegenerateLicenseKey => "Regenerate this license key? This cannot be undone.",
                super::app::ConfirmKind::RemoveLicenseIp => "Remove this IP from the whitelist?",
                super::app::ConfirmKind::RemoveDomain => "Remove this domain?",
                _ => "Are you sure?",
            };
            overlay_lines.push(Line::from(""));
            overlay_lines.push(Line::from(vec![
                Span::styled("  \u{26a0} ", Style::default().fg(yellow)),
                Span::styled(prompt, Style::default().fg(yellow)),
                Span::styled("  [y] Yes  [any] Cancel", Style::default().fg(label)),
            ]));
            overlay_lines.push(Line::from(""));
        }

        if let Some(ref target) = app.input_mode {
            let prompt = match target {
                super::app::InputTarget::AddLicenseIp => "  IP: ",
                super::app::InputTarget::AddDomain => "  Domain: ",
                _ => "  Input: ",
            };
            overlay_lines.push(Line::from(vec![
                Span::styled(prompt, Style::default().fg(accent)),
                Span::styled(
                    app.input_buffer.as_str(),
                    Style::default().fg(value).add_modifier(Modifier::BOLD),
                ),
                Span::styled("\u{2588}", Style::default().fg(accent)),
            ]));
        }

        frame.render_widget(
            Paragraph::new(overlay_lines).style(Style::default().bg(bg)),
            overlay,
        );
    }

    render_footer_detail(frame, app, layout[4]);
}

fn render_detail_tab(
    frame: &mut Frame,
    app: &App,
    svc: &super::models::Service,
    area: Rect,
    palette: &Theme,
) {
    theme!(app => bg, accent, border_dim, label, value, success, yellow);
    let w = area.width as usize;
    let cw = w.saturating_sub(6);
    let half = cw / 2;
    let mut lines: Vec<Line> = Vec::new();

    let status_color = match svc.status {
        ServiceStatus::Active => success,
        ServiceStatus::Expired => Color::Rgb(200, 80, 80),
        ServiceStatus::Suspended => yellow,
    };

    lines.push(Line::from(""));
    card_top(&mut lines, cw, border_dim);

    lines.push(card_row_labels("License Key", "Issue Date", half, label, border_dim));
    lines.push(card_row_values(&svc.license_key, &svc.issue_date, half, value, border_dim));
    card_empty(&mut lines, border_dim);

    lines.push(card_row_labels("Next Renewal", "Service Status", half, label, border_dim));
    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled(
            format!("{:<w$}", svc.renewal_date, w = half),
            Style::default().fg(value).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" \u{25c9} {} ", svc.status.label()),
            Style::default().fg(status_color),
        ),
    ]));
    card_empty(&mut lines, border_dim);

    lines.push(card_row_labels("Auto Renew", "Price", half, label, border_dim));
    let renew_label = if svc.auto_renew { "\u{2713} Enabled" } else { "\u{2717} Disabled" };
    let renew_color = if svc.auto_renew { success } else { label };
    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled(
            format!("{:<w$}", renew_label, w = half),
            Style::default().fg(renew_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("Free", Style::default().fg(value).add_modifier(Modifier::BOLD)),
    ]));

    card_bottom(&mut lines, cw, border_dim);
    lines.push(Line::from(""));

    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            "Authentication Mode",
            Style::default().fg(value).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  Choose how your product key validates access",
            Style::default().fg(label),
        ),
    ]));
    lines.push(Line::from(""));

    let is_ip_mode = svc.auth_mode == "ip_whitelist";
    let ip_border = if is_ip_mode { accent } else { border_dim };
    let api_border = if !is_ip_mode { accent } else { border_dim };
    let ip_icon = if is_ip_mode { "\u{25c9}" } else { "\u{25cb}" };
    let api_icon = if !is_ip_mode { "\u{25c9}" } else { "\u{25cb}" };
    let ip_text_color = if is_ip_mode { accent } else { label };
    let api_text_color = if !is_ip_mode { accent } else { label };
    let bw = half.saturating_sub(3);

    lines.push(Line::from(vec![
        Span::styled("  \u{256d}", Style::default().fg(ip_border)),
        Span::styled("\u{2500}".repeat(bw), Style::default().fg(ip_border)),
        Span::styled("\u{256e}", Style::default().fg(ip_border)),
        Span::styled(" ", Style::default()),
        Span::styled("\u{256d}", Style::default().fg(api_border)),
        Span::styled("\u{2500}".repeat(bw), Style::default().fg(api_border)),
        Span::styled("\u{256e}", Style::default().fg(api_border)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(ip_border)),
        Span::styled(format!("{} ", ip_icon), Style::default().fg(ip_text_color)),
        Span::styled(
            format!("IP Whitelist{:<w$}", "", w = bw.saturating_sub(15)),
            Style::default().fg(ip_text_color).add_modifier(if is_ip_mode { Modifier::BOLD } else { Modifier::empty() }),
        ),
        Span::styled("\u{2502}", Style::default().fg(ip_border)),
        Span::styled(" ", Style::default()),
        Span::styled("\u{2502} ", Style::default().fg(api_border)),
        Span::styled(format!("{} ", api_icon), Style::default().fg(api_text_color)),
        Span::styled(
            format!("API Key Only{:<w$}", "", w = bw.saturating_sub(15)),
            Style::default().fg(api_text_color).add_modifier(if !is_ip_mode { Modifier::BOLD } else { Modifier::empty() }),
        ),
        Span::styled("\u{2502}", Style::default().fg(api_border)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(ip_border)),
        Span::styled(
            format!("  Only whitelisted IPs{:<w$}", "", w = bw.saturating_sub(23)),
            Style::default().fg(label),
        ),
        Span::styled("\u{2502}", Style::default().fg(ip_border)),
        Span::styled(" ", Style::default()),
        Span::styled("\u{2502} ", Style::default().fg(api_border)),
        Span::styled(
            format!("  Access from any IP{:<w$}", "", w = bw.saturating_sub(21)),
            Style::default().fg(label),
        ),
        Span::styled("\u{2502}", Style::default().fg(api_border)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  \u{2570}", Style::default().fg(ip_border)),
        Span::styled("\u{2500}".repeat(bw), Style::default().fg(ip_border)),
        Span::styled("\u{256f}", Style::default().fg(ip_border)),
        Span::styled(" ", Style::default()),
        Span::styled("\u{2570}", Style::default().fg(api_border)),
        Span::styled("\u{2500}".repeat(bw), Style::default().fg(api_border)),
        Span::styled("\u{256f}", Style::default().fg(api_border)),
    ]));
    lines.push(Line::from(""));

    if svc.auth_mode == "api_key" {
        lines.push(Line::from(vec![
            Span::styled("  \u{256d}", Style::default().fg(RED_WARN)),
            Span::styled("\u{2500}".repeat(cw), Style::default().fg(RED_WARN)),
            Span::styled("\u{256e}", Style::default().fg(RED_WARN)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  \u{2502} ", Style::default().fg(RED_WARN)),
            Span::styled("\u{26a0} ", Style::default().fg(yellow)),
            Span::styled("API Key Mode Active", Style::default().fg(yellow).add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  \u{2502} ", Style::default().fg(RED_WARN)),
            Span::styled(
                "Your license can be used from any IP. Keep your API key secure. Rentals are disabled.",
                Style::default().fg(RED_WARN),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  \u{2570}", Style::default().fg(RED_WARN)),
            Span::styled("\u{2500}".repeat(cw), Style::default().fg(RED_WARN)),
            Span::styled("\u{256f}", Style::default().fg(RED_WARN)),
        ]));
    }
    lines.push(Line::from(""));

    if !svc.allowed_domains.is_empty() || !svc.devnet_allowed_domains.is_empty() {
        card_top(&mut lines, cw, border_dim);
        lines.push(Line::from(vec![
            Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
            Span::styled("Allowed Domains", Style::default().fg(value).add_modifier(Modifier::BOLD)),
        ]));
        card_empty(&mut lines, border_dim);

        if !svc.allowed_domains.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
                Span::styled("Mainnet:", Style::default().fg(label)),
            ]));
            for domain in &svc.allowed_domains {
                lines.push(Line::from(vec![
                    Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
                    Span::styled("\u{2022} ", Style::default().fg(accent)),
                    Span::styled(domain.as_str(), Style::default().fg(value)),
                ]));
            }
            card_empty(&mut lines, border_dim);
        }

        if !svc.devnet_allowed_domains.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
                Span::styled("Devnet:", Style::default().fg(label)),
            ]));
            for domain in &svc.devnet_allowed_domains {
                lines.push(Line::from(vec![
                    Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
                    Span::styled("\u{2022} ", Style::default().fg(PURPLE)),
                    Span::styled(domain.as_str(), Style::default().fg(value)),
                ]));
            }
        }

        card_bottom(&mut lines, cw, border_dim);
        lines.push(Line::from(""));
    }

    card_top(&mut lines, cw, border_dim);
    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled("Service Limits", Style::default().fg(value).add_modifier(Modifier::BOLD)),
    ]));
    card_empty(&mut lines, border_dim);

    let third = cw / 3;
    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled(format!("{:<tw$}", "RPS Limit", tw = third / 2), Style::default().fg(label)),
        Span::styled(format!("{:<tw$}", "1000 req/s", tw = third / 2), Style::default().fg(value).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{:<tw$}", "TPS Limit", tw = third / 2), Style::default().fg(label)),
        Span::styled(format!("{:<tw$}", "300 tx/s", tw = third / 2), Style::default().fg(value).add_modifier(Modifier::BOLD)),
        Span::styled("gRPC (Geyser)  ", Style::default().fg(label)),
        Span::styled("\u{2713} Enabled", Style::default().fg(success)),
    ]));
    card_empty(&mut lines, border_dim);

    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled(format!("{:<tw$}", "Jetstream", tw = third / 2), Style::default().fg(label)),
        Span::styled(format!("{:<tw$}", "\u{2713} Enabled", tw = third / 2), Style::default().fg(success)),
        Span::styled(format!("{:<tw$}", "WebSockets", tw = third / 2), Style::default().fg(label)),
        Span::styled("\u{2713} Enabled", Style::default().fg(success)),
    ]));

    card_bottom(&mut lines, cw, border_dim);
    lines.push(Line::from(""));

    if let Some(first_loc) = svc.locations.first() {
        card_top(&mut lines, cw, border_dim);
        lines.push(Line::from(vec![
            Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
            Span::styled("Quick Connect", Style::default().fg(value).add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
            Span::styled(svc.name.as_str(), Style::default().fg(label)),
        ]));
        card_empty(&mut lines, border_dim);

        for ep in &first_loc.endpoints {
            let bc = badge_color(&ep.label, palette);
            lines.push(Line::from(vec![
                Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
                Span::styled(
                    format!(" {:<19} ", ep.label),
                    Style::default().fg(bg).bg(bc).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(ep.url.as_str(), Style::default().fg(value)),
            ]));
        }

        card_bottom(&mut lines, cw, border_dim);
        lines.push(Line::from(""));
    }

    card_top(&mut lines, cw, border_dim);
    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled(
            format!("Available Endpoints ({})", svc.locations.len()),
            Style::default().fg(value).add_modifier(Modifier::BOLD),
        ),
    ]));
    card_empty(&mut lines, border_dim);

    for (i, loc) in svc.locations.iter().enumerate() {
        let is_selected = i == app.detail_ep_selected;
        let is_expanded = app.detail_ep_expanded.contains(&i);
        let ep_count = loc.endpoints.len();
        let arrow = if is_expanded { "\u{25bc}" } else { "\u{25b6}" };
        let marker = if is_selected { "\u{25b6} " } else { "  " };
        let name_color = if is_selected { accent } else { value };
        let border = if is_selected { accent } else { border_dim };

        lines.push(Line::from(vec![
            Span::styled("  \u{2502}  \u{256d}", Style::default().fg(border)),
            Span::styled(
                "\u{2500}".repeat(cw.saturating_sub(5)),
                Style::default().fg(border),
            ),
            Span::styled("\u{256e}", Style::default().fg(border)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}  \u{2502} ", Style::default().fg(border)),
            Span::styled(marker, Style::default().fg(accent)),
            Span::styled(format!("{} ", arrow), Style::default().fg(if is_selected { accent } else { label })),
            Span::styled(
                format!("{:<8}", loc.name),
                Style::default().fg(name_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("#{} \u{2022} {} slots \u{2022} {} ep", loc.id, loc.slots, ep_count),
                Style::default().fg(label),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(" Active ", Style::default().fg(bg).bg(success)),
        ]));

        if is_expanded {
            for ep in &loc.endpoints {
                let bc = badge_color(&ep.label, palette);
                lines.push(Line::from(vec![
                    Span::styled("  \u{2502}  \u{2502}     ", Style::default().fg(border)),
                    Span::styled(
                        format!(" {:<19} ", ep.label),
                        Style::default().fg(bg).bg(bc).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  ", Style::default()),
                    Span::styled(ep.url.as_str(), Style::default().fg(value)),
                ]));
            }
        }

        lines.push(Line::from(vec![
            Span::styled("  \u{2502}  \u{2570}", Style::default().fg(border)),
            Span::styled(
                "\u{2500}".repeat(cw.saturating_sub(5)),
                Style::default().fg(border),
            ),
            Span::styled("\u{256f}", Style::default().fg(border)),
        ]));
    }

    card_bottom(&mut lines, cw, border_dim);

    let scroll = app.detail_scroll;
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(bg)).scroll((scroll, 0)),
        area,
    );
}

fn render_ips_tab(
    frame: &mut Frame,
    app: &App,
    svc: &super::models::Service,
    area: Rect,
    _palette: &Theme,
) {
    theme!(app => bg, accent, border_dim, label, value, success, yellow);
    let cw = (area.width as usize).saturating_sub(6);
    let mut lines: Vec<Line> = Vec::new();

    use super::app::DomainNetwork;
    let is_mainnet = app.detail_domain_network == DomainNetwork::Mainnet;
    let filtered_locs: Vec<(usize, &super::models::Location)> = svc.locations.iter()
        .enumerate()
        .filter(|(_, l)| {
            if is_mainnet { !l.name.eq_ignore_ascii_case("devnet") }
            else { l.name.eq_ignore_ascii_case("devnet") }
        })
        .collect();

    let mainnet_ips: usize = svc.locations.iter()
        .filter(|l| !l.name.eq_ignore_ascii_case("devnet"))
        .map(|l| l.ips.len())
        .sum();
    let devnet_ips: usize = svc.locations.iter()
        .filter(|l| l.name.eq_ignore_ascii_case("devnet"))
        .map(|l| l.ips.len())
        .sum();

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("\u{25cf} ", Style::default().fg(if is_mainnet { success } else { label })),
        Span::styled(
            format!("Mainnet ({} IPs)", mainnet_ips),
            Style::default().fg(if is_mainnet { success } else { label })
                .add_modifier(if is_mainnet { Modifier::BOLD } else { Modifier::empty() }),
        ),
        Span::styled("    ", Style::default()),
        Span::styled("\u{25cf} ", Style::default().fg(if !is_mainnet { PURPLE } else { label })),
        Span::styled(
            format!("Devnet ({} IPs)", devnet_ips),
            Style::default().fg(if !is_mainnet { PURPLE } else { label })
                .add_modifier(if !is_mainnet { Modifier::BOLD } else { Modifier::empty() }),
        ),
    ]));
    lines.push(Line::from(""));

    card_top(&mut lines, cw, border_dim);
    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled(
            format!("{:<25}", "LOCATION & IP ADDRESS"),
            Style::default().fg(label),
        ),
        Span::styled(
            format!("{:<12}", "STATUS"),
            Style::default().fg(label),
        ),
        Span::styled(
            format!("{:<10}", "TYPE"),
            Style::default().fg(label),
        ),
        Span::styled("EXPIRES", Style::default().fg(label)),
    ]));
    card_empty(&mut lines, border_dim);

    for (display_idx, (_orig_idx, loc)) in filtered_locs.iter().enumerate() {
        let is_selected = display_idx == app.detail_location_selected;
        let is_expanded = app.detail_location_expanded == Some(app.detail_location_selected) && is_selected;
        let arrow = if is_expanded { "\u{25b4}" } else { "\u{25be}" };
        let used = loc.ips.len();
        let available = (loc.slots as usize).saturating_sub(used);
        let name_color = if is_selected { accent } else { value };
        let marker = if is_selected { "\u{25b6} " } else { "  " };

        lines.push(Line::from(vec![
            Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
            Span::styled(marker, Style::default().fg(accent)),
            Span::styled(format!("{} ", arrow), Style::default().fg(if is_selected { accent } else { label })),
            Span::styled(
                format!("{:<8}", loc.name),
                Style::default().fg(name_color).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}     ", Style::default().fg(border_dim)),
            Span::styled(
                format!("{}/{} slots used", used, loc.slots),
                Style::default().fg(label),
            ),
            Span::styled(
                format!("{}available: {}", " ".repeat(15), available),
                Style::default().fg(if available > 0 { success } else { label }),
            ),
        ]));

        if is_expanded && !loc.ips.is_empty() {
            for (j, ip) in loc.ips.iter().enumerate() {
                let ip_selected = j == app.detail_ip_selected;
                let ip_marker = if ip_selected { "\u{25b6} " } else { "  " };
                let ip_color = if ip_selected { accent } else { value };
                let type_label = if ip.is_rented { "Rented" } else { "Owned" };
                let type_color = if ip.is_rented { yellow } else { success };
                let expires = if ip.expires_at.is_empty() { "\u{2014}".to_string() } else { ip.expires_at.clone() };
                lines.push(Line::from(vec![
                    Span::styled("  \u{2502}     ", Style::default().fg(border_dim)),
                    Span::styled(ip_marker, Style::default().fg(accent)),
                    Span::styled(
                        format!("{}. {:<18}", j + 1, ip.ip_address),
                        Style::default().fg(ip_color),
                    ),
                    Span::styled(
                        format!("{:<12}", "\u{25c9} Active"),
                        Style::default().fg(success),
                    ),
                    Span::styled(
                        format!("{:<10}", type_label),
                        Style::default().fg(type_color),
                    ),
                    Span::styled(expires, Style::default().fg(label)),
                ]));
            }
        }

        card_empty(&mut lines, border_dim);
    }

    if filtered_locs.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}     ", Style::default().fg(border_dim)),
            Span::styled("No locations for this network.", Style::default().fg(label)),
        ]));
        card_empty(&mut lines, border_dim);
    }

    card_bottom(&mut lines, cw, border_dim);

    if svc.auth_mode == "api_key" {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  \u{256d}", Style::default().fg(border_dim)),
            Span::styled("\u{2500}".repeat(cw), Style::default().fg(border_dim)),
            Span::styled("\u{256e}", Style::default().fg(border_dim)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
            Span::styled("\u{f084} ", Style::default().fg(yellow)),
            Span::styled(
                "API Key Mode Active",
                Style::default().fg(value).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
            Span::styled(
                "IP whitelisting is disabled when using API Key authentication mode.",
                Style::default().fg(label),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  \u{2570}", Style::default().fg(border_dim)),
            Span::styled("\u{2500}".repeat(cw), Style::default().fg(border_dim)),
            Span::styled("\u{256f}", Style::default().fg(border_dim)),
        ]));
    }

    let scroll = app.detail_scroll;
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(bg)).scroll((scroll, 0)),
        area,
    );
}

fn render_domains_tab(
    frame: &mut Frame,
    app: &App,
    svc: &super::models::Service,
    area: Rect,
) {
    use super::app::DomainNetwork;
    theme!(app => bg, accent, border_dim, label, value, success);
    let cw = (area.width as usize).saturating_sub(6);
    let mut lines: Vec<Line> = Vec::new();

    let is_mainnet = app.detail_domain_network == DomainNetwork::Mainnet;
    let domains = if is_mainnet { &svc.allowed_domains } else { &svc.devnet_allowed_domains };
    let net_label = if is_mainnet { "Mainnet" } else { "Devnet" };

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("\u{25cf} ", Style::default().fg(if is_mainnet { success } else { label })),
        Span::styled(
            format!("Mainnet ({} domains)", svc.allowed_domains.len()),
            Style::default().fg(if is_mainnet { success } else { label })
                .add_modifier(if is_mainnet { Modifier::BOLD } else { Modifier::empty() }),
        ),
        Span::styled("    ", Style::default()),
        Span::styled("\u{25cf} ", Style::default().fg(if !is_mainnet { PURPLE } else { label })),
        Span::styled(
            format!("Devnet ({} domains)", svc.devnet_allowed_domains.len()),
            Style::default().fg(if !is_mainnet { PURPLE } else { label })
                .add_modifier(if !is_mainnet { Modifier::BOLD } else { Modifier::empty() }),
        ),
    ]));
    lines.push(Line::from(""));

    card_top(&mut lines, cw, border_dim);
    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled(
            format!("\u{f0ac} {} Allowed Domains", net_label),
            Style::default().fg(value).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        if domains.is_empty() {
            Span::styled("All domains allowed (no restrictions)", Style::default().fg(label))
        } else {
            Span::styled(
                format!("{} domain{} configured", domains.len(), if domains.len() != 1 { "s" } else { "" }),
                Style::default().fg(label),
            )
        },
    ]));
    card_empty(&mut lines, border_dim);

    if domains.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}     ", Style::default().fg(border_dim)),
            Span::styled("\u{f0ac} ", Style::default().fg(border_dim)),
            Span::styled("No domains configured", Style::default().fg(label)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}     ", Style::default().fg(border_dim)),
            Span::styled("API can be accessed from any domain.", Style::default().fg(label)),
        ]));
    } else {
        for (i, domain) in domains.iter().enumerate() {
            let is_selected = i == app.detail_domain_selected;
            let marker = if is_selected { "\u{25b6} " } else { "  " };
            let fg = if is_selected { accent } else { value };
            lines.push(Line::from(vec![
                Span::styled("  \u{2502}   ", Style::default().fg(border_dim)),
                Span::styled(marker, Style::default().fg(accent)),
                Span::styled(domain.as_str(), Style::default().fg(fg)),
            ]));
        }
    }
    card_empty(&mut lines, border_dim);

    lines.push(Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled("Patterns: ", Style::default().fg(label).add_modifier(Modifier::BOLD)),
        Span::styled("Use ", Style::default().fg(label)),
        Span::styled("*.example.com", Style::default().fg(value)),
        Span::styled(" to match all subdomains", Style::default().fg(label)),
    ]));

    card_bottom(&mut lines, cw, border_dim);

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(bg)),
        area,
    );
}
fn card_top(lines: &mut Vec<Line>, cw: usize, border_dim: Color) {
    lines.push(Line::from(vec![
        Span::styled("  \u{256d}", Style::default().fg(border_dim)),
        Span::styled("\u{2500}".repeat(cw), Style::default().fg(border_dim)),
        Span::styled("\u{256e}", Style::default().fg(border_dim)),
    ]));
}

fn card_bottom(lines: &mut Vec<Line>, cw: usize, border_dim: Color) {
    lines.push(Line::from(vec![
        Span::styled("  \u{2570}", Style::default().fg(border_dim)),
        Span::styled("\u{2500}".repeat(cw), Style::default().fg(border_dim)),
        Span::styled("\u{256f}", Style::default().fg(border_dim)),
    ]));
}

fn card_empty(lines: &mut Vec<Line>, border_dim: Color) {
    lines.push(Line::from(vec![
        Span::styled("  \u{2502}", Style::default().fg(border_dim)),
    ]));
}

fn card_row_labels(left: &str, right: &str, half: usize, label: Color, border_dim: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled(
            format!("{:<w$}", left, w = half),
            Style::default().fg(label),
        ),
        Span::styled(right.to_string(), Style::default().fg(label)),
    ])
}

fn card_row_values(left: &str, right: &str, half: usize, value: Color, border_dim: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled("  \u{2502} ", Style::default().fg(border_dim)),
        Span::styled(
            format!("{:<w$}", left, w = half),
            Style::default().fg(value).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            right.to_string(),
            Style::default().fg(value).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn render_footer_detail(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => accent, label, bg_sidebar);
    let keys: Vec<(&str, &str)> = if app.input_mode.is_some() {
        vec![
            ("[Esc]", "Cancel"),
            ("[Enter]", "Submit"),
        ]
    } else if app.confirm.is_some() {
        vec![
            ("[y]", "Confirm"),
            ("[any]", "Cancel"),
        ]
    } else if app.detail_tab == super::app::DetailTab::WhitelistedIps && app.detail_location_expanded.is_some() {
        vec![
            ("[q]", "Quit"),
            ("[Esc]", "Back"),
            ("[Tab]", "Switch Tab"),
            ("[↑↓]", "Select IP"),
            ("[Enter]", "Collapse"),
            ("[n]", "Net"),
            ("[a]", "Add IP"),
            ("[d]", "Remove IP"),
            ("[R]", "Refresh"),
            ("[t]", "Theme"),
        ]
    } else if app.detail_tab == super::app::DetailTab::Details {
        vec![
            ("[q]", "Quit"),
            ("[Esc]", "Back"),
            ("[Tab]", "Switch Tab"),
            ("[↑↓]", "Scroll"),
            ("[PgUp/Dn]", "Endpoints"),
            ("[Enter]", "Toggle"),
            ("[m]", "Auth Mode"),
            ("[w]", "Auto Renew"),
            ("[r]", "Regen Key"),
            ("[R]", "Refresh"),
            ("[t]", "Theme"),
        ]
    } else if app.detail_tab == super::app::DetailTab::Domains {
        vec![
            ("[q]", "Quit"),
            ("[Esc]", "Back"),
            ("[Tab]", "Switch Tab"),
            ("[↑↓]", "Select"),
            ("[n]", "Net"),
            ("[a]", "Add"),
            ("[d]", "Remove"),
            ("[R]", "Refresh"),
            ("[t]", "Theme"),
        ]
    } else if app.detail_tab == super::app::DetailTab::WhitelistedIps {
        vec![
            ("[q]", "Quit"),
            ("[Esc]", "Back"),
            ("[Tab]", "Switch Tab"),
            ("[↑↓]", "Select"),
            ("[Enter]", "Expand"),
            ("[n]", "Net"),
            ("[R]", "Refresh"),
            ("[t]", "Theme"),
        ]
    } else {
        vec![
            ("[q]", "Quit"),
            ("[Esc]", "Back"),
            ("[Tab]", "Switch Tab"),
            ("[↑↓]", "Scroll"),
            ("[R]", "Refresh"),
            ("[t]", "Theme"),
        ]
    };

    let mut spans: Vec<Span> = vec![Span::styled(" ", Style::default())];
    for (key, desc) in keys {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{desc}  "),
            Style::default().fg(label),
        ));
    }

    let footer = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(bg_sidebar));
    frame.render_widget(footer, area);
}

fn render_footer_services(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => accent, label, bg_sidebar);
    let keys = vec![
        ("[q]", "Quit"),
        ("[Esc]", "Back"),
        ("[↑↓]", "Select"),
        ("[Enter]", "View"),
        ("[Tab]", "Layout"),
        ("[[]", "Collapse"),
        ("[t]", "Theme"),
    ];

    let mut spans: Vec<Span> = vec![Span::styled(" ", Style::default())];
    for (key, desc) in keys {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{desc}  "),
            Style::default().fg(label),
        ));
    }

    let footer = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(bg_sidebar));
    frame.render_widget(footer, area);
}
fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    theme!(app => accent, label, bg_sidebar);
    let keys = vec![
        ("[q]", "Quit"),
        ("[Tab]", "Focus"),
        ("[↑↓]", "Nav"),
        ("[[]", "Collapse"),
        ("[t]", "Theme"),
    ];

    let mut spans: Vec<Span> = vec![Span::styled(" ", Style::default())];
    for (key, desc) in keys {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{desc}  "),
            Style::default().fg(label),
        ));
    }

    let footer = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(bg_sidebar));
    frame.render_widget(footer, area);
}
fn shrink(area: Rect, top: u16, right: u16, bottom: u16, left: u16) -> Rect {
    Rect {
        x: area.x + left,
        y: area.y + top,
        width: area.width.saturating_sub(left + right),
        height: area.height.saturating_sub(top + bottom),
    }
}
