//! The pane as the device's operator: arora's questions wait here until the
//! person answers them in the pane.
//!
//! A question is asked on the device's thread and answered on the app's:
//! each one is queued with the channel its answer goes back on, the pane
//! draws what is queued, and an answer sent resolves the question's future.
//! An access request is ruled on within a bounded time, as arora's
//! [`UnattendedOperator`](arora::operator::UnattendedOperator) rules: past
//! its grace it is allowed, and it leaves the pane.
//!
//! In a build with Studio the pane is also the device's enroller
//! (`arora::studio::Enroller`): the offer to enroll the device waits, for as
//! long as it takes, until the person presses Enroll; its name and model
//! family are asked as questions; the codes show in the pane, which reads
//! them from the Studio bridge's status.

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
    /// The offer to enroll the device with Studio, while it waits for the
    /// person: the pane's Enroll accepts it.
    #[cfg_attr(not(feature = "studio"), allow(dead_code))]
    offer: Option<oneshot::Sender<bool>>,
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
        if asks
            .offer
            .as_ref()
            .is_some_and(oneshot::Sender::is_canceled)
        {
            asks.offer = None;
        }
        asks
    }

    /// Whether the device is offered enrollment with Studio, waiting for the
    /// person to accept it ([`enroll`](Self::enroll)).
    #[cfg(feature = "studio")]
    pub(super) fn offered(&self) -> bool {
        self.asks().offer.is_some()
    }

    /// Accept the offer to enroll the device with Studio, if one waits.
    #[cfg(feature = "studio")]
    pub(super) fn enroll(&self) {
        if let Some(offer) = self.asks().offer.take() {
            let _ = offer.send(true);
        }
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

    #[cfg(feature = "studio")]
    fn enroller(self: Arc<Self>) -> Arc<dyn arora::studio::Enroller> {
        self
    }
}

#[cfg(feature = "studio")]
#[async_trait::async_trait]
impl arora::studio::Enroller for PaneOperator {
    /// Waits for the person to press Enroll. The pane does not open on the
    /// offer: a device nobody enrolls shows its face, and is enrolled from
    /// the pane once someone opens it.
    async fn offer_enrollment(&self) -> bool {
        let (offer, accepted) = oneshot::channel();
        self.asks().offer = Some(offer);
        accepted.await.unwrap_or(false)
    }

    async fn ask_name(&self) -> Option<String> {
        self.ask_text(
            "This device's name in Semio Studio (blank for a generated one)",
            false,
        )
        .await
    }

    async fn ask_model_family(&self) -> Option<String> {
        self.ask_text("This device's model family", false).await
    }

    /// The pane shows the code from the Studio bridge's status; a code
    /// opens the pane.
    fn show_code(&self, code: Option<&arora::studio::EnrollmentCode>) -> bool {
        if code.is_some() {
            self.asks().raised = true;
        }
        true
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

    /// The offer to enroll waits until the person presses Enroll, and leaves
    /// the pane once the device no longer waits on it.
    #[cfg(feature = "studio")]
    #[tokio::test]
    async fn the_offer_to_enroll_waits_for_enroll() {
        use arora::studio::Enroller;

        let operator = PaneOperator::default();
        let offering = tokio::spawn({
            let operator = operator.clone();
            async move { operator.offer_enrollment().await }
        });
        while !operator.offered() {
            tokio::task::yield_now().await;
        }
        assert!(!operator.asks().raised, "the offer does not open the pane");
        operator.enroll();
        assert!(offering.await.unwrap(), "Enroll accepts the offer");
        assert!(!operator.offered());

        let abandoned = tokio::spawn({
            let operator = operator.clone();
            async move { operator.offer_enrollment().await }
        });
        while !operator.offered() {
            tokio::task::yield_now().await;
        }
        abandoned.abort();
        let _ = abandoned.await;
        assert!(
            !operator.offered(),
            "an offer nobody waits on leaves the pane"
        );
    }
}
