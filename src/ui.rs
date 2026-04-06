//! Layout and rendering: header, table, footer, filter bar, kill confirmation.

use crate::app::App;
use crate::ports::{PortCategory, PortEntry};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table,
    TableState, Wrap,
};
use ratatui::Frame;

const HEADER_BG: Color = Color::Rgb(30, 30, 46);
const HEADER_FG: Color = Color::Rgb(205, 214, 244);
const ROW_SELECTED_BG: Color = Color::Rgb(49, 50, 68);
const ROW_SELECTED_FG: Color = Color::Rgb(239, 241, 255);
const FOOTER_FG: Color = Color::Rgb(108, 112, 134);
const KEY_ACCENT: Color = Color::Rgb(137, 180, 250);

pub fn draw(
    f: &mut Frame<'_>,
    app: &App,
    table_state: &mut TableState,
    table_area_override: Option<Rect>,
) {
    let area = f.area();

    let footer_h = if app.filter_editing { 3 } else { 2 };
    let details_h: u16 = if app.show_details { 6 } else { 0 };

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(details_h),
            Constraint::Length(footer_h),
        ])
        .split(area);

    let header_area = main_chunks[0];
    let table_block_area = main_chunks[1];
    let details_area = main_chunks[2];
    let footer_area = main_chunks[3];

    render_header(f, app, header_area);
    render_table(
        f,
        app,
        table_state,
        table_area_override.unwrap_or(table_block_area),
    );

    if app.show_details {
        render_details(f, app, details_area);
    }

    if app.filter_editing {
        let filter_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(footer_area);
        render_footer(f, app, filter_chunks[0], true);
        render_filter_line(f, app, filter_chunks[1]);
    } else {
        render_footer(f, app, footer_area, false);
    }

    if let Some(ref prompt) = app.kill_prompt {
        render_kill_popup(f, area, prompt);
    }
}

fn usage_color(pct: f32) -> Color {
    if pct >= 80.0 {
        Color::Rgb(243, 139, 168) // red
    } else if pct >= 50.0 {
        Color::Rgb(249, 226, 175) // yellow
    } else {
        Color::Rgb(166, 227, 161) // green
    }
}

fn render_header(f: &mut Frame<'_>, app: &App, area: Rect) {
    let ss = &app.sys_stats;
    let mem_pct = if ss.mem_total_gib > 0.0 {
        (ss.mem_used_gib / ss.mem_total_gib * 100.0) as f32
    } else {
        0.0
    };

    let title = Line::from(vec![
        Span::styled(
            " PORT CLI ",
            Style::default()
                .fg(HEADER_FG)
                .bg(HEADER_BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  CPU: ", Style::default().fg(FOOTER_FG).bg(HEADER_BG)),
        Span::styled(
            format!("{:.0}%", ss.cpu_pct),
            Style::default().fg(usage_color(ss.cpu_pct)).bg(HEADER_BG).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  MEM: ", Style::default().fg(FOOTER_FG).bg(HEADER_BG)),
        Span::styled(
            format!("{:.1}/{:.0} GB", ss.mem_used_gib, ss.mem_total_gib),
            Style::default().fg(usage_color(mem_pct)).bg(HEADER_BG).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default().bg(HEADER_BG)),
        Span::styled(
            format!("sort:{}  ", app.sort_mode.label()),
            Style::default().fg(Color::Rgb(166, 218, 239)).bg(HEADER_BG),
        ),
        Span::styled(
            if app.last_error.is_some() {
                "! scan error  ".to_string()
            } else if app.auto_refresh {
                "refresh: 2s  ".to_string()
            } else {
                "refresh: off  ".to_string()
            },
            Style::default()
                .fg(if app.auto_refresh && app.last_error.is_none() {
                    Color::Rgb(166, 227, 161)
                } else {
                    Color::Rgb(243, 139, 168)
                })
                .bg(HEADER_BG),
        ),
        Span::styled("[?] help ", Style::default().fg(FOOTER_FG).bg(HEADER_BG)),
    ]);

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::Rgb(69, 71, 90)))
        .style(Style::default().bg(HEADER_BG));

    let inner = block.inner(area);
    f.render_widget(block, area);
    let p = Paragraph::new(title).alignment(Alignment::Left);
    f.render_widget(p, inner);
}

fn port_style(entry: &PortEntry, selected: bool) -> Style {
    let base = match entry.port_category() {
        PortCategory::WellKnown => Style::default().fg(Color::Rgb(243, 139, 168)),
        PortCategory::Registered => Style::default().fg(Color::Rgb(166, 227, 161)),
        PortCategory::Dynamic => Style::default().fg(Color::Rgb(249, 226, 175)),
    };
    if selected {
        base.bg(ROW_SELECTED_BG)
            .fg(ROW_SELECTED_FG)
            .add_modifier(Modifier::BOLD)
    } else {
        base
    }
}

fn render_table(f: &mut Frame<'_>, app: &App, table_state: &mut TableState, area: Rect) {
    let visible: Vec<&PortEntry> = app.visible_ports();
    let conflicts = app.conflicting_ports();
    if visible.is_empty() {
        table_state.select(None);
    } else {
        table_state.select(Some(app.selected));
    }
    let header_style = Style::default()
        .fg(HEADER_FG)
        .bg(HEADER_BG)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec!["PID", "Process", "Port", "CPU%", "MEM", "Address", "User", "Proto"])
        .style(header_style)
        .height(1);

    let rows: Vec<Row> = visible
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let selected = i == app.selected;
            let is_conflict = conflicts.contains(&e.port);
            let st = port_style(e, selected);
            let pid = Style::default()
                .fg(if selected {
                    ROW_SELECTED_FG
                } else {
                    Color::Rgb(205, 214, 244)
                })
                .bg(if selected {
                    ROW_SELECTED_BG
                } else {
                    Color::Reset
                })
                .add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() });

            let name_st = Style::default()
                .fg(if selected {
                    ROW_SELECTED_FG
                } else {
                    Color::Rgb(137, 220, 235)
                })
                .bg(if selected {
                    ROW_SELECTED_BG
                } else {
                    Color::Reset
                })
                .add_modifier(Modifier::BOLD);

            let conflict_st = Style::default()
                .fg(Color::Rgb(250, 179, 135))
                .bg(if selected { ROW_SELECTED_BG } else { Color::Reset })
                .add_modifier(Modifier::BOLD);

            let port_text = if is_conflict {
                format!("{} !", e.port)
            } else {
                format!("{}", e.port)
            };

            let cpu_text = format!("{:.1}", e.cpu_pct);
            let mem_gib = e.mem_mib / 1024.0;
            let mem_text = format!("{:.1} GB", mem_gib);

            let resource_st = Style::default()
                .fg(if selected {
                    ROW_SELECTED_FG
                } else if e.cpu_pct >= 50.0 || mem_gib >= 0.5 {
                    Color::Rgb(243, 139, 168) // red for high usage
                } else if e.cpu_pct >= 10.0 || mem_gib >= 0.125 {
                    Color::Rgb(249, 226, 175) // yellow for moderate
                } else {
                    Color::Rgb(205, 214, 244) // normal
                })
                .bg(if selected { ROW_SELECTED_BG } else { Color::Reset })
                .add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() });

            Row::new(vec![
                ratatui::widgets::Cell::from(format!("{}", e.pid)).style(pid),
                ratatui::widgets::Cell::from(e.command.as_str()).style(name_st),
                ratatui::widgets::Cell::from(port_text).style(if is_conflict { conflict_st } else { st }),
                ratatui::widgets::Cell::from(cpu_text).style(resource_st),
                ratatui::widgets::Cell::from(mem_text).style(resource_st),
                ratatui::widgets::Cell::from(e.address.as_str()).style(st),
                ratatui::widgets::Cell::from(e.user.as_str()).style(st),
                ratatui::widgets::Cell::from(e.ip_version.as_str()).style(st),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(8),
        Constraint::Min(10),
        Constraint::Length(7),
        Constraint::Length(6),
        Constraint::Length(7),
        Constraint::Length(18),
        Constraint::Length(12),
        Constraint::Length(6),
    ];

    let table_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(69, 71, 90)))
        .title(Line::from(vec![
            Span::styled(" Listening TCP ", Style::default().fg(Color::Rgb(166, 218, 239))),
            Span::styled(
                format!(" ({}) ", visible.len()),
                Style::default().fg(FOOTER_FG),
            ),
        ]));

    let inner = table_block.inner(area);
    f.render_widget(table_block, area);

    if visible.is_empty() {
        let hint = if app.last_error.is_some() {
            "Could not read ports. Check stderr / run `lsof -iTCP -sTCP:LISTEN -nP` manually. [r] refresh"
        } else {
            "No matching listening TCP ports. [/] filter  [r] refresh"
        };
        let msg = Paragraph::new(hint)
            .style(Style::default().fg(FOOTER_FG))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        f.render_widget(msg, inner);
        return;
    }

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .row_highlight_style(
            Style::default()
                .bg(ROW_SELECTED_BG)
                .fg(ROW_SELECTED_FG)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(table, inner, table_state);

    // Scrollbar: track visible slice vs full list (TableState handles row window)
    let row_count = visible.len();
    if row_count > inner.height.saturating_sub(2) as usize && inner.height > 2 {
        let mut sb = ScrollbarState::new(row_count).position(table_state.selected().unwrap_or(0));
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(Style::default().fg(Color::Rgb(69, 71, 90)))
            .thumb_style(Style::default().fg(Color::Rgb(137, 180, 250)));
        f.render_stateful_widget(
            scrollbar,
            inner.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut sb,
        );
    }
}

fn key_span(key: &str, desc: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!("[{}] ", key),
            Style::default().fg(KEY_ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{}  ", desc), Style::default().fg(FOOTER_FG)),
    ]
}

fn render_footer(f: &mut Frame<'_>, _app: &App, area: Rect, filter_mode: bool) {
    let mut spans: Vec<Span> = Vec::new();
    if filter_mode {
        spans.extend([
            Span::styled("filter  ", Style::default().fg(Color::Rgb(249, 226, 175))),
            Span::styled("type to narrow • ", Style::default().fg(FOOTER_FG)),
        ]);
        spans.extend(key_span("Enter", "apply"));
        spans.extend(key_span("Esc", "exit filter"));
    } else {
        spans.extend(key_span("q", "quit"));
        spans.extend(key_span("j/k", "nav"));
        spans.extend(key_span("K/⏎", "kill"));
        spans.extend(key_span("d", "details"));
        spans.extend(key_span("w", "watch"));
        spans.extend(key_span("r", "refresh"));
        spans.extend(key_span("s", "sort"));
        spans.extend(key_span("/", "filter"));
    }

    let line = Line::from(spans);
    let p = Paragraph::new(line).wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn render_filter_line(f: &mut Frame<'_>, app: &App, area: Rect) {
    // Must stay single-line: any Block borders need extra rows; with Length(1) the inner
    // area was empty and the typed filter drew past the bottom of the terminal.
    let text = format!(" /{}", app.filter);
    let p = Paragraph::new(Line::from(vec![Span::styled(
        text,
        Style::default()
            .fg(Color::Rgb(205, 214, 244))
            .bg(Color::Rgb(24, 24, 37))
            .add_modifier(Modifier::BOLD),
    )]))
    .style(Style::default().bg(Color::Rgb(24, 24, 37)));
    f.render_widget(p, area);
}

fn render_details(f: &mut Frame<'_>, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(69, 71, 90)))
        .title(Line::from(vec![
            Span::styled(
                " Details ",
                Style::default()
                    .fg(Color::Rgb(137, 180, 250))
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let entry = match app.selected_entry() {
        Some(e) => e,
        None => {
            let msg = Paragraph::new("No selection")
                .style(Style::default().fg(FOOTER_FG))
                .alignment(Alignment::Center);
            f.render_widget(msg, inner);
            return;
        }
    };

    let conflicts = app.conflicting_ports();
    let is_conflict = conflicts.contains(&entry.port);

    let category = match entry.port_category() {
        PortCategory::WellKnown => "well-known (0-1023)",
        PortCategory::Registered => "registered (1024-49151)",
        PortCategory::Dynamic => "dynamic (49152-65535)",
    };

    let mut detail_spans: Vec<Span> = vec![
        Span::styled("  Port: ", Style::default().fg(FOOTER_FG)),
        Span::styled(
            format!("{} ", entry.port),
            Style::default()
                .fg(Color::Rgb(205, 214, 244))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("({})  ", category),
            Style::default().fg(Color::Rgb(166, 218, 239)),
        ),
    ];

    if is_conflict {
        detail_spans.push(Span::styled(
            "CONFLICT: multiple PIDs on this port  ",
            Style::default()
                .fg(Color::Rgb(250, 179, 135))
                .add_modifier(Modifier::BOLD),
        ));
    }

    let mut extra_line_parts: Vec<Span> = Vec::new();
    if let Some((cached_pid, ref details)) = app.details_cache {
        if cached_pid == entry.pid {
            if let Some(ppid) = details.parent_pid {
                extra_line_parts.push(Span::styled("  Parent PID: ", Style::default().fg(FOOTER_FG)));
                extra_line_parts.push(Span::styled(
                    format!("{}  ", ppid),
                    Style::default().fg(Color::Rgb(205, 214, 244)),
                ));
            }
            if let Some(fds) = details.open_files {
                extra_line_parts.push(Span::styled("Open FDs: ", Style::default().fg(FOOTER_FG)));
                extra_line_parts.push(Span::styled(
                    format!("{}  ", fds),
                    Style::default().fg(Color::Rgb(205, 214, 244)),
                ));
            }
            if let Some(conns) = details.established_conns {
                extra_line_parts.push(Span::styled("Established: ", Style::default().fg(FOOTER_FG)));
                extra_line_parts.push(Span::styled(
                    format!("{}", conns),
                    Style::default().fg(if conns > 0 {
                        Color::Rgb(166, 227, 161)
                    } else {
                        Color::Rgb(205, 214, 244)
                    }),
                ));
            }
        }
    }

    let lines = vec![
        Line::from(detail_spans),
        Line::from(extra_line_parts),
    ];

    let p = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(p, inner);
}

fn render_kill_popup(f: &mut Frame<'_>, area: Rect, prompt: &crate::app::KillPrompt) {
    let (pid, cmd) = match prompt {
        crate::app::KillPrompt::Pending { pid, command } => (*pid, command.as_str()),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(243, 139, 168)))
        .title(" Confirm ")
        .title_style(
            Style::default()
                .fg(Color::Rgb(243, 139, 168))
                .add_modifier(Modifier::BOLD),
        );

    let text = format!(
        "Kill PID {} ({})?\n\n[y] SIGTERM    [f] SIGKILL (force)    [n] cancel",
        pid, cmd
    );

    let popup_w = (text.lines().map(|l| l.len()).max().unwrap_or(40) + 4).min(area.width as usize) as u16;
    let popup_h = 7u16;
    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(popup_w)) / 2,
        y: area.y + (area.height.saturating_sub(popup_h)) / 2,
        width: popup_w,
        height: popup_h,
    };

    f.render_widget(Clear, popup_area);
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let p = Paragraph::new(text)
        .style(Style::default().fg(Color::Rgb(205, 214, 244)))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(p, inner);
}

pub fn draw_help(f: &mut Frame<'_>, show: bool) {
    if !show {
        return;
    }
    let area = f.area();
    let text = concat!(
        "portui — listening TCP ports\n\n",
        "q / Esc     Quit\n",
        "j / ↓       Move down\n",
        "k / ↑       Move up\n",
        "Enter / K   Kill selected process\n",
        "              y = SIGTERM  f = SIGKILL\n",
        "d / Tab     Toggle details pane\n",
        "w           Toggle auto-refresh (2s)\n",
        "r           Manual refresh\n",
        "s           Cycle sort: port → pid → name\n",
        "/           Filter by name, user, port, pid\n",
        "g / Home    Jump to top\n",
        "G / End     Jump to bottom\n",
        "?           Toggle this help\n\n",
        "Port conflict: ! marks ports with\n",
        "multiple processes listening\n",
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .border_style(Style::default().fg(Color::Rgb(137, 180, 250)));

    let w = 50.min(area.width);
    let h = 24.min(area.height);
    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup_area);
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);
    let p = Paragraph::new(text)
        .style(Style::default().fg(Color::Rgb(205, 214, 244)))
        .wrap(Wrap { trim: true });
    f.render_widget(p, inner);
}
