//! The pane as the device's operator: arora's questions wait here until the
//! person answers them in the pane.
//!
//! A question is asked on the device's thread and answered on the app's:
//! each one is queued with the channel its answer goes back on, the pane
//! draws what is queued, and an answer sent resolves the question's future.
//! An access request is ruled on within a bounded time, as arora's
//! [`UnattendedOperator`](arora::operator::UnattendedOperator) rules: past
//! its grace it is allowed, and it leaves the pane.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use arora::operator::{AccessRequestSummary, AccessRuling, Operator, DEFAULT_ACCESS_GRACE};
use arora_bridge::AccessDecision;
use futures::channel::oneshot;

/// The pane's side of the device's questions; clones share them.
#[derive(Clone)]
pub struct PaneOperator {
    asks: Arc<Mutex<Asks>>,
    grace: Duration,
}

/// The questions waiting for the person.
#[derive(Default)]
pub(super) struct Asks {
    pub(super) texts: VecDeque<TextAsk>,
    pub(super) access: VecDeque<AccessAsk>,
    /// Set by a question as it arrives; the pane opens itself on it.
    pub(super) raised: bool,
}

/// A free-text question.
pub(super) struct TextAsk {
    pub(super) label: String,
    /// When set, the person cannot skip it.
    pub(super) required: bool,
    /// What the person has typed so far.
    pub(super) draft: String,
    answer: oneshot::Sender<Option<String>>,
}

/// An access request, ruled on by `until` or allowed then.
pub(super) struct AccessAsk {
    pub(super) request: AccessRequestSummary,
    pub(super) until: Instant,
    ruling: oneshot::Sender<AccessRuling>,
}

impl TextAsk {
    /// Answer with what was typed, or skip with `None`.
    pub(super) fn answer(self, answer: Option<String>) {
        let _ = self.answer.send(answer);
    }
}

impl AccessAsk {
    pub(super) fn rule(self, decision: AccessDecision, remember: bool) {
        let _ = self.ruling.send(AccessRuling { decision, remember });
    }
}

impl Default for PaneOperator {
    fn default() -> Self {
        Self::with_grace(DEFAULT_ACCESS_GRACE)
    }
}

impl PaneOperator {
    /// An operator that allows an access request left unanswered for
    /// `grace`.
    pub fn with_grace(grace: Duration) -> Self {
        Self {
            asks: Arc::default(),
            grace,
        }
    }

    /// The questions waiting, with the ones no longer awaited (an access
    /// request past its grace) dropped.
    pub(super) fn asks(&self) -> MutexGuard<'_, Asks> {
        let mut asks = self
            .asks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        asks.texts.retain(|ask| !ask.answer.is_canceled());
        asks.access.retain(|ask| !ask.ruling.is_canceled());
        asks
    }
}

#[async_trait::async_trait]
impl Operator for PaneOperator {
    async fn ask_text(&self, label: &str, required: bool) -> Option<String> {
        let (answer, answered) = oneshot::channel();
        {
            let mut asks = self.asks();
            asks.texts.push_back(TextAsk {
                label: label.to_string(),
                required,
                draft: String::new(),
                answer,
            });
            asks.raised = true;
        }
        answered.await.ok().flatten()
    }

    async fn decide_access(&self, request: &AccessRequestSummary) -> AccessRuling {
        let (ruling, ruled) = oneshot::channel();
        {
            let mut asks = self.asks();
            asks.access.push_back(AccessAsk {
                request: request.clone(),
                until: Instant::now() + self.grace,
                ruling,
            });
            asks.raised = true;
        }
        match tokio::time::timeout(self.grace, ruled).await {
            Ok(Ok(ruling)) => ruling,
            // Unanswered in time, or the pane gone: allowed, as unattended.
            _ => {
                log::info!(
                    "{} — allowed after {} s unanswered",
                    request.describe(),
                    self.grace.as_secs()
                );
                AccessRuling {
                    decision: AccessDecision::Allowed,
                    remember: false,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AccessRequestSummary {
        AccessRequestSummary {
            client_id: "client-1".into(),
            user_id: Some("user-1".into()),
            permission: "join the session".into(),
        }
    }

    #[tokio::test]
    async fn a_question_waits_for_the_pane_s_answer() {
        let operator = PaneOperator::default();
        let asking = tokio::spawn({
            let operator = operator.clone();
            async move { operator.ask_text("Device name", false).await }
        });
        let ask = loop {
            if let Some(ask) = operator.asks().texts.pop_front() {
                break ask;
            }
            tokio::task::yield_now().await;
        };
        assert_eq!(ask.label, "Device name");
        assert!(!ask.required);
        assert!(operator.asks().raised, "the question raises the pane");
        ask.answer(Some("quori".into()));
        assert_eq!(asking.await.unwrap().as_deref(), Some("quori"));
    }

    #[tokio::test]
    async fn an_access_request_takes_the_pane_s_ruling() {
        let operator = PaneOperator::default();
        let deciding = tokio::spawn({
            let operator = operator.clone();
            async move { operator.decide_access(&request()).await }
        });
        let ask = loop {
            if let Some(ask) = operator.asks().access.pop_front() {
                break ask;
            }
            tokio::task::yield_now().await;
        };
        assert_eq!(ask.request.subject(), "user-1");
        ask.rule(AccessDecision::Rejected, true);
        assert_eq!(
            deciding.await.unwrap(),
            AccessRuling {
                decision: AccessDecision::Rejected,
                remember: true
            }
        );
    }

    #[tokio::test]
    async fn an_access_request_left_unanswered_is_allowed_and_leaves_the_pane() {
        let operator = PaneOperator::with_grace(Duration::from_millis(50));
        let ruling = operator.decide_access(&request()).await;
        assert_eq!(
            ruling,
            AccessRuling {
                decision: AccessDecision::Allowed,
                remember: false
            }
        );
        assert!(operator.asks().access.is_empty());
    }
}
