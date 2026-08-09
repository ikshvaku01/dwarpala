use crate::ui::app::{App, Panel, Status, MenuTab};
use ratatui::{prelude::*, widgets::*};
// Clear is used to wipe the popup area before drawing so background content
// doesn't bleed through. Without it ratatui composites on top of whatever
// was rendered in the previous pass, producing the visual artefacts seen.
use ratatui::widgets::Clear;

// ── Entry point ──────────────────────────────────────────────────────────────

pub fn draw(f: &mut Frame, app: &App) {
    let outer = Block::default()
        .title(Span::styled(" ◈ Dwarpala ", Style::new().fg(Color::Cyan).bold()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Color::DarkGray));
    let inner = outer.inner(f.area());
    f.render_widget(outer, f.area());

    let [hdr, mid, prog, ftr] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(3),
        Constraint::Length(1),
    ]).areas(inner);

    header(f, hdr, app);
    body(f, mid, app);
    progress_bar(f, prog, app);
    footer(f, ftr, app);

    // Menu overlay is rendered last so it sits on top of everything.
    if app.menu_open {
        draw_menu_overlay(f, app);
    }
}

// ── Main TUI sections ────────────────────────────────────────────────────────

fn header(f: &mut Frame, area: Rect, app: &App) {
    let status_span = match &app.status {
        Status::Idle               => Span::styled("● IDLE",    Style::new().fg(Color::Yellow)),
        Status::Running            => Span::styled("▶ RUNNING", Style::new().fg(Color::Green)),
        Status::Done { count, .. } =>
            Span::styled(format!("✓ DONE ({count} files)"), Style::new().fg(Color::Cyan)),
        Status::Err(_)             => Span::styled("✗ ERROR",   Style::new().fg(Color::Red)),
    };
    let dim = Style::new().fg(Color::DarkGray);
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled("  Root: ", dim),
        Span::styled(app.root.to_string_lossy().into_owned(), Style::new().fg(Color::White)),
        Span::raw("   "),
        Span::styled("Type: ", dim),
        Span::styled(app.ptype.clone(), Style::new().fg(Color::Magenta).bold()),
        Span::raw("   "),
        Span::styled("Files: ", dim),
        Span::styled(app.files.len().to_string(), Style::new().fg(Color::White)),
        Span::raw("   "),
        status_span,
        Span::raw("   "),
        Span::styled(format!("{:.1}s", app.elapsed.as_secs_f32()), dim),
    ])), area);
}

fn body(f: &mut Frame, area: Rect, app: &App) {
    let [left, right] = Layout::horizontal([
        Constraint::Percentage(55),
        Constraint::Percentage(45),
    ]).areas(area);
    let [files_area, log_area] = Layout::vertical([
        Constraint::Percentage(60),
        Constraint::Percentage(40),
    ]).areas(left);
    file_list(f, files_area, app);
    pattern_list(f, right, app);
    log_panel(f, log_area, app);
}

fn panel_block(title: String, active: bool) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            if active { Style::new().fg(Color::Cyan).bold() }
            else      { Style::new().fg(Color::DarkGray) },
        ))
        .borders(Borders::ALL)
        .border_type(if active { BorderType::Thick } else { BorderType::Plain })
        .border_style(if active { Style::new().fg(Color::Cyan) }
                      else      { Style::new().fg(Color::DarkGray) })
}

fn file_list(f: &mut Frame, area: Rect, app: &App) {
    let active = app.panel == Panel::Files;
    let block  = panel_block(format!("Included Files ({})", app.files.len()), active);
    let inner  = block.inner(area);
    f.render_widget(block, area);
    let items: Vec<ListItem> = app.files.iter()
        .map(|s| ListItem::new(Span::styled(s.clone(), Style::new().fg(Color::White))))
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() { state.select(Some(app.f_scroll)); }
    f.render_stateful_widget(
        List::new(items).highlight_style(Style::new().fg(Color::Black).bg(Color::White)),
        inner, &mut state,
    );
}

fn pattern_list(f: &mut Frame, area: Rect, app: &App) {
    let active = app.panel == Panel::Patterns;
    let block  = panel_block(
        format!("Ignore Patterns ({})  [-] remove", app.patterns.len()), active,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    let items: Vec<ListItem> = app.patterns.iter().enumerate().map(|(i, s)| {
        let style = if active && i == app.p_scroll {
            Style::new().fg(Color::Black).bg(Color::Yellow)
        } else {
            Style::new().fg(Color::Yellow)
        };
        ListItem::new(Span::styled(s.clone(), style))
    }).collect();
    let mut state = ListState::default();
    if !items.is_empty() { state.select(Some(app.p_scroll)); }
    f.render_stateful_widget(
        List::new(items).highlight_style(Style::new().fg(Color::Black).bg(Color::Yellow)),
        inner, &mut state,
    );
}

fn log_panel(f: &mut Frame, area: Rect, app: &App) {
    let active = app.panel == Panel::Log;
    let block  = panel_block("Log".to_string(), active);
    let inner  = block.inner(area);
    f.render_widget(block, area);
    let h     = inner.height as usize;
    let start = app.l_scroll.saturating_sub(h.saturating_sub(1));
    let lines: Vec<Line> = app.log.iter().skip(start).map(|l| {
        let color = if l.starts_with('+')      { Color::Green }
            else if l.starts_with('-')          { Color::Red }
            else if l.starts_with('✓')          { Color::Cyan }
            else if l.starts_with('✗')          { Color::Red }
            else if l.starts_with('↺')          { Color::Magenta }
            else                                { Color::Gray };
        Line::from(Span::styled(l.clone(), Style::new().fg(color)))
    }).collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn progress_bar(f: &mut Frame, area: Rect, app: &App) {
    let sep   = Block::default().borders(Borders::TOP)
        .border_style(Style::new().fg(Color::DarkGray));
    let inner = sep.inner(area);
    f.render_widget(sep, area);
    let [bar, lbl] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(inner);

    let (ratio, text, color) = match &app.status {
        Status::Idle    => (0.0, "  [R] compile".into(), Color::DarkGray),
        Status::Running => {
            let r = if app.total > 0 { app.progress as f64 / app.total as f64 } else { 0.0 };
            (r, format!("  [{}/{}] {}", app.progress, app.total, app.cur_file), Color::Gray)
        }
        Status::Done { path, count } =>
            (1.0, format!("  ✓ {count} files → {}", path.display()), Color::Cyan),
        Status::Err(e)  => (0.0, format!("  ✗ {e}"), Color::Red),
    };
    f.render_widget(
        Gauge::default()
            .gauge_style(Style::new().fg(Color::Cyan).bg(Color::DarkGray))
            .ratio(ratio),
        bar,
    );
    f.render_widget(Paragraph::new(Span::styled(text, Style::new().fg(color))), lbl);
}

fn footer(f: &mut Frame, area: Rect, app: &App) {
    let keys = if app.menu_open {
        "[1/2/3] Tab  [←→] Cycle  [Esc] Close menu"
    } else if matches!(app.status, Status::Running) {
        "[M] Menu  [Tab] Switch  [↑↓] Scroll  [Q] Quit"
    } else {
        "[R] Run  [M] Menu  [Tab] Switch  [↑↓] Scroll  [-] Remove Pattern  [Q] Quit"
    };
    f.render_widget(
        Paragraph::new(Span::styled(keys, Style::new().fg(Color::DarkGray))), area,
    );
}

// ── Menu overlay ─────────────────────────────────────────────────────────────

fn draw_menu_overlay(f: &mut Frame, app: &App) {
    let area  = f.area();
    let w     = 72u16.min(area.width.saturating_sub(4));
    let h     = 26u16.min(area.height.saturating_sub(2));
    let popup = centered_fixed(w, h, area);

    // Erase whatever the main TUI rendered in this region.
    // This must happen before any other widget is drawn into `popup`.
    f.render_widget(Clear, popup);

    let chrome = Block::default()
        .title(Span::styled(" ◈ Menu ", Style::new().fg(Color::Cyan).bold()))
        .title_bottom(Span::styled(
            " [Esc] Close  [1/2/3] Select Tab  [←→] Cycle ",
            Style::new().fg(Color::DarkGray),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::new().fg(Color::Cyan))
        .style(Style::new().bg(Color::Black));

    let inner = chrome.inner(popup);
    f.render_widget(chrome, popup);

    // Layout: tab bar | separator | content
    let [tab_row, sep_row, content] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ]).areas(inner);

    draw_tab_bar(f, tab_row, app);

    // Separator line
    f.render_widget(
        Block::default().borders(Borders::TOP)
            .border_style(Style::new().fg(Color::DarkGray)),
        sep_row,
    );

    match app.menu_tab {
        MenuTab::Home    => draw_home_tab(f, content, app),
        MenuTab::License => draw_license_tab(f, content, app),
        MenuTab::Credits => draw_credits_tab(f, content, app),
    }
}

fn draw_tab_bar(f: &mut Frame, area: Rect, app: &App) {
    let tabs = [
        ("1: Home",    &MenuTab::Home),
        ("2: License", &MenuTab::License),
        ("3: Credits", &MenuTab::Credits),
    ];
    let mut spans = vec![Span::raw(" ")];
    for (label, variant) in &tabs {
        let active = &app.menu_tab == *variant;
        let style  = if active {
            Style::new().fg(Color::Black).bg(Color::Cyan).bold()
        } else {
            Style::new().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!(" {label} "), style));
        spans.push(Span::raw("  "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ── Home tab ─────────────────────────────────────────────────────────────────

fn draw_home_tab(f: &mut Frame, area: Rect, app: &App) {
    let dim  = Style::new().fg(Color::DarkGray);
    let hi   = Style::new().fg(Color::White);
    let bold = Style::new().fg(Color::Cyan).bold();

    let status_str = match &app.status {
        Status::Idle      => "Idle".to_string(),
        Status::Running   => format!("Running ({}/{})", app.progress, app.total),
        Status::Done { count, .. } => format!("Done — {count} files"),
        Status::Err(e)    => format!("Error: {e}"),
    };

    let files_count    = app.files.len().to_string();
    let patterns_count = app.patterns.len().to_string();

    let lines: Vec<Line> = vec![
        Line::from(Span::styled("  Session", bold)),
        Line::from(vec![
            Span::styled("  ─────────────────────────────────────", dim),
        ]),
        kv("  Project",  &app.ptype,               hi),
        kv("  Edition",  app.edition.label(),       hi),
        kv("  Files",    &files_count,    hi),
        kv("  Patterns", &patterns_count, hi),
        kv("  Status",   &status_str,               hi),
        Line::from(""),
        Line::from(Span::styled("  Keyboard Shortcuts", bold)),
        Line::from(Span::styled("  ─────────────────────────────────────", dim)),
        shortcut("  [R]",       "Compile files into output document"),
        shortcut("  [M]",       "Open / close this menu"),
        shortcut("  [Tab]",     "Cycle between Files / Patterns / Log panels"),
        shortcut("  [↑↓]",      "Scroll the focused panel"),
        shortcut("  [PgUp/Dn]", "Scroll faster"),
        shortcut("  [-]",       "Remove selected ignore pattern"),
        shortcut("  [Q]",       "Quit"),
    ];

    f.render_widget(Paragraph::new(lines), area);
}

fn kv<'a>(key: &'a str, val: &'a str, val_style: Style) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{key:<14}"), Style::new().fg(Color::DarkGray)),
        Span::styled(val, val_style),
    ])
}

fn shortcut<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{key:<14}"), Style::new().fg(Color::Cyan)),
        Span::styled(desc, Style::new().fg(Color::Gray)),
    ])
}

// ── License tab ──────────────────────────────────────────────────────────────

fn draw_license_tab(f: &mut Frame, area: Rect, app: &App) {
    let dim  = Style::new().fg(Color::DarkGray);
    let bold = Style::new().fg(Color::Cyan).bold();

    let tier_color = match app.edition {
        crate::edition::Edition::Community  => Color::DarkGray,
        crate::edition::Edition::Pro        => Color::Cyan,
        crate::edition::Edition::Team       => Color::Magenta,
        crate::edition::Edition::Enterprise => Color::Yellow,
    };

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled("  Current License", bold)),
        Line::from(Span::styled("  ─────────────────────────────────────", dim)),
        Line::from(vec![
            Span::styled("  Edition       ", dim),
            Span::styled(app.edition.label(), Style::new().fg(tier_color).bold()),
        ]),
        Line::from(""),
    ];

    if app.has_license_screen {
        lines.extend([
            Line::from(Span::styled(
                "  ┌─ License Manager ─────────────────────────────────────────┐",
                dim,
            )),
            Line::from(vec![
                Span::styled("  │  ", dim),
                Span::styled(
                    "[L] or [Enter]  Open the License Manager",
                    Style::new().fg(Color::Green).bold(),
                ),
                Span::styled("                   │", dim),
            ]),
            Line::from(Span::styled(
                "  └────────────────────────────────────────────────────────────┘",
                dim,
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Enter and validate your Pro / Team / Enterprise key in the",
                Style::new().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "  License Manager. The key is saved encrypted and applied on",
                Style::new().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "  the next launch.",
                Style::new().fg(Color::Gray),
            )),
        ]);
    } else {
        lines.extend([
            Line::from(Span::styled(
                "  License management is not available in the Community binary.",
                Style::new().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  To unlock Pro / Team / Enterprise features, download the",
                Style::new().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "  Enterprise edition from:",
                Style::new().fg(Color::Gray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "    https://github.com/your-org/dwarpala",
                Style::new().fg(Color::Cyan),
            )),
        ]);
    }

    f.render_widget(Paragraph::new(lines), area);
}

// ── Credits tab ──────────────────────────────────────────────────────────────

fn draw_credits_tab(f: &mut Frame, area: Rect, app: &App) {
    let dim  = Style::new().fg(Color::DarkGray);
    let bold = Style::new().fg(Color::Cyan).bold();
    let gray = Style::new().fg(Color::Gray);

    let lines: Vec<Line> = vec![
        Line::from(Span::styled("  ◈ Dwarpala  v0.1.0", bold)),
        Line::from(Span::styled(
            "  ────────────────────────────────────────────────────",
            dim,
        )),
        Line::from(Span::styled(
            "  Compile and transform codebases into AI-ready datasets.",
            gray,
        )),
        Line::from(""),
        kv("  Edition",    app.edition.label(),                     Style::new().fg(Color::White)),
        kv("  Repository", "github.com/your-org/dwarpala",          Style::new().fg(Color::Cyan)),
        kv("  License",    "MIT (Community) / Proprietary (Enterprise)", gray),
        kv("  Rust",       "1.93+",                                  gray),
        Line::from(""),
        Line::from(Span::styled(
            "  Built with ♥ using Rust",
            Style::new().fg(Color::Magenta),
        )),
        Line::from(""),
        Line::from(Span::styled("  Core Libraries", bold)),
        Line::from(Span::styled(
            "  ────────────────────────────────────────────────────",
            dim,
        )),
        dep("  ratatui",   "0.30",  "TUI framework"),
        dep("  crossterm", "0.29",  "Terminal backend"),
        dep("  clap",      "4.5",   "Argument parsing"),
        dep("  walkdir",   "2.5",   "Directory traversal"),
        dep("  notify",    "8.2",   "Filesystem events"),
        dep("  serde",     "1.0",   "Config serialisation"),
        dep("  regex",     "1.12",  "Pattern matching"),
    ];

    f.render_widget(Paragraph::new(lines), area);
}

fn dep<'a>(name: &'a str, ver: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{name:<14}"), Style::new().fg(Color::Cyan)),
        Span::styled(format!("{ver:<8}"),   Style::new().fg(Color::DarkGray)),
        Span::styled(desc,                  Style::new().fg(Color::Gray)),
    ])
}

// ── Geometry helper ───────────────────────────────────────────────────────────

fn centered_fixed(w: u16, h: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height))
}