//! axon CLI — developer tool for registering agents and inspecting the network.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name    = "axon",
    about   = "AXON Protocol CLI",
    version = env!("CARGO_PKG_VERSION"),
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Register this agent with the AXON network.
    Register {
        /// Natural-language capability description.
        #[arg(short, long)]
        capability: String,
        /// Base price per compute unit (picoSUI).
        #[arg(short, long, default_value_t = 1_000)]
        price: u64,
        /// Performance bond stake (picoSUI).
        #[arg(short, long, default_value_t = 50_000_000)]
        stake: u64,
    },
    /// Query the DHT for agents matching a capability description.
    Query {
        /// Capability description to search for.
        #[arg(short, long)]
        capability: String,
        /// Number of results to return.
        #[arg(short, long, default_value_t = 5)]
        top_k: usize,
    },
    /// Show this node's current identity.
    Identity,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Register { capability, price, stake } => {
            use axon_sdk::{register, AXON_DEVNET_GENESIS};
            println!("Registering agent…");
            let result = register(&capability, price, stake, &AXON_DEVNET_GENESIS).await?;
            println!("✓ Registered");
            println!("  agent_id : {}", hex::encode(result.agent_id));
            println!("  cid      : {}", result.manifest_cid);
            println!("  price    : {} picoSUI/CU", price);
        }
        Commands::Query { capability, top_k } => {
            use axon_sdk::discover;
            println!("Searching AXON network for: '{capability}'…\n");
            
            let result = discover(&capability, top_k as u32).await?;
            
            if result.agents.is_empty() {
                println!("No agents found matching that description.");
            } else {
                for (i, agent) in result.agents.iter().enumerate() {
                    println!("[{}] Agent: {}", i + 1, hex::encode(&agent.manifest.agent_id[..8]));
                    println!("    Similarity: {:.2}%", agent.similarity * 100.0);
                    println!("    Price:      {} picoSUI/CU", agent.manifest.base_price_per_cu);
                    println!("    Tags:       {:?}", agent.manifest.capability_tags);
                    println!();
                }
            }
        }
        Commands::Identity => {
            println!("TODO: load identity from .axon/identity.key");
        }
    }

    Ok(())
}
