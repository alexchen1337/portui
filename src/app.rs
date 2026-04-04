//! Application state: selection, sort, filter, kill confirmation.

use crate::ports::{PortEntry, scan_ports};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Port,
    Pid,
    Name,
}

impl SortMode {
    pub fn next(self) -> Self {
        match self {
            SortMode::Port => SortMode::Pid,
            SortMode::Pid => SortMode::Name,
            SortMode::Name => SortMode::Port,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortMode::Port => "port",
            SortMode::Pid => "pid",
            SortMode::Name => "name",
        }
    }
}

#[derive(Debug, Clone)]
pub enum KillPrompt {
    Pending { pid: u32, command: String },
}

pub struct App {
    /// Raw scan result (last successful refresh).
    pub ports: Vec<PortEntry>,
    pub last_error: Option<String>,
    pub sort_mode: SortMode,
    /// Filter substring (case-insensitive); empty = no filter.
    pub filter: String,
    pub filter_editing: bool,
    /// Index into `visible_ports()` list.
    pub selected: usize,
    pub kill_prompt: Option<KillPrompt>,
    /// Ticks since start; used for auto-refresh (4 ticks × 250ms = 1s, plan said 2s → 8 ticks).
    pub tick: u64,
}

const REFRESH_TICKS: u64 = 8; // 8 × 250ms = 2s

impl App {
    pub fn new() -> Self {
        Self {
            ports: Vec::new(),
            last_error: None,
            sort_mode: SortMode::Port,
            filter: String::new(),
            filter_editing: false,
            selected: 0,
            kill_prompt: None,
            tick: 0,
        }
    }

    pub fn refresh_ports(&mut self) {
        match scan_ports() {
            Ok(list) => {
                self.last_error = None;
                self.ports = list;
                self.apply_sort();
                self.clamp_selection();
            }
            Err(e) => {
                self.last_error = Some(e.to_string());
            }
        }
    }

    /// Returns true if periodic refresh should run this tick.
    pub fn should_auto_refresh(&self) -> bool {
        self.tick > 0 && self.tick.is_multiple_of(REFRESH_TICKS)
    }

    pub fn advance_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    fn sorted_ports(&self) -> Vec<PortEntry> {
        let mut v = self.ports.clone();
        match self.sort_mode {
            SortMode::Port => v.sort_by(|a, b| {
                a.port
                    .cmp(&b.port)
                    .then_with(|| a.pid.cmp(&b.pid))
                    .then_with(|| a.command.cmp(&b.command))
            }),
            SortMode::Pid => v.sort_by(|a, b| {
                a.pid
                    .cmp(&b.pid)
                    .then_with(|| a.port.cmp(&b.port))
                    .then_with(|| a.command.cmp(&b.command))
            }),
            SortMode::Name => v.sort_by(|a, b| {
                a.command
                    .to_lowercase()
                    .cmp(&b.command.to_lowercase())
                    .then_with(|| a.port.cmp(&b.port))
                    .then_with(|| a.pid.cmp(&b.pid))
            }),
        }
        v
    }

    fn apply_sort(&mut self) {
        let sorted = self.sorted_ports();
        self.ports = sorted;
    }

    pub fn cycle_sort(&mut self) {
        self.sort_mode = self.sort_mode.next();
        self.apply_sort();
        self.clamp_selection();
    }

    fn matches_filter(entry: &PortEntry, filter: &str) -> bool {
        if filter.is_empty() {
            return true;
        }
        let f = filter.to_lowercase();
        entry.command.to_lowercase().contains(&f)
            || entry.user.to_lowercase().contains(&f)
            || entry.address.to_lowercase().contains(&f)
            || entry.port.to_string().contains(&f)
            || entry.pid.to_string().contains(&f)
    }

    pub fn visible_ports(&self) -> Vec<&PortEntry> {
        self.ports
            .iter()
            .filter(|e| Self::matches_filter(e, &self.filter))
            .collect()
    }

    pub fn clamp_selection(&mut self) {
        let n = self.visible_ports().len();
        if n == 0 {
            self.selected = 0;
        } else if self.selected >= n {
            self.selected = n - 1;
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        let n = self.visible_ports().len();
        if n > 0 {
            self.selected = (self.selected + 1).min(n - 1);
        }
    }

    pub fn jump_top(&mut self) {
        self.selected = 0;
    }

    pub fn jump_bottom(&mut self) {
        let n = self.visible_ports().len();
        if n > 0 {
            self.selected = n - 1;
        }
    }

    pub fn selected_entry(&self) -> Option<&PortEntry> {
        self.visible_ports().get(self.selected).copied()
    }

    pub fn open_kill_prompt(&mut self) {
        if let Some(e) = self.selected_entry() {
            self.kill_prompt = Some(KillPrompt::Pending {
                pid: e.pid,
                command: e.command.clone(),
            });
        }
    }

    pub fn dismiss_kill_prompt(&mut self) {
        self.kill_prompt = None;
    }

    pub fn confirm_kill(&mut self) -> Option<u32> {
        let pid = match &self.kill_prompt {
            Some(KillPrompt::Pending { pid, .. }) => *pid,
            None => return None,
        };
        self.kill_prompt = None;
        Some(pid)
    }

    pub fn on_filter_changed(&mut self) {
        self.clamp_selection();
    }
}
