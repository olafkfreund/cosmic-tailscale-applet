use std::sync::LazyLock;

use regex::Regex;
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

/// Fetch all Tailscale state in one async batch.
pub async fn fetch_tailscale_state() -> Result<TailscaleState, AppError> {
  let ip = get_tailscale_ip().await.unwrap_or_else(|e| {
    warn!("Failed to get IP: {e}");
    fl!("not-available")
  });

  let connected = get_tailscale_con_status().await.unwrap_or(false);
  let ssh_enabled = get_tailscale_ssh_status().await.unwrap_or(false);
  let routes_enabled = get_tailscale_routes_status().await.unwrap_or(false);
  let is_exit_node = get_is_exit_node().await.unwrap_or(false);

  let devices = get_tailscale_devices().await.unwrap_or_else(|e| {
    warn!("Failed to get devices: {e}");
    vec!["Select".to_string()]
  });

  let exit_nodes = if is_exit_node {
    vec![String::from(
      "Can't select an exit node\nwhile host is an exit node!",
    )]
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
    connected,
    ssh_enabled,
    routes_enabled,
    is_exit_node,
    devices,
    exit_nodes,
    acct_list,
    current_acct,
  })
}

/// Get the IPv4 address assigned to this computer.
pub async fn get_tailscale_ip() -> Result<String, AppError> {
  let ip_cmd = Command::new("tailscale")
    .args(["ip", "-4"])
    .output()
    .await?;

  let ip = String::from_utf8(ip_cmd.stdout)?;
  Ok(ip.trim().to_string())
}

/// Get a preference value from `tailscale debug prefs`.
async fn get_tailscale_pref(key: &str) -> Result<bool, AppError> {
  let prefs_cmd = Command::new("tailscale")
    .args(["debug", "prefs"])
    .output()
    .await?;

  let output = String::from_utf8(prefs_cmd.stdout)?;
  let line = output
    .lines()
    .find(|line| line.contains(key))
    .unwrap_or("");

  Ok(line.contains("true"))
}

/// Get Tailscale's connection status
pub async fn get_tailscale_con_status() -> Result<bool, AppError> {
  get_tailscale_pref("WantRunning").await
}

/// Get the current status of the SSH enablement
pub async fn get_tailscale_ssh_status() -> Result<bool, AppError> {
  get_tailscale_pref("RunSSH").await
}

/// Get the current status of the accept-routes enablement
pub async fn get_tailscale_routes_status() -> Result<bool, AppError> {
  get_tailscale_pref("RouteAll").await
}

pub async fn get_tailscale_devices() -> Result<Vec<String>, AppError> {
  let ts_status_cmd = Command::new("tailscale")
    .arg("status")
    .output()
    .await?;

  let out = String::from_utf8(ts_status_cmd.stdout)?;

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
  Command::new("tailscale").arg(arg).output().await?;
  Ok(())
}

/// Send files through Tail Drop
pub async fn tailscale_send(file_paths: Vec<Option<String>>, target: &str) -> Option<String> {
  let mut errors = Vec::new();

  for path in &file_paths {
    match path {
      Some(p) => {
        match Command::new("tailscale")
          .args(["file", "cp", p, &format!("{target}:")])
          .output()
          .await
        {
          Ok(output) => {
            if !output.stderr.is_empty() {
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
      None => {
        return Some(String::from(
          "Something went wrong sending the file!\nPossible bad file path!",
        ));
      }
    }
  }

  if !errors.is_empty() {
    return Some("One or more files were not sent successfully!".to_string());
  }

  None
}

/// Receive files through Tail Drop
pub async fn tailscale_receive() -> String {
  let Some(download_path) = dirs::download_dir() else {
    return "Could not determine Downloads directory!".to_string();
  };

  let download_str = download_path.to_string_lossy().to_string();

  match Command::new("tailscale")
    .args(["file", "get", &download_str])
    .output()
    .await
  {
    Ok(output) => {
      if output.stderr.is_empty() {
        "Received file(s) in Downloads!".to_string()
      } else {
        String::from_utf8_lossy(&output.stderr).to_string()
      }
    }
    Err(e) => format!("Failed to receive files: {e}"),
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

  let output = Command::new("tailscale")
    .args(["set", &value])
    .output()
    .await?;

  if !output.stderr.is_empty() {
    let err = String::from_utf8_lossy(&output.stderr).to_string();
    warn!("Error setting {flag} to {enabled}: {err}");
  }

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
  Command::new("tailscale")
    .args(["set", &format!("--advertise-exit-node={is_exit_node}")])
    .output()
    .await?;

  tailscale_int_up(true).await
}

/// Get the status of whether or not the host is an exit node
pub async fn get_is_exit_node() -> Result<bool, AppError> {
  let output = Command::new("tailscale")
    .args(["debug", "prefs"])
    .output()
    .await?;

  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let adv_rts = stdout
    .lines()
    .filter(|line| line.to_lowercase().contains("advertiseroutes"))
    .flat_map(|line| line.chars())
    .collect::<String>();

  Ok(!adv_rts.contains("null") && !adv_rts.is_empty())
}

/// Add/remove exit node's access to the host's local LAN
pub async fn exit_node_allow_lan_access(is_allowed: bool) -> Result<(), AppError> {
  Command::new("tailscale")
    .args([
      "set",
      &format!("--exit-node-allow-lan-access={is_allowed}"),
    ])
    .output()
    .await?;

  Ok(())
}

/// Get available exit nodes
pub async fn get_avail_exit_nodes() -> Result<Vec<String>, AppError> {
  let exit_node_list_cmd = Command::new("tailscale")
    .args(["exit-node", "list"])
    .output()
    .await?;

  let exit_node_list_string = String::from_utf8(exit_node_list_cmd.stdout)?;

  if exit_node_list_string.is_empty() {
    debug!("No exit nodes found");
    return Ok(vec!["No exit nodes found!".to_string()]);
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
  Command::new("tailscale")
    .args(["set", &format!("--exit-node={exit_node}")])
    .output()
    .await?;

  Ok(())
}

pub async fn switch_accounts(acct_name: &str) -> Result<bool, AppError> {
  let cmd = Command::new("tailscale")
    .args(["switch", acct_name])
    .output()
    .await?;

  let success = String::from_utf8(cmd.stdout)?;
  Ok(success.to_lowercase().contains("success"))
}

pub async fn get_acct_list() -> Result<Vec<String>, AppError> {
  let accts = Command::new("tailscale")
    .args(["switch", "--list"])
    .output()
    .await?;

  let accts_str = String::from_utf8_lossy(&accts.stdout).to_string();

  let tailnets: Vec<String> = accts_str
    .lines()
    .filter(|line| !line.to_lowercase().starts_with("id"))
    .map(std::string::ToString::to_string)
    .collect();

  let ret_accts: Vec<String> = tailnets
    .iter()
    .filter_map(|acct| acct.split_whitespace().nth(1).map(std::string::ToString::to_string))
    .collect();

  Ok(ret_accts)
}

pub async fn get_current_acct() -> Result<String, AppError> {
  let cmd = Command::new("tailscale")
    .args(["status", "--json"])
    .output()
    .await?;

  let output = String::from_utf8_lossy(&cmd.stdout).to_string();

  let acct = output
    .lines()
    .filter(|line| line.trim().starts_with("\"Name\""))
    .find_map(|line| {
      line
        .split_whitespace()
        .last()
        .map(|s| s.replace(['"', ','], ""))
    })
    .unwrap_or_default();

  Ok(acct)
}
