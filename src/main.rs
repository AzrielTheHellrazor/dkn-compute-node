use dkn_compute::config::tasks::DriaComputeNodeTasks;
use dkn_compute::utils::wait_for_termination;
use dkn_compute::{config::DriaComputeNodeConfig, node::DriaComputeNode};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use dkn_compute::workers::diagnostic::*;
use dkn_compute::workers::heartbeat::*;
use dkn_compute::workers::search_python::*;
use dkn_compute::workers::synthesis::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();

    const VERSION: &str = env!("CARGO_PKG_VERSION");
    log::info!("Using Dria Compute Node v{}", VERSION);

    let tasks = DriaComputeNodeTasks::new();
    let config = DriaComputeNodeConfig::new();
    let cancellation = CancellationToken::new();
    let node = Arc::new(DriaComputeNode::new(config, cancellation.clone()));

    log::info!("Starting workers...");
    log::info!("{:?}", tasks);
    let tracker = TaskTracker::new();

    tracker.spawn(heartbeat_worker(
        node.clone(),
        "heartbeat",
        tokio::time::Duration::from_millis(1000),
    ));

    tracker.spawn(diagnostic_worker(
        node.clone(),
        tokio::time::Duration::from_secs(60),
    ));

    if tasks.synthesis {
        tracker.spawn(synthesis_worker(
            node.clone(),
            "synthesis",
            tokio::time::Duration::from_millis(1000),
        ));
    }

    if tasks.search {
        // TODO: add a feature / env var to enable/disable search_python
        // and use search_rust instead
        tracker.spawn(search_worker(
            node.clone(),
            "search_python",
            tokio::time::Duration::from_millis(1000),
        ));
    }

    // close tracker after spawning everything
    tracker.close();

    // wait for all workers
    wait_for_termination(cancellation).await?;
    log::warn!("Stopping workers");
    tracker.wait().await;

    Ok(())
}
