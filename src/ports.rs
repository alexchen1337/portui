//! Parse listening TCP ports from `lsof`.

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub struct PortEntry {
    pub command: String,
    pub pid: u32,
    pub user: String,
    pub port: u16,
    pub address: String,
    /// "IPv4" or "IPv6"
    pub ip_version: String,
    /// CPU usage percentage (from `ps`)
    pub cpu_pct: f32,
    /// Resident memory in MiB (from `ps`)
    pub mem_mib: f64,
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

    enrich_with_resource_usage(&mut entries);

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
        cpu_pct: 0.0,
        mem_mib: 0.0,
    })
}

/// Overall system resource usage.
#[derive(Debug, Clone, Copy)]
pub struct SystemStats {
    pub cpu_pct: f32,
    pub mem_used_gib: f64,
    pub mem_total_gib: f64,
}

/// Fetch system-wide CPU and memory usage (macOS).
pub fn system_stats(total_mem_gib: f64) -> SystemStats {
    SystemStats {
        cpu_pct: system_cpu_pct(),
        mem_used_gib: system_used_mem_gib(),
        mem_total_gib: total_mem_gib,
    }
}

/// Total physical memory in GiB via sysctl.
pub fn total_memory_gib() -> f64 {
    Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok())
        .map(|bytes| bytes / (1024.0 * 1024.0 * 1024.0))
        .unwrap_or(0.0)
}

/// Parse overall CPU% from `top -l 1 -n 0 -s 0`.
fn system_cpu_pct() -> f32 {
    let output = Command::new("top")
        .args(["-l", "1", "-n", "0", "-s", "0"])
        .output()
        .ok();
    let Some(output) = output else { return 0.0 };
    let text = String::from_utf8_lossy(&output.stdout);
    // Line: "CPU usage: 5.26% user, 3.45% sys, 91.28% idle"
    for line in text.lines() {
        if line.starts_with("CPU usage:") {
            let mut user = 0.0f32;
            let mut sys = 0.0f32;
            for part in line.split(',') {
                let part = part.trim();
                if part.contains("user") {
                    user = part.split_whitespace()
                        .find_map(|w| w.trim_end_matches('%').parse::<f32>().ok())
                        .unwrap_or(0.0);
                } else if part.contains("sys") {
                    sys = part.split_whitespace()
                        .find_map(|w| w.trim_end_matches('%').parse::<f32>().ok())
                        .unwrap_or(0.0);
                }
            }
            return user + sys;
        }
    }
    0.0
}

/// Parse used memory from `vm_stat` (active + wired + compressor pages).
fn system_used_mem_gib() -> f64 {
    let output = Command::new("vm_stat").output().ok();
    let Some(output) = output else { return 0.0 };
    let text = String::from_utf8_lossy(&output.stdout);

    let page_size: f64 = Command::new("sysctl")
        .args(["-n", "vm.pagesize"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(16384.0);

    let mut active: f64 = 0.0;
    let mut wired: f64 = 0.0;
    let mut compressor: f64 = 0.0;

    for line in text.lines() {
        let parse_pages = |l: &str| -> f64 {
            l.split(':')
                .nth(1)
                .and_then(|v| v.trim().trim_end_matches('.').parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        if line.starts_with("Pages active:") {
            active = parse_pages(line);
        } else if line.starts_with("Pages wired down:") {
            wired = parse_pages(line);
        } else if line.starts_with("Pages occupied by compressor:") {
            compressor = parse_pages(line);
        }
    }

    (active + wired + compressor) * page_size / (1024.0 * 1024.0 * 1024.0)
}

/// Batch-fetch CPU% and RSS for all PIDs using a single `ps` invocation.
fn enrich_with_resource_usage(entries: &mut [PortEntry]) {
    let pids: Vec<String> = entries.iter().map(|e| e.pid.to_string()).collect::<HashSet<_>>().into_iter().collect();
    if pids.is_empty() {
        return;
    }

    let Ok(output) = Command::new("ps")
        .args(["-p", &pids.join(","), "-o", "pid=,pcpu=,rss="])
        .output()
    else {
        return;
    };

    let mut stats: HashMap<u32, (f32, f64)> = HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            if let (Ok(pid), Ok(cpu), Ok(rss_kb)) = (
                parts[0].parse::<u32>(),
                parts[1].parse::<f32>(),
                parts[2].parse::<f64>(),
            ) {
                stats.insert(pid, (cpu, rss_kb / 1024.0));
            }
        }
    }

    for entry in entries.iter_mut() {
        if let Some(&(cpu, mem)) = stats.get(&entry.pid) {
            entry.cpu_pct = cpu;
            entry.mem_mib = mem;
        }
    }
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
        assert_eq!(e.cpu_pct, 0.0);
        assert_eq!(e.mem_mib, 0.0);
    }

    #[test]
    fn parse_ipv6_line() {
        let line = "nginx   99 root   6u  IPv6 0xabc      0t0  TCP [::1]:443 (LISTEN)";
        let e = parse_lsof_line(line).unwrap();
        assert_eq!(e.port, 443);
        assert_eq!(e.address, "[::1]");
        assert_eq!(e.cpu_pct, 0.0);
    }

    #[test]
    fn scan_ports_smoke() {
        let r = scan_ports();
        assert!(r.is_ok(), "scan_ports: {:?}", r.err());
    }
}
