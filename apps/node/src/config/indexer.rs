use std::time::Duration;

use bitcoin::BlockHash;
use serde::Deserialize;
use yuv_indexers::IndexingParams;

pub const DEFAULT_POLLING_PERIOD: Duration = Duration::from_secs(5);

/// One day:
pub const DEFAULT_MAX_CONFIRMATION_TIME: Duration = Duration::from_secs(60 * 60 * 24);

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
}

fn default_polling_period() -> Duration {
    DEFAULT_POLLING_PERIOD
}

fn default_max_confirmation_time() -> Duration {
    DEFAULT_MAX_CONFIRMATION_TIME
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
        }
    }
}
