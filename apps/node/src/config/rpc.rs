use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct RpcConfig {
    /// Address to listen of incoming connections
    pub address: SocketAddr,

    /// Maximum number of items per list request
    #[serde(default = "default_max_items_per_request")]
    pub max_items_per_request: usize,
}

fn default_max_items_per_request() -> usize {
    50
}
