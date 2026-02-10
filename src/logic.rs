use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;
use tokio::process::Command;
use tracing::{debug, error, warn};

use crate::error::AppError;
use crate::fl;

static IP_REGEX: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}").unwrap());

static HOSTNAME_REGEX: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\w+\.[\w.]+\.ts\.net").unwrap());

/// All Tailscale state fetched in one batch.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct TailscaleState {
  pub ip: String,
  pub connected: bool,
  pub ssh_enabled: bool,
  pub routes_enabled: bool,
  pub is_exit_node: bool,
  pub devices: Vec<String>,
  pub exit_nodes: Vec<String>,
  pub acct_list: Vec<String>,
  pub current_acct: String,
}

/// Parsed preferences from `tailscale debug prefs`.
#[allow(clippy::struct_excessive_bools)]
struct TailscalePrefs {
  want_running: bool,
  run_ssh: bool,
  route_all: bool,
  is_exit_node: bool,
}

/// Fetch all preferences from a single `tailscale debug prefs` call.
async fn fetch_tailscale_prefs() -> Result<TailscalePrefs, AppError> {
  let output = run_tailscale_cmd(&["debug", "prefs"]).await?;
  let prefs: Value = serde_json::from_str(&output)?;

  Ok(TailscalePrefs {
    want_running: prefs.get("WantRunning").and_then(Value::as_bool).unwrap_or(false),
    run_ssh: prefs.get("RunSSH").and_then(Value::as_bool).unwrap_or(false),
    route_all: prefs.get("RouteAll").and_then(Value::as_bool).unwrap_or(false),
    is_exit_node: prefs
      .get("AdvertiseRoutes")
      .is_some_and(|v| !v.is_null() && v.as_str() != Some("")),
  })
}

/// Fetch all Tailscale state in one async batch.
pub async fn fetch_tailscale_state() -> Result<TailscaleState, AppError> {
  let ip = get_tailscale_ip().await.unwrap_or_else(|e| {
    warn!("Failed to get IP: {e}");
    fl!("not-available")
  });

  let prefs = fetch_tailscale_prefs().await.unwrap_or_else(|e| {
    warn!("Failed to fetch prefs: {e}");
    TailscalePrefs {
      want_running: false,
      run_ssh: false,
      route_all: false,
      is_exit_node: false,
    }
  });

  let devices = get_tailscale_devices().await.unwrap_or_else(|e| {
    warn!("Failed to get devices: {e}");
    vec!["Select".to_string()]
  });

  let exit_nodes = if prefs.is_exit_node {
    vec![fl!("exit-node-is-host")]
  } else {
    get_avail_exit_nodes().await.unwrap_or_else(|e| {
      warn!("Failed to get exit nodes: {e}");
      vec!["None".to_string()]
    })
  };

  let acct_list = get_acct_list().await.unwrap_or_default();
  let current_acct = get_current_acct().await.unwrap_or_default();

  Ok(TailscaleState {
    ip,
    connected: prefs.want_running,
    ssh_enabled: prefs.run_ssh,
    routes_enabled: prefs.route_all,
    is_exit_node: prefs.is_exit_node,
    devices,
    exit_nodes,
    acct_list,
    current_acct,
  })
}

/// Run a tailscale CLI command and return stdout, checking the exit code.
async fn run_tailscale_cmd(args: &[&str]) -> Result<String, AppError> {
  let output = Command::new("tailscale")
    .args(args)
    .output()
    .await?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(AppError::CliFailure(format!(
      "tailscale {} exited with {}: {}",
      args.join(" "),
      output.status,
      stderr.trim()
    )));
  }

  Ok(String::from_utf8(output.stdout)?)
}

/// Get the IPv4 address assigned to this computer.
pub async fn get_tailscale_ip() -> Result<String, AppError> {
  let ip = run_tailscale_cmd(&["ip", "-4"]).await?;
  Ok(ip.trim().to_string())
}

pub async fn get_tailscale_devices() -> Result<Vec<String>, AppError> {
  let out = run_tailscale_cmd(&["status"]).await?;

  let mut devices: Vec<String> = out
    .lines()
    .filter(|line| IP_REGEX.is_match(line))
    .filter_map(|line| line.split_whitespace().nth(1).map(std::string::ToString::to_string))
    .collect();

  if !devices.is_empty() {
    devices.remove(0);
  }
  devices.insert(0, "Select".to_string());

  Ok(devices)
}

/// Set the Tailscale connection up/down
pub async fn tailscale_int_up(up: bool) -> Result<(), AppError> {
  let arg = if up { "up" } else { "down" };
  run_tailscale_cmd(&[arg]).await?;
  Ok(())
}

/// Send files through Tail Drop
pub async fn tailscale_send(file_paths: &[PathBuf], target: &str) -> Option<String> {
  let mut errors = Vec::new();

  for path in file_paths {
    let p = path.to_string_lossy();
    match Command::new("tailscale")
      .args(["file", "cp", &*p, &format!("{target}:")])
      .output()
      .await
    {
      Ok(output) => {
        if !output.status.success() || !output.stderr.is_empty() {
          let err = String::from_utf8_lossy(&output.stderr).to_string();
          warn!("Error sending file {p}: {err}");
          errors.push(err);
        }
      }
      Err(e) => {
        error!("Failed to execute tailscale file cp: {e}");
        errors.push(format!("Failed to send {p}: {e}"));
      }
    }
  }

  if !errors.is_empty() {
    return Some(fl!("send-files-partial-fail"));
  }

  None
}

/// Receive files through Tail Drop (with 30-second timeout).
pub async fn tailscale_receive() -> String {
  let Some(download_path) = dirs::download_dir() else {
    return fl!("no-downloads-dir");
  };

  let download_str = download_path.to_string_lossy().to_string();

  let receive_fut = Command::new("tailscale")
    .args(["file", "get", &download_str])
    .output();

  match tokio::time::timeout(std::time::Duration::from_secs(30), receive_fut).await {
    Ok(Ok(output)) => {
      if output.status.success() && output.stderr.is_empty() {
        fl!("received-files-success")
      } else {
        String::from_utf8_lossy(&output.stderr).to_string()
      }
    }
    Ok(Err(e)) => format!("Failed to receive files: {e}"),
    Err(_) => String::from("No files received (timed out after 30s)"),
  }
}

pub async fn clear_status(wait_time: u64) -> Option<String> {
  tokio::time::sleep(std::time::Duration::from_secs(wait_time)).await;
  None
}

/// Toggle a tailscale flag on/off
async fn set_tailscale_flag(flag: &str, enabled: bool) -> Result<(), AppError> {
  let value = if enabled {
    format!("--{flag}")
  } else {
    format!("--{flag}=false")
  };

  run_tailscale_cmd(&["set", &value]).await?;
  Ok(())
}

/// Toggle SSH on/off
pub async fn set_ssh(ssh: bool) -> Result<(), AppError> {
  set_tailscale_flag("ssh", ssh).await
}

/// Toggle accept-routes on/off
pub async fn set_routes(accept_routes: bool) -> Result<(), AppError> {
  set_tailscale_flag("accept-routes", accept_routes).await
}

/// Make current host an exit node
pub async fn enable_exit_node(is_exit_node: bool) -> Result<(), AppError> {
  let flag = format!("--advertise-exit-node={is_exit_node}");
  run_tailscale_cmd(&["set", &flag]).await?;
  tailscale_int_up(true).await
}

/// Add/remove exit node's access to the host's local LAN
pub async fn exit_node_allow_lan_access(is_allowed: bool) -> Result<(), AppError> {
  let flag = format!("--exit-node-allow-lan-access={is_allowed}");
  run_tailscale_cmd(&["set", &flag]).await?;
  Ok(())
}

/// Get available exit nodes
pub async fn get_avail_exit_nodes() -> Result<Vec<String>, AppError> {
  let exit_node_list_string = run_tailscale_cmd(&["exit-node", "list"]).await?;

  if exit_node_list_string.is_empty() {
    debug!("No exit nodes found");
    return Ok(vec![fl!("no-exit-nodes")]);
  }

  let mut exit_node_list: Vec<String> = vec!["None".to_string()];

  let nodes: Vec<String> = exit_node_list_string
    .lines()
    .filter(|line| HOSTNAME_REGEX.is_match(line))
    .filter_map(|hostname| {
      hostname
        .split_whitespace()
        .nth(1)
        .and_then(|fqdn| fqdn.split('.').next())
        .map(std::string::ToString::to_string)
    })
    .collect();

  exit_node_list.extend(nodes);
  Ok(exit_node_list)
}

/// Set selected exit node as the exit node through Tailscale CLI
pub async fn set_exit_node(exit_node: &str) -> Result<(), AppError> {
  let flag = format!("--exit-node={exit_node}");
  run_tailscale_cmd(&["set", &flag]).await?;
  Ok(())
}

pub async fn switch_accounts(acct_name: &str) -> Result<bool, AppError> {
  let output = run_tailscale_cmd(&["switch", acct_name]).await?;
  Ok(output.to_lowercase().contains("success"))
}

pub async fn get_acct_list() -> Result<Vec<String>, AppError> {
  let accts_str = run_tailscale_cmd(&["switch", "--list"]).await?;

  let ret_accts: Vec<String> = accts_str
    .lines()
    .filter(|line| !line.to_lowercase().starts_with("id"))
    .filter_map(|acct| acct.split_whitespace().nth(1).map(std::string::ToString::to_string))
    .collect();

  Ok(ret_accts)
}

/// Get the current account name from `tailscale status --json`.
pub async fn get_current_acct() -> Result<String, AppError> {
  let output = run_tailscale_cmd(&["status", "--json"]).await?;
  let status: Value = serde_json::from_str(&output)?;

  let acct = status
    .get("Self")
    .and_then(|s| s.get("DNSName"))
    .and_then(Value::as_str)
    .map(|dns| dns.trim_end_matches('.').to_string())
    .unwrap_or_default();

  Ok(acct)
}
