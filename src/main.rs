//! portui — TUI for listening TCP ports (lsof) with kill (SIGTERM).

mod app;
mod ports;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::TableState;
use ratatui::Terminal;
use std::io::{self, stdout};
use std::time::Duration;

fn main() -> Result<()> {
    run()
}

fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    app.refresh_ports();

    let mut table_state = TableState::default();
    let mut show_help = false;
    let poll_ms = 250u64;

    loop {
        terminal.draw(|f| {
            ui::draw(f, &app, &mut table_state, None);
            ui::draw_help(f, show_help);
        })?;

        if app.should_auto_refresh() {
            app.refresh_ports();
        }
        app.advance_tick();

        if event::poll(Duration::from_millis(poll_ms))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Release {
                    continue;
                }

                if show_help {
                    if matches!(key.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?')) {
                        show_help = false;
                    }
                    continue;
                }

                if app.kill_prompt.is_some() {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            if let Some(pid) = app.confirm_kill() {
                                let _ = signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                                app.refresh_ports();
                            }
                        }
                        KeyCode::Char('f') | KeyCode::Char('F') => {
                            if let Some(pid) = app.confirm_kill() {
                                let _ = signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
                                app.refresh_ports();
                            }
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            app.dismiss_kill_prompt();
                        }
                        _ => {}
                    }
                    continue;
                }

                if app.filter_editing {
                    match key.code {
                        KeyCode::Esc => {
                            app.filter_editing = false;
                            app.on_filter_changed();
                        }
                        KeyCode::Enter => {
                            app.filter_editing = false;
                            app.on_filter_changed();
                        }
                        KeyCode::Backspace => {
                            app.filter.pop();
                            app.on_filter_changed();
                        }
                        KeyCode::Char(c) => {
                            app.filter.push(c);
                            app.on_filter_changed();
                        }
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('?') => show_help = !show_help,
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Home | KeyCode::Char('g') => app.jump_top(),
                    KeyCode::End | KeyCode::Char('G') => app.jump_bottom(),
                    KeyCode::Char('r') => app.refresh_ports(),
                    KeyCode::Char('s') => app.cycle_sort(),
                    KeyCode::Char('/') => {
                        app.filter_editing = true;
                    }
                    KeyCode::Char('d') | KeyCode::Tab => app.toggle_details(),
                    KeyCode::Char('w') => app.toggle_auto_refresh(),
                    KeyCode::Enter | KeyCode::Char('K') => app.open_kill_prompt(),
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}
