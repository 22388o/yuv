use std::time::Duration;

use bitcoin::BlockHash;
use serde::Deserialize;
use yuv_indexers::{BlockLoaderConfig, IndexingParams};

pub const DEFAULT_POLLING_PERIOD: Duration = Duration::from_secs(5);

/// One day:
pub const DEFAULT_MAX_CONFIRMATION_TIME: Duration = Duration::from_secs(60 * 60 * 24);

pub const DEFAULT_RESTART_INTERVAL: Duration = Duration::from_secs(5);
pub const MAX_RESTART_ATTEMPTS: u32 = 10;

#[derive(Clone, Deserialize)]
pub struct IndexerConfig {
    #[serde(default = "default_polling_period")]
    pub polling_period: Duration,

    #[serde(default)]
    pub starting_block: Option<BlockHash>,

    #[serde(default)]
    pub step_back: Option<u64>,

    #[serde(default = "default_max_confirmation_time")]
    pub max_confirmation_time: Duration,

    #[serde(default)]
    pub blockloader: BlockLoaderConfig,

    #[serde(default = "default_restart_interval")]
    pub restart_interval: Duration,

    #[serde(default = "default_max_restart_attempts")]
    pub max_restart_attempts: u32,
}

fn default_polling_period() -> Duration {
    DEFAULT_POLLING_PERIOD
}

fn default_max_confirmation_time() -> Duration {
    DEFAULT_MAX_CONFIRMATION_TIME
}

fn default_restart_interval() -> Duration {
    DEFAULT_RESTART_INTERVAL
}

fn default_max_restart_attempts() -> u32 {
    MAX_RESTART_ATTEMPTS
}

impl From<IndexerConfig> for IndexingParams {
    fn from(value: IndexerConfig) -> Self {
        let def = IndexingParams::default();

        Self {
            yuv_genesis_block_hash: value.starting_block,
            index_step_back: value.step_back.unwrap_or(def.index_step_back),
        }
    }
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            polling_period: default_polling_period(),
            starting_block: Default::default(),
            step_back: Default::default(),
            max_confirmation_time: default_max_confirmation_time(),
            blockloader: BlockLoaderConfig::default(),
            restart_interval: default_restart_interval(),
            max_restart_attempts: default_max_restart_attempts(),
        }
    }
}
