use ollama_workflows::{Entry, Executor, Model, ProgramMemory, Workflow};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

use crate::node::DriaComputeNode;

#[derive(Debug, Deserialize)]
struct WorkflowPayload {
    pub(crate) workflow: Workflow,
    pub(crate) model: String,
    pub(crate) prompt: String,
}

const REQUEST_TOPIC: &str = "workflow";
const RESPONSE_TOPIC: &str = "results";

pub fn workflow_worker(
    node: Arc<DriaComputeNode>,
    sleep_amount: Duration,
) -> tokio::task::JoinHandle<()> {
    // this ID is given in the workflow itself, but within Dria we always
    // use "final_result" for this ID.
    let final_result_id = "final_result".to_string();

    tokio::spawn(async move {
        node.subscribe_topic(REQUEST_TOPIC).await;
        node.subscribe_topic(RESPONSE_TOPIC).await;

        loop {
            tokio::select! {
                _ = node.cancellation.cancelled() => break,
                _ = tokio::time::sleep(sleep_amount) => {
                    let tasks = match node.process_topic(REQUEST_TOPIC, true).await {
                        Ok(messages) => {
                            if messages.is_empty() {
                                continue;
                            }
                            node.parse_messages::<WorkflowPayload>(messages, true)
                        }
                        Err(e) => {
                            log::error!("Error processing topic {}: {}", REQUEST_TOPIC, e);
                            continue;
                        }
                    };
                    if tasks.is_empty() {
                        log::info!("No {} tasks.", REQUEST_TOPIC);
                    } else {
                        node.set_busy(true);

                        log::info!("Processing {} {} tasks.", tasks.len(), REQUEST_TOPIC);
                        for task in &tasks {
                            log::debug!("Task ID: {}", task.task_id);
                        }

                        for task in tasks {
                            // read model from the task
                            let model = Model::try_from(task.input.model.clone()).unwrap_or_else(|model| {
                                log::error!("Invalid model provided: {}, defaulting.", model);
                                Model::default()
                            });
                            log::info!("Using model {}", model);

                            // execute workflow with cancellation
                            let executor = Executor::new(model);
                            let mut memory = ProgramMemory::new();
                            let entry = Entry::String(task.input.prompt);
                            tokio::select! {
                                _ = node.cancellation.cancelled() => {
                                    log::info!("Received cancellation, quitting all tasks.");
                                    break;
                                },
                                _ = executor.execute(Some(&entry), task.input.workflow, &mut memory) => ()
                            }

                            // read final result from memory
                            let result = match memory.read(&final_result_id) {
                                Some(entry) => entry.to_string(),
                                None => {
                                    log::error!("No final result found in memory for task {}", task.task_id);
                                    continue;
                                },
                            };

                            // send result to the response
                            if let Err(e) = node.send_result(RESPONSE_TOPIC, &task.public_key, &task.task_id, result).await {
                                log::error!("Error sending task result: {}", e);
                                continue;
                            };
                        }

                        node.set_busy(false);
                    }
                }
            }
        }

        node.unsubscribe_topic_ignored(REQUEST_TOPIC).await;
        node.unsubscribe_topic_ignored(RESPONSE_TOPIC).await;
    })
}
