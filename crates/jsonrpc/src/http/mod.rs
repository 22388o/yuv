#[cfg(feature = "reqwest_http")]
pub mod reqwest_http;

const DEFAULT_URL: &str = "http://127.0.0.1";
const DEFAULT_PORT: u16 = 8332; // the default RPC port for bitcoind.
const DEFAULT_TIMEOUT_SECONDS: u64 = 15;
