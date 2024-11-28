pub(crate) mod config;
pub(crate) mod handlers;
pub(crate) mod node;
pub(crate) mod payloads;
pub(crate) mod utils;
pub(crate) mod workers;

/// Crate version of the compute node.
/// This value is attached within the published messages.
pub const DRIA_COMPUTE_NODE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use config::{DriaComputeNodeConfig, DriaNetworkType};
pub use node::DriaComputeNode;
