use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use serde::{Deserialize, Serialize};

#[derive(
  Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, CosmicConfigEntry,
)]
#[version = 2]
pub struct TailscaleConfig {
  #[serde(default)]
  pub exit_node_idx: usize,
  #[serde(default)]
  pub allow_lan: bool,
}
