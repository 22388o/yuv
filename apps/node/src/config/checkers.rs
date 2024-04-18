use serde::Deserialize;

pub const DEFAULT_POOL_SIZE: usize = 10;

#[derive(Deserialize)]
pub struct CheckersConfig {
    /// Number of checkers in working pool
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
}

fn default_pool_size() -> usize {
    DEFAULT_POOL_SIZE
}

impl Default for CheckersConfig {
    fn default() -> Self {
        Self {
            pool_size: default_pool_size(),
        }
    }
}
