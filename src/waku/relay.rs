#![allow(unused)]

use std::borrow::BorrowMut;

use crate::{utils::message::Message, waku::BaseClient};
use urlencoding;

/// Client for [11/WAKU2-RELAY](https://github.com/vacp2p/rfc-index/blob/main/waku/standards/core/11/relay.md) operations.
///
/// The relay client is used to send and receive messages to Waku network. It works as follows:
///
/// 1. A node subscribes to a content topic
/// 2. Nodes that are subscribed to the same content topic can send and receive messages via the network.
/// 3. On termination, the node unsubscribes from the content topic.
///
#[derive(Debug, Clone)]
pub struct RelayClient {
    base: BaseClient,
    content_topics: Vec<String>,
}

impl RelayClient {
    pub fn new(base: BaseClient) -> Self {
        RelayClient {
            base,
            content_topics: Vec::new(),
        }
    }

    /// Send a message.
    pub async fn send_message(&self, message: Message) -> Result<(), Box<dyn std::error::Error>> {
        let message = serde_json::json!(message);
        self.base.post("relay/v1/auto/messages", message).await?;

        Ok(())
    }

    /// Get messages with a given content topic.
    ///
    /// The content topic must have been subscribed to before.
    pub async fn get_messages(
        &self,
        content_topic: &str,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        let topic = urlencoding::encode(content_topic).to_string();
        let res = self
            .base
            .get(&format!("relay/v1/auto/messages/{}", topic), None)
            .await?;

        let msgs = res.json().await?;

        Ok(msgs)
    }

    /// Subscribe to an array of content topics.
    pub async fn subscribe(
        &mut self,
        content_topics: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let res = self
            .base
            .post(
                "relay/v1/auto/subscriptions",
                serde_json::json!(content_topics),
            )
            .await?;

        // add content_topics to self.content_topics
        self.content_topics.extend(content_topics);
        Ok(())
    }

    /// Unsubscribe from an array of content topics.
    pub async fn unsubscribe(
        &mut self,
        content_topics: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.base
            .delete(
                "relay/v1/auto/subscriptions",
                serde_json::json!(content_topics),
            )
            .await?;

        // remove content_topics from self.content_topics
        self.content_topics
            .retain(|topic| !content_topics.contains(topic));
        Ok(())
    }
}
