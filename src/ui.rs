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

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(if app.filter_editing { 3 } else { 2 }),
        ])
        .split(area);

    let header_area = main_chunks[0];
    let table_block_area = main_chunks[1];
    let footer_area = main_chunks[2];

    render_header(f, app, header_area);
    render_table(
        f,
        app,
        table_state,
        table_area_override.unwrap_or(table_block_area),
    );

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

fn render_header(f: &mut Frame<'_>, app: &App, area: Rect) {
    let title = Line::from(vec![
        Span::styled(
            " PORT CLI ",
            Style::default()
                .fg(HEADER_FG)
                .bg(HEADER_BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  sort:{}  ", app.sort_mode.label()),
            Style::default().fg(Color::Rgb(166, 218, 239)).bg(HEADER_BG),
        ),
        Span::styled(
            if app.last_error.is_some() {
                "  ! scan error  "
            } else {
                "  refresh: 2s  "
            },
            Style::default().fg(Color::Rgb(243, 139, 168)).bg(HEADER_BG),
        ),
        Span::styled("  [?] help ", Style::default().fg(FOOTER_FG).bg(HEADER_BG)),
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
    if visible.is_empty() {
        table_state.select(None);
    } else {
        table_state.select(Some(app.selected));
    }
    let header_style = Style::default()
        .fg(HEADER_FG)
        .bg(HEADER_BG)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec!["PID", "Process", "Port", "Address", "User", "Proto"])
        .style(header_style)
        .height(1);

    let rows: Vec<Row> = visible
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let selected = i == app.selected;
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

            Row::new(vec![
                ratatui::widgets::Cell::from(format!("{}", e.pid)).style(pid),
                ratatui::widgets::Cell::from(e.command.as_str()).style(name_st),
                ratatui::widgets::Cell::from(format!("{}", e.port)).style(st),
                ratatui::widgets::Cell::from(e.address.as_str()).style(st),
                ratatui::widgets::Cell::from(e.user.as_str()).style(st),
                ratatui::widgets::Cell::from(e.ip_version.as_str()).style(st),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(8),
        Constraint::Min(12),
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
        spans.extend(key_span("j/↓", "down"));
        spans.extend(key_span("k/↑", "up"));
        spans.extend(key_span("K/⏎", "kill"));
        spans.extend(key_span("r", "refresh"));
        spans.extend(key_span("s", "sort"));
        spans.extend(key_span("/", "filter"));
        spans.extend(key_span("g/G", "top/end"));
    }

    let line = Line::from(spans);
    let p = Paragraph::new(line).wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn render_filter_line(f: &mut Frame<'_>, app: &App, area: Rect) {
    let text = format!(" /{}", app.filter);
    let p = Paragraph::new(Line::from(vec![Span::styled(
        text,
        Style::default()
            .fg(Color::Rgb(205, 214, 244))
            .add_modifier(Modifier::BOLD),
    )]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(137, 180, 250))),
    );
    f.render_widget(p, area);
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

    let text = format!("Send SIGTERM to PID {} ({})?\n\n[y] yes   [n] no   [Esc] cancel", pid, cmd);

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
        "port-cli — listening TCP ports\n\n",
        "q / Esc     Quit\n",
        "j / ↓       Move down\n",
        "k / ↑       Move up\n",
        "Enter / K   Kill selected (SIGTERM)\n",
        "r           Refresh list\n",
        "s           Cycle sort: port → pid → name\n",
        "/           Filter by name, user, port, pid\n",
        "g / Home    Jump to top\n",
        "G / End     Jump to bottom\n",
        "?           Toggle this help\n",
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .border_style(Style::default().fg(Color::Rgb(137, 180, 250)));

    let w = 50.min(area.width);
    let h = 18.min(area.height);
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
