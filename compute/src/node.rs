use dkn_p2p::{
    libp2p::{
        gossipsub::{Message, MessageAcceptance, MessageId},
        PeerId,
    },
    DriaP2PClient, DriaP2PCommander, DriaP2PProtocol,
};
use eyre::{eyre, Result};
use tokio::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tokio_util::{either::Either, sync::CancellationToken};

use crate::{
    config::*,
    handlers::*,
    utils::{crypto::secret_to_keypair, AvailableNodes, DKNMessage},
    workers::workflow::{WorkflowsWorker, WorkflowsWorkerInput, WorkflowsWorkerOutput},
};

/// Number of seconds between refreshing the Kademlia DHT.
const PEER_REFRESH_INTERVAL_SECS: u64 = 30;

pub struct DriaComputeNode {
    pub config: DriaComputeNodeConfig,
    pub p2p: DriaP2PCommander,
    pub available_nodes: AvailableNodes,
    pub cancellation: CancellationToken,
    peer_last_refreshed: Instant,
    // channels
    message_rx: mpsc::Receiver<(PeerId, MessageId, Message)>,
    worklow_tx: mpsc::Sender<WorkflowsWorkerInput>,
    publish_rx: mpsc::Receiver<WorkflowsWorkerOutput>,
}

impl DriaComputeNode {
    /// Creates a new `DriaComputeNode` with the given configuration and cancellation token.
    ///
    /// Returns the node instance and p2p client together. P2p MUST be run in a separate task before this node is used at all.
    pub async fn new(
        config: DriaComputeNodeConfig,
        cancellation: CancellationToken,
    ) -> Result<(DriaComputeNode, DriaP2PClient, WorkflowsWorker)> {
        // create the keypair from secret key
        let keypair = secret_to_keypair(&config.secret_key);

        // get available nodes (bootstrap, relay, rpc) for p2p
        let mut available_nodes = AvailableNodes::new(config.network_type);
        available_nodes.populate_with_statics();
        available_nodes.populate_with_env();
        if let Err(e) = available_nodes.populate_with_api().await {
            log::error!("Error populating available nodes: {:?}", e);
        };

        // we are using the major.minor version as the P2P version
        // so that patch versions do not interfere with the protocol
        let protocol = DriaP2PProtocol::new_major_minor(config.network_type.protocol_name());
        log::info!("Using identity: {}", protocol);

        // create p2p client
        let (p2p_client, p2p_commander, message_rx) = DriaP2PClient::new(
            keypair,
            config.p2p_listen_addr.clone(),
            available_nodes.bootstrap_nodes.clone().into_iter(),
            available_nodes.relay_nodes.clone().into_iter(),
            available_nodes.rpc_addrs.clone().into_iter(),
            protocol,
        )?;

        // create workflow worker
        let (worklow_tx, workflow_rx) = mpsc::channel(256);
        let (publish_tx, publish_rx) = mpsc::channel(256);
        let workflows_worker = WorkflowsWorker::new(workflow_rx, publish_tx);

        Ok((
            DriaComputeNode {
                config,
                p2p: p2p_commander,
                cancellation,
                available_nodes,
                message_rx,
                worklow_tx,
                publish_rx,
                peer_last_refreshed: Instant::now(),
            },
            p2p_client,
            workflows_worker,
        ))
    }

    /// Subscribe to a certain task with its topic.
    pub async fn subscribe(&mut self, topic: &str) -> Result<()> {
        let ok = self.p2p.subscribe(topic).await?;
        if ok {
            log::info!("Subscribed to {}", topic);
        } else {
            log::info!("Already subscribed to {}", topic);
        }
        Ok(())
    }

    /// Unsubscribe from a certain task with its topic.
    pub async fn unsubscribe(&mut self, topic: &str) -> Result<()> {
        let ok = self.p2p.unsubscribe(topic).await?;
        if ok {
            log::info!("Unsubscribed from {}", topic);
        } else {
            log::info!("Already unsubscribed from {}", topic);
        }
        Ok(())
    }

    /// Publishes a given message to the network w.r.t the topic of it.
    ///
    /// Internally, identity is attached to the the message which is then JSON serialized to bytes
    /// and then published to the network as is.
    pub async fn publish(&mut self, mut message: DKNMessage) -> Result<()> {
        // attach protocol name to the message
        message = message.with_identity(self.p2p.protocol().name.clone());

        let message_bytes = serde_json::to_vec(&message)?;
        let message_id = self.p2p.publish(&message.topic, message_bytes).await?;
        log::info!("Published message ({}) to {}", message_id, message.topic);
        Ok(())
    }

    /// Returns the list of connected peers, `mesh` and `all`.
    #[inline(always)]
    pub async fn peers(&self) -> Result<(Vec<PeerId>, Vec<PeerId>)> {
        self.p2p.peers().await
    }

    /// Handles a GossipSub message received from the network.
    async fn handle_message(
        &mut self,
        (peer_id, message_id, message): (PeerId, &MessageId, Message),
    ) -> MessageAcceptance {
        // refresh admin rpc peer ids
        // TODO: move this to main loop with tokio select
        if self.available_nodes.can_refresh() {
            log::info!("Refreshing available nodes.");

            if let Err(e) = self.available_nodes.populate_with_api().await {
                log::error!("Error refreshing available nodes: {:?}", e);
            };

            // dial all rpc nodes for better connectivity
            for rpc_addr in self.available_nodes.rpc_addrs.iter() {
                log::debug!("Dialling RPC node: {}", rpc_addr);
                if let Err(e) = self.p2p.dial(rpc_addr.clone()).await {
                    log::warn!("Error dialling RPC node: {:?}", e);
                };
            }

            // print network info
            log::debug!("{:?}", self.p2p.network_info().await);
        }

        // check peer count
        // TODO: move this to main loop with tokio select
        if self.peer_last_refreshed.elapsed() > Duration::from_secs(PEER_REFRESH_INTERVAL_SECS) {
            match self.p2p.peer_counts().await {
                Ok((mesh, all)) => log::info!("Peer Count (mesh/all): {} / {}", mesh, all),
                Err(e) => {
                    log::error!("Error getting peer counts: {:?}", e);
                }
            }

            self.peer_last_refreshed = Instant::now();

            // TODO: add peer list as well
        }

        // handle message with respect to its topic
        let topic_str = message.topic.as_str();
        if std::matches!(
            topic_str,
            PingpongHandler::LISTEN_TOPIC | WorkflowHandler::LISTEN_TOPIC
        ) {
            // ensure that the message is from a valid source (origin)
            let Some(source_peer_id) = message.source else {
                log::warn!(
                    "Received {} message from {} without source.",
                    topic_str,
                    peer_id
                );
                return MessageAcceptance::Ignore;
            };

            // log the received message
            log::info!(
                "Received {} message ({}) from {}",
                topic_str,
                message_id,
                peer_id,
            );

            // ensure that message is from the known RPCs
            if !self.available_nodes.rpc_nodes.contains(&source_peer_id) {
                log::warn!(
                    "Received message from unauthorized source: {}",
                    source_peer_id
                );
                log::debug!("Allowed sources: {:#?}", self.available_nodes.rpc_nodes);
                return MessageAcceptance::Ignore;
            }

            // first, parse the raw gossipsub message to a prepared message
            let message = match self.parse_message_to_prepared_message(message.clone()) {
                Ok(message) => message,
                Err(e) => {
                    log::error!("Error parsing message: {:?}", e);
                    log::debug!("Message: {}", String::from_utf8_lossy(&message.data));
                    return MessageAcceptance::Ignore;
                }
            };

            // then handle the prepared message
            let handler_result = match topic_str {
                WorkflowHandler::LISTEN_TOPIC => {
                    let compute_result = WorkflowHandler::handle_compute(self, message).await;
                    match compute_result {
                        Ok(Either::Left(acceptance)) => Ok(acceptance),
                        Ok(Either::Right(workflow_message)) => {
                            if let Err(e) = self.worklow_tx.send(workflow_message).await {
                                log::error!("Error sending workflow message: {:?}", e);
                            };

                            Ok(MessageAcceptance::Accept)
                        }
                        Err(err) => Err(err),
                    }
                }
                PingpongHandler::LISTEN_TOPIC => PingpongHandler::handle_ping(self, message).await,
                _ => unreachable!(), // unreachable because of the if condition
            };

            // validate the message based on the result
            match handler_result {
                Ok(acceptance) => {
                    return acceptance;
                }
                Err(err) => {
                    log::error!("Error handling {} message: {:?}", topic_str, err);
                    return MessageAcceptance::Ignore;
                }
            }
        } else if std::matches!(
            topic_str,
            PingpongHandler::RESPONSE_TOPIC | WorkflowHandler::RESPONSE_TOPIC
        ) {
            // since we are responding to these topics, we might receive messages from other compute nodes
            // we can gracefully ignore them and propagate it to to others
            log::trace!("Ignoring message for topic: {}", topic_str);
            return MessageAcceptance::Accept;
        } else {
            // reject this message as its from a foreign topic
            log::warn!("Received message from unexpected topic: {}", topic_str);
            return MessageAcceptance::Reject;
        }
    }

    /// Runs the main loop of the compute node.
    /// This method is not expected to return until cancellation occurs.
    pub async fn run(&mut self) -> Result<()> {
        // subscribe to topics
        self.subscribe(PingpongHandler::LISTEN_TOPIC).await?;
        self.subscribe(PingpongHandler::RESPONSE_TOPIC).await?;
        self.subscribe(WorkflowHandler::LISTEN_TOPIC).await?;
        self.subscribe(WorkflowHandler::RESPONSE_TOPIC).await?;

        // main loop, listens for message events in particular
        // the underlying p2p client is expected to handle the rest within its own loop
        loop {
            tokio::select! {
                publish_msg = self.publish_rx.recv() => {
                    if let Some(result) = publish_msg {
                        WorkflowHandler::handle_publish(self, result).await?;
                    }
                },
                gossipsub_msg = self.message_rx.recv() => {
                    if let Some((peer_id, message_id, message)) = gossipsub_msg {
                        // handle the message, returning a message acceptance for the received one
                        let acceptance = self.handle_message((peer_id, &message_id, message)).await;

                        // validate the message based on the acceptance
                        // cant do anything but log if this gives an error as well
                        if let Err(e) = self.p2p.validate_message(&message_id, &peer_id, acceptance).await {
                            log::error!("Error validating message {}: {:?}", message_id, e);
                        }
                    } else {
                        log::warn!("Message channel closed.");
                        break;
                    };
                },
                _ = self.cancellation.cancelled() => break,
            }
        }

        // unsubscribe from topics
        self.unsubscribe(PingpongHandler::LISTEN_TOPIC).await?;
        self.unsubscribe(PingpongHandler::RESPONSE_TOPIC).await?;
        self.unsubscribe(WorkflowHandler::LISTEN_TOPIC).await?;
        self.unsubscribe(WorkflowHandler::RESPONSE_TOPIC).await?;

        // shutdown channels
        self.shutdown().await?;

        Ok(())
    }

    /// Shutdown channels between p2p, worker and yourself.
    pub async fn shutdown(&mut self) -> Result<()> {
        log::debug!("Sending shutdown command to p2p client.");
        self.p2p.shutdown().await?;

        log::debug!("Closing message channel.");
        self.message_rx.close();

        log::debug!("Closing publish channel.");
        self.publish_rx.close();

        Ok(())
    }
    /// Parses a given raw Gossipsub message to a prepared P2PMessage object.
    /// This prepared message includes the topic, payload, version and timestamp.
    ///
    /// This also checks the signature of the message, expecting a valid signature from admin node.
    // TODO: move this somewhere?
    pub fn parse_message_to_prepared_message(&self, message: Message) -> Result<DKNMessage> {
        // the received message is expected to use IdentHash for the topic, so we can see the name of the topic immediately.
        log::debug!("Parsing {} message.", message.topic.as_str());
        let message = DKNMessage::try_from(message)?;
        log::debug!("Parsed: {}", message);

        // check dria signature
        // NOTE: when we have many public keys, we should check the signature against all of them
        // TODO: public key here will be given dynamically
        if !message.is_signed(&self.config.admin_public_key)? {
            return Err(eyre!("Invalid signature."));
        }

        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    #[ignore = "run this manually"]
    async fn test_publish_message() -> eyre::Result<()> {
        env::set_var("RUST_LOG", "none,dkn_compute=debug,dkn_p2p=debug");
        let _ = env_logger::builder().is_test(true).try_init();

        // create node
        let cancellation = CancellationToken::new();
        let (mut node, p2p, _) =
            DriaComputeNode::new(DriaComputeNodeConfig::default(), cancellation.clone())
                .await
                .expect("should create node");

        // spawn p2p task
        let p2p_task = tokio::spawn(async move { p2p.run().await });

        // launch & wait for a while for connections
        log::info!("Waiting a bit for peer setup.");
        tokio::select! {
            _ = node.run() => (),
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(20)) => cancellation.cancel(),
        }
        log::info!("Connected Peers:\n{:#?}", node.peers().await?);

        // publish a dummy message
        let topic = "foo";
        let message = DKNMessage::new("hello from the other side", topic);
        node.subscribe(topic).await.expect("should subscribe");
        node.publish(message).await.expect("should publish");
        node.unsubscribe(topic).await.expect("should unsubscribe");

        // close everything
        log::info!("Shutting down node.");
        node.p2p.shutdown().await?;

        // wait for task handle
        p2p_task.await?;

        Ok(())
    }
}
