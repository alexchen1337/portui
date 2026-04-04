//! Parse listening TCP ports from `lsof`.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortEntry {
    pub command: String,
    pub pid: u32,
    pub user: String,
    pub port: u16,
    pub address: String,
    /// "IPv4" or "IPv6"
    pub ip_version: String,
}

impl PortEntry {
    pub fn port_category(&self) -> PortCategory {
        match self.port {
            0..=1023 => PortCategory::WellKnown,
            1024..=49151 => PortCategory::Registered,
            _ => PortCategory::Dynamic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortCategory {
    WellKnown,
    Registered,
    Dynamic,
}

/// Run `lsof` and return deduplicated listening TCP entries, sorted by port then PID.
pub fn scan_ports() -> Result<Vec<PortEntry>> {
    let output = Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-nP"])
        .output()
        .context("failed to run `lsof`; is it installed?")?;

    if !output.status.success() {
        anyhow::bail!(
            "lsof exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    let mut seen = HashSet::new();

    for line in text.lines().skip(1) {
        if let Some(entry) = parse_lsof_line(line) {
            let key = (entry.pid, entry.port, entry.address.clone(), entry.ip_version.clone());
            if seen.insert(key) {
                entries.push(entry);
            }
        }
    }

    entries.sort_by(|a, b| {
        a.port
            .cmp(&b.port)
            .then_with(|| a.pid.cmp(&b.pid))
            .then_with(|| a.command.cmp(&b.command))
    });

    Ok(entries)
}

fn parse_lsof_line(line: &str) -> Option<PortEntry> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 9 {
        return None;
    }

    let command = parts[0].to_string();
    let pid: u32 = parts[1].parse().ok()?;
    let user = parts[2].to_string();

    // Find "TCP" token; next token is host:port
    let tcp_idx = parts.iter().position(|&p| p == "TCP")?;
    let name_part = parts.get(tcp_idx + 1)?;
    if !name_part.contains(':') {
        return None;
    }

    let (address, port) = split_host_port(name_part)?;
    let ip_version = if parts.contains(&"IPv6") {
        "IPv6".to_string()
    } else {
        "IPv4".to_string()
    };

    Some(PortEntry {
        command,
        pid,
        user,
        port,
        address: normalize_address(address),
        ip_version,
    })
}

/// Split `host:port` where host may be IPv6 in brackets, e.g. `[::1]:8080`.
fn split_host_port(s: &str) -> Option<(&str, u16)> {
    if let Some(bracket_end) = s.rfind(']') {
        if s.len() > bracket_end + 1 && s.as_bytes().get(bracket_end + 1) == Some(&b':') {
            let port_str = &s[bracket_end + 2..];
            let port: u16 = port_str.parse().ok()?;
            return Some((&s[..=bracket_end], port));
        }
    }
    let colon = s.rfind(':')?;
    let host = &s[..colon];
    let port: u16 = s[colon + 1..].parse().ok()?;
    Some((host, port))
}

fn normalize_address(host: &str) -> String {
    match host {
        "*" => "*".to_string(),
        "::" | "[::]" => "*".to_string(),
        h if h.starts_with('[') && h.ends_with(']') => h.to_string(),
        h => h.to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct ProcessDetails {
    pub parent_pid: Option<u32>,
    pub open_files: Option<usize>,
    pub established_conns: Option<usize>,
}

pub fn get_process_details(pid: u32, port: u16) -> ProcessDetails {
    ProcessDetails {
        parent_pid: get_parent_pid(pid),
        open_files: get_open_file_count(pid),
        established_conns: get_established_connections(port),
    }
}

fn get_parent_pid(pid: u32) -> Option<u32> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "ppid="])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

fn get_open_file_count(pid: u32) -> Option<usize> {
    let output = Command::new("lsof")
        .args(["-p", &pid.to_string()])
        .output()
        .ok()?;
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .count()
            .saturating_sub(1),
    )
}

fn get_established_connections(port: u16) -> Option<usize> {
    let port_str = format!(":{}", port);
    let output = Command::new("lsof")
        .args(["-iTCP", "-sTCP:ESTABLISHED", "-nP"])
        .output()
        .ok()?;
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .skip(1)
            .filter(|line| line.contains(&port_str))
            .count(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ipv4_line() {
        let line = "node    12345 alex   21u  IPv4 0x12345678      0t0  TCP *:3000 (LISTEN)";
        let e = parse_lsof_line(line).unwrap();
        assert_eq!(e.command, "node");
        assert_eq!(e.pid, 12345);
        assert_eq!(e.user, "alex");
        assert_eq!(e.port, 3000);
        assert_eq!(e.address, "*");
    }

    #[test]
    fn parse_ipv6_line() {
        let line = "nginx   99 root   6u  IPv6 0xabc      0t0  TCP [::1]:443 (LISTEN)";
        let e = parse_lsof_line(line).unwrap();
        assert_eq!(e.port, 443);
        assert_eq!(e.address, "[::1]");
    }

    #[test]
    fn scan_ports_smoke() {
        let r = scan_ports();
        assert!(r.is_ok(), "scan_ports: {:?}", r.err());
    }
}
