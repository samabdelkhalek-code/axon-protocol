//! axond configuration — loaded from environment variables and CLI flags.

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(
    name    = "axond",
    about   = "AXON Protocol daemon — DHT node, request router, escrow monitor",
    version = env!("CARGO_PKG_VERSION"),
)]
pub struct Config {
    /// Listening address for the P2P transport layer.
    /// Format: /ip4/<addr>/tcp/<port>
    #[arg(long, env = "AXON_LISTEN_ADDR", default_value = "/ip4/0.0.0.0/tcp/7777")]
    pub listen_addr: String,

    /// SUI RPC endpoint for on-chain escrow verification and settlement.
    #[arg(
        long,
        env = "AXON_SUI_RPC",
        default_value = "https://fullnode.devnet.sui.io:443"
    )]
    pub sui_rpc: String,

    /// Hex-encoded AXON protocol treasury address on SUI.
    #[arg(long, env = "AXON_TREASURY_ADDRESS", default_value = "0x0")]
    pub treasury_address: String,

    /// Path to the agent's Ed25519 signing key (PEM or hex file).
    /// Generated on first run if absent.
    #[arg(long, env = "AXON_KEY_FILE", default_value = ".axon/identity.key")]
    pub key_file: String,

    /// Path to the sled database for session and manifest storage.
    #[arg(long, env = "AXON_DB_PATH", default_value = ".axon/db")]
    pub db_path: String,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long, env = "AXON_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    /// Bootstrap peer multiaddresses (comma-separated).
    /// Leave empty to operate as a bootstrap node.
    #[arg(long, env = "AXON_BOOTSTRAP_PEERS", default_value = "")]
    pub bootstrap_peers: String,

    /// Maximum concurrent inbound sessions.
    #[arg(long, env = "AXON_MAX_SESSIONS", default_value_t = 64)]
    pub max_sessions: usize,

    /// AXON genesis block hash for this network (hex-encoded 32 bytes).
    /// Devnet / testnet / mainnet each have distinct values.
    #[arg(
        long,
        env = "AXON_GENESIS_HASH",
        default_value = "0000000000000000000000000000000000000000000000000000000000000001"
    )]
    pub genesis_hash: String,
}

impl Config {
    pub fn genesis_hash_bytes(&self) -> [u8; 32] {
        let bytes = hex::decode(&self.genesis_hash)
            .expect("AXON_GENESIS_HASH must be valid hex");
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes[..32]);
        arr
    }

    pub fn bootstrap_peer_list(&self) -> Vec<String> {
        if self.bootstrap_peers.is_empty() {
            vec![]
        } else {
            self.bootstrap_peers.split(',').map(str::to_owned).collect()
        }
    }
}
