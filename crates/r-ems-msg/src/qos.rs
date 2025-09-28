//! ---
//! ems_section: "02-messaging-ipc-data-model"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Message schema helpers and protocol codecs."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::Message;

/// Delivery guarantees supported by the messaging subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryGuarantee {
    /// Deliver the message at most once; no retry tracking.
    AtMostOnce,
    /// Deliver the message at least once; retries until acknowledgement or `max_retries` reached.
    AtLeastOnce {
        /// Maximum number of retry attempts before declaring the message dropped.
        max_retries: u8,
        /// Minimum waiting period between retry attempts.
        retry_interval: Duration,
    },
    /// Attempt exactly-once semantics by retrying and deduplicating via sequence numbers.
    ExactlyOnce {
        /// Maximum number of retry attempts before the caller must reconcile duplicates.
        max_retries: u8,
        /// Minimum waiting period between retry attempts.
        retry_interval: Duration,
    },
}

impl DeliveryGuarantee {
    fn retry_policy(&self) -> Option<(u8, Duration)> {
        match self {
            DeliveryGuarantee::AtLeastOnce {
                max_retries,
                retry_interval,
            }
            | DeliveryGuarantee::ExactlyOnce {
                max_retries,
                retry_interval,
            } => Some((*max_retries, *retry_interval)),
            DeliveryGuarantee::AtMostOnce => None,
        }
    }
}

/// Tracks sequence numbers and pending acknowledgements for a delivery guarantee.
#[derive(Clone)]
pub struct QoSManager {
    guarantee: DeliveryGuarantee,
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    next_sequence: u64,
    pending: HashMap<u64, PendingMessage>,
}

struct PendingMessage {
    message: Message,
    attempts: u8,
    last_attempt: Instant,
}

impl QoSManager {
    /// Create a new QoS manager with the chosen delivery guarantee.
    pub fn new(guarantee: DeliveryGuarantee) -> Self {
        Self {
            guarantee,
            state: Arc::new(Mutex::new(State::default())),
        }
    }

    /// Assign a sequence number to a message and track it for retries if required.
    pub fn register(&self, message: Message) -> (u64, Message) {
        let mut guard = self.state.lock().expect("qos state poisoned");
        guard.next_sequence = guard.next_sequence.wrapping_add(1);
        let sequence = guard.next_sequence;

        if let Some((_max_retries, _interval)) = self.guarantee.retry_policy() {
            guard.pending.insert(
                sequence,
                PendingMessage {
                    message: message.clone(),
                    attempts: 0,
                    last_attempt: Instant::now(),
                },
            );
        }

        (sequence, message)
    }

    /// Mark a sequence as acknowledged, removing it from retry tracking.
    pub fn acknowledge(&self, sequence: u64) {
        if let Some((_max_retries, _)) = self.guarantee.retry_policy() {
            let mut guard = self.state.lock().expect("qos state poisoned");
            guard.pending.remove(&sequence);
        }
    }

    /// Determine which messages should be retried based on the retry policy.
    pub fn pending_for_retry(&self) -> Vec<(u64, Message)> {
        let Some((max_retries, interval)) = self.guarantee.retry_policy() else {
            return Vec::new();
        };

        let mut guard = self.state.lock().expect("qos state poisoned");
        let now = Instant::now();
        let mut to_retry = Vec::new();
        let mut to_remove = Vec::new();

        for (sequence, pending) in guard.pending.iter_mut() {
            if pending.attempts >= max_retries {
                to_remove.push(*sequence);
                continue;
            }
            if now.duration_since(pending.last_attempt) >= interval {
                pending.attempts += 1;
                pending.last_attempt = now;
                to_retry.push((*sequence, pending.message.clone()));
            }
        }

        for sequence in to_remove {
            guard.pending.remove(&sequence);
        }

        to_retry
    }

    /// Forcefully drop all tracked pending messages.
    pub fn drain_pending(&self) -> Vec<(u64, Message)> {
        let Some((_max_retries, _)) = self.guarantee.retry_policy() else {
            return Vec::new();
        };
        let mut guard = self.state.lock().expect("qos state poisoned");
        guard
            .pending
            .drain()
            .map(|(seq, pending)| (seq, pending.message))
            .collect()
    }

    /// Access the current sequence counter (useful for testing).
    pub fn current_sequence(&self) -> u64 {
        let guard = self.state.lock().expect("qos state poisoned");
        guard.next_sequence
    }
}

impl Default for QoSManager {
    fn default() -> Self {
        Self::new(DeliveryGuarantee::AtLeastOnce {
            max_retries: 3,
            retry_interval: Duration::from_millis(100),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Message, MessagePayload, TelemetryFrame, TelemetryValues};

    fn telemetry_message() -> Message {
        let mut values = TelemetryValues::new();
        values.insert("voltage".into(), 480.0);
        let frame = TelemetryFrame::new("grid-a", "controller-a", values);
        Message::new(MessagePayload::Telemetry(frame))
    }

    #[test]
    fn at_most_once_does_not_retry() {
        let qos = QoSManager::new(DeliveryGuarantee::AtMostOnce);
        qos.register(telemetry_message());
        assert!(qos.pending_for_retry().is_empty());
    }

    #[test]
    fn at_least_once_retries_until_ack_or_limit() {
        let qos = QoSManager::new(DeliveryGuarantee::AtLeastOnce {
            max_retries: 2,
            retry_interval: Duration::from_millis(1),
        });
        let (sequence, _) = qos.register(telemetry_message());
        std::thread::sleep(Duration::from_millis(2));
        let retry_batch = qos.pending_for_retry();
        assert_eq!(retry_batch.len(), 1);
        assert_eq!(retry_batch[0].0, sequence);
        qos.acknowledge(sequence);
        assert!(qos.pending_for_retry().is_empty());
    }

    #[test]
    fn retries_stop_after_max_attempts() {
        let qos = QoSManager::new(DeliveryGuarantee::AtLeastOnce {
            max_retries: 1,
            retry_interval: Duration::from_millis(1),
        });
        qos.register(telemetry_message());
        std::thread::sleep(Duration::from_millis(2));
        assert_eq!(qos.pending_for_retry().len(), 1);
        std::thread::sleep(Duration::from_millis(2));
        assert!(
            qos.pending_for_retry().is_empty(),
            "should stop retrying after max attempts"
        );
    }
}
