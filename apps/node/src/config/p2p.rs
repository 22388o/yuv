use bitcoin::Network;
use eyre::Context;
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, ToSocketAddrs};
use std::str::FromStr;
use yuv_p2p::client;

#[derive(Serialize, Deserialize, Clone)]
pub struct P2pConfig {
    /// Address to listen to incoming connections
    pub address: String,
    /// P2p network type
    #[serde(default = "default_network", deserialize_with = "deserialize_network")]
    pub network: Network,
    /// Maximum amount of inbound connections
    pub max_inbound_connections: usize,
    /// Maximum amount of outbound connections
    pub max_outbound_connections: usize,
    /// List of nodes to connect to firstly.
    #[serde(default)]
    pub bootnodes: Vec<String>,
}

fn deserialize_network<'de, D>(deserializer: D) -> Result<Network, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    Network::from_str(&s).map_err(serde::de::Error::custom)
}

fn default_network() -> Network {
    Network::Bitcoin
}

impl TryFrom<P2pConfig> for client::P2PConfig {
    type Error = eyre::Error;

    fn try_from(value: P2pConfig) -> Result<client::P2PConfig, Self::Error> {
        let bootnodes: Vec<SocketAddr> = value
            .bootnodes
            .iter()
            .map(|x| {
                x.to_socket_addrs()
                    .wrap_err("Failed to resolve bootnode address")
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();

        let address = value
            .address
            .to_socket_addrs()
            .wrap_err("Failed to resolve address")?
            .next()
            .ok_or_else(|| eyre::eyre!("No address found in listen address"))?;

        Ok(client::P2PConfig::new(
            value.network,
            address,
            bootnodes,
            value.max_inbound_connections,
            value.max_outbound_connections,
        ))
    }
}