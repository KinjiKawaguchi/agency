//! The calls the node makes, timed (docs/protocol.md, "Metrics"): to another node, to a team's runtime, to a
//! push service, and to GitHub. Each port is wrapped in a decorator that times one call and adds it to a
//! `call` series, so that a slow runtime or a slow peer shows up as itself and not as a slow route.
//!
//! A name is a fixed phrase from the lists below, never an id, a path, an address, or a message; the one
//! thing that varies is the `peer` of a call to another node, which is the host of its origin and nothing
//! more (no port, no user information, no path). What a call answered is never kept, only how long it took
//! and how it ended: `ok`, `error`, `timeout`, or, for a peer, the class of its HTTP status.
use super::{Recorder, SeriesKey};
use crate::github::{Gh, GhError};
use agency_core::config::HostedTeam;
use agency_core::ports::{DefinitionAccess, Notifier, PeerError, Peers, TeamRuntime, ViewRead};
use agency_core::push::{Notice, PushError};
use agency_core::read_auth::ReadAuth;
use agency_core::web_push::WebSubscription;
use agency_proto::about::About;
use agency_proto::activity::Activity;
use agency_proto::client::{PushToken, VapidKey};
use agency_proto::effort::Effort;
use agency_proto::evaluation::Evaluation;
use agency_proto::metrics::SeriesKind;
use agency_proto::org::{SignedOrg, Versions};
use agency_proto::participant_metrics::ParticipantMetrics;
use agency_proto::root_change::RootAnswer;
use agency_proto::runtime::RuntimeError;
use agency_proto::tickets::Tickets;
use agency_proto::{
    DeviceChange, Envelope, JoinRequest, RuntimeDecision, Signed, TeamView, Timestamp, TokenUsage,
};
use async_trait::async_trait;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A call that failed within this share (a twentieth) of its time limit of the limit counts as having run out
/// of time: the limit is where the other side stops waiting, and a clock reads a little short of it.
const TIMEOUT_MARGIN_DIVISOR: u32 = 20;

/// Runs `call` and adds it to the series `name` (and `peer`), with the status `status_of` gives it from the
/// answer and the time it took, or to none when it gives `None`.
async fn measure<T, E>(
    recorder: &Recorder,
    name: &str,
    peer: Option<String>,
    call: impl Future<Output = Result<T, E>>,
    status_of: impl FnOnce(&Result<T, E>, Duration) -> Option<&'static str>,
) -> Result<T, E> {
    let started = Instant::now();
    let answer = call.await;
    let elapsed = started.elapsed();
    if let Some(status) = status_of(&answer, elapsed) {
        let key = SeriesKey::new(SeriesKind::Call, name, status);
        recorder.record(
            match peer {
                Some(peer) => key.with_peer(peer),
                None => key,
            },
            elapsed,
        );
    }
    answer
}

/// A failure after the whole time a call is given (to within a twentieth of it) is a time-out; any other is an
/// error.
fn failure(elapsed: Duration, timeout: Duration) -> &'static str {
    match elapsed + timeout / TIMEOUT_MARGIN_DIVISOR >= timeout {
        true => "timeout",
        false => "error",
    }
}

/// The host of a node's origin, and nothing else of it: no scheme, user information, port, or path.
fn host_of(origin: &str) -> Option<String> {
    reqwest::Url::parse(origin)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
}

// ── Other nodes ──

/// `Peers`, timed: the series `peer <method>` with the host of the node called as its `peer`.
pub struct TimedPeers<P> {
    inner: P,
    recorder: Recorder,
    timeout: Duration,
}

impl<P> TimedPeers<P> {
    /// `timeout` is the time `inner` gives a call; a call that fails after it is a time-out.
    pub fn new(inner: P, recorder: Recorder, timeout: Duration) -> Self {
        Self {
            inner,
            recorder,
            timeout,
        }
    }

    async fn call<T>(
        &self,
        name: &str,
        node: &str,
        call: impl Future<Output = Result<T, PeerError>>,
    ) -> Result<T, PeerError> {
        let timeout = self.timeout;
        measure(
            &self.recorder,
            name,
            host_of(node),
            call,
            |answer, elapsed| {
                Some(match answer {
                    Ok(_) => "2xx",
                    Err(PeerError::Status { status, .. }) => match status / 100 {
                        2 => "2xx",
                        3 => "3xx",
                        4 => "4xx",
                        5 => "5xx",
                        _ => "error",
                    },
                    Err(PeerError::Unreachable(_)) => failure(elapsed, timeout),
                })
            },
        )
        .await
    }
}

#[async_trait]
impl<P: Peers> Peers for TimedPeers<P> {
    async fn get_view(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
        have: Option<&str>,
    ) -> Result<ViewRead, PeerError> {
        self.call(
            "peer get_view",
            node,
            self.inner.get_view(node, path, auth, have),
        )
        .await
    }

    async fn get_usage(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<TokenUsage, PeerError> {
        self.call(
            "peer get_usage",
            node,
            self.inner.get_usage(node, path, auth),
        )
        .await
    }

    async fn get_activity(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<Activity, PeerError> {
        let read = self.inner.get_activity(node, path, auth);
        self.call("peer get_activity", node, read).await
    }

    async fn get_evaluation(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<Evaluation, PeerError> {
        let read = self.inner.get_evaluation(node, path, auth);
        self.call("peer get_evaluation", node, read).await
    }

    async fn get_effort(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<Effort, PeerError> {
        self.call(
            "peer get_effort",
            node,
            self.inner.get_effort(node, path, auth),
        )
        .await
    }

    async fn get_about(&self, node: &str, path: &str, auth: &ReadAuth) -> Result<About, PeerError> {
        self.call(
            "peer get_about",
            node,
            self.inner.get_about(node, path, auth),
        )
        .await
    }

    async fn get_definition(
        &self,
        node: &str,
        path: &str,
        access: &DefinitionAccess,
    ) -> Result<SignedOrg, PeerError> {
        let read = self.inner.get_definition(node, path, access);
        self.call("peer get_definition", node, read).await
    }

    async fn get_versions(
        &self,
        node: &str,
        path: &str,
        auth: &ReadAuth,
    ) -> Result<Versions, PeerError> {
        let read = self.inner.get_versions(node, path, auth);
        self.call("peer get_versions", node, read).await
    }

    async fn post_envelope(
        &self,
        node: &str,
        path: &str,
        envelope: &Signed<Envelope>,
    ) -> Result<(), PeerError> {
        let send = self.inner.post_envelope(node, path, envelope);
        self.call("peer post_envelope", node, send).await
    }

    async fn post_definition(
        &self,
        node: &str,
        path: &str,
        doc: &SignedOrg,
    ) -> Result<(), PeerError> {
        let send = self.inner.post_definition(node, path, doc);
        self.call("peer post_definition", node, send).await
    }

    async fn post_device_change(
        &self,
        node: &str,
        path: &str,
        change: &Signed<DeviceChange>,
    ) -> Result<RootAnswer, PeerError> {
        let send = self.inner.post_device_change(node, path, change);
        self.call("peer post_device_change", node, send).await
    }

    async fn post_join(
        &self,
        node: &str,
        path: &str,
        request: &Signed<JoinRequest>,
    ) -> Result<RootAnswer, PeerError> {
        let send = self.inner.post_join(node, path, request);
        self.call("peer post_join", node, send).await
    }

    async fn get_join(&self, node: &str, path: &str) -> Result<RootAnswer, PeerError> {
        self.call("peer get_join", node, self.inner.get_join(node, path))
            .await
    }
}

// ── Team runtimes ──

/// `TeamRuntime`, timed: the series `runtime <op>`, for every op but `decide`, which the command runtime times
/// itself. A runtime that does not offer an op answers 404 without running anything, and that is no call to time.
pub struct TimedRuntime {
    inner: Arc<dyn TeamRuntime>,
    recorder: Recorder,
    timeout: Duration,
}

impl TimedRuntime {
    /// `timeout` is the time `inner` gives a call; a call that fails after it is a time-out.
    pub fn new(inner: Arc<dyn TeamRuntime>, recorder: Recorder, timeout: Duration) -> Self {
        Self {
            inner,
            recorder,
            timeout,
        }
    }

    async fn call<T>(
        &self,
        name: &str,
        call: impl Future<Output = Result<T, RuntimeError>>,
    ) -> Result<T, RuntimeError> {
        let timeout = self.timeout;
        measure(
            &self.recorder,
            name,
            None,
            call,
            |answer, elapsed| match answer {
                Ok(_) => Some("ok"),
                Err(error) if error.status == 404 => None,
                Err(_) => Some(failure(elapsed, timeout)),
            },
        )
        .await
    }
}

#[async_trait]
impl TeamRuntime for TimedRuntime {
    async fn view(&self, team: &HostedTeam) -> Result<TeamView, RuntimeError> {
        self.call("runtime view", self.inner.view(team)).await
    }

    async fn decide(
        &self,
        team: &HostedTeam,
        decision: RuntimeDecision,
    ) -> Result<(), RuntimeError> {
        // Not timed here: the command runtime times each delivery of a decision itself (`recording`), as the series
        // `runtime decide`, and a second measurement would count every delivery twice.
        self.inner.decide(team, decision).await
    }

    async fn token_usage(&self, team: &HostedTeam, days: u32) -> Result<TokenUsage, RuntimeError> {
        self.call("runtime usage", self.inner.token_usage(team, days))
            .await
    }

    async fn participant_metrics(
        &self,
        team: &HostedTeam,
        days: u32,
        utc_offset_minutes: i32,
    ) -> Result<ParticipantMetrics, RuntimeError> {
        let read = self
            .inner
            .participant_metrics(team, days, utc_offset_minutes);
        self.call("runtime participantMetrics", read).await
    }

    async fn activity(
        &self,
        team: &HostedTeam,
        since: &Timestamp,
        limit: u32,
    ) -> Result<Activity, RuntimeError> {
        self.call("runtime activity", self.inner.activity(team, since, limit))
            .await
    }

    async fn tickets(&self, team: &HostedTeam) -> Result<Tickets, RuntimeError> {
        self.call("runtime tickets", self.inner.tickets(team)).await
    }
}

// ── Push services ──

/// Which of a notifier's two ways to a device a `TimedNotifier` times.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Way {
    /// To an iPhone through APNs: `apns send` and `apns desk_changed`.
    Apns,
    /// To a browser through its push service: `webpush send` and `webpush send_many`.
    WebPush,
}

/// `Notifier`, timed on one of its ways (the other way is passed on as it is, so that a notifier made of two,
/// APNs and Web Push, is timed once on each). A token or a subscription that is gone is the service's answer,
/// `4xx`; it did not fail. A batch of subscriptions is one call: `error` when any of them failed. Wrap only a
/// notifier that sends: one that sends nothing has nothing to time.
pub struct TimedNotifier<N> {
    inner: N,
    recorder: Recorder,
    way: Way,
}

impl<N> TimedNotifier<N> {
    /// Times the sends to an iPhone.
    pub fn apns(inner: N, recorder: Recorder) -> Self {
        Self {
            inner,
            recorder,
            way: Way::Apns,
        }
    }

    /// Times the sends to a browser.
    pub fn web_push(inner: N, recorder: Recorder) -> Self {
        Self {
            inner,
            recorder,
            way: Way::WebPush,
        }
    }

    async fn call(
        &self,
        name: &str,
        call: impl Future<Output = Result<(), PushError>>,
    ) -> Result<(), PushError> {
        measure(&self.recorder, name, None, call, |answer, _| {
            Some(match answer {
                Ok(()) => "ok",
                Err(PushError::Gone) => "4xx",
                Err(PushError::Failed(_)) => "error",
            })
        })
        .await
    }
}

#[async_trait]
impl<N: Notifier> Notifier for TimedNotifier<N> {
    async fn send(&self, token: &PushToken, notice: &Notice) -> Result<(), PushError> {
        let send = self.inner.send(token, notice);
        match self.way {
            Way::Apns => self.call("apns send", send).await,
            Way::WebPush => send.await,
        }
    }

    fn web_push_key(&self) -> Option<VapidKey> {
        self.inner.web_push_key()
    }

    async fn send_desk_changed(&self, token: &PushToken) -> Result<(), PushError> {
        let send = self.inner.send_desk_changed(token);
        match self.way {
            Way::Apns => self.call("apns desk_changed", send).await,
            Way::WebPush => send.await,
        }
    }

    fn web_push_check_keys(&self, p256dh: &str, auth: &str) -> Result<(), String> {
        self.inner.web_push_check_keys(p256dh, auth)
    }

    async fn send_web_many(
        &self,
        subscriptions: &[WebSubscription],
        notice: &Notice,
    ) -> Vec<Result<(), PushError>> {
        let started = Instant::now();
        let answers = self.inner.send_web_many(subscriptions, notice).await;
        if self.way == Way::WebPush && !subscriptions.is_empty() {
            let status = match answers.iter().all(Result::is_ok) {
                true => "ok",
                false => "error",
            };
            let key = SeriesKey::new(SeriesKind::Call, "webpush send_many", status);
            self.recorder.record(key, started.elapsed());
        }
        answers
    }

    async fn send_web(
        &self,
        subscription: &WebSubscription,
        notice: &Notice,
    ) -> Result<(), PushError> {
        let send = self.inner.send_web(subscription, notice);
        match self.way {
            Way::Apns => send.await,
            Way::WebPush => self.call("webpush send", send).await,
        }
    }
}

// ── GitHub ──

/// `Gh`, timed: the series `github graphql` and `github api` (a read of the REST API), one for each time the
/// command runs. A read the cache answers never gets here.
pub struct TimedGh<G> {
    inner: G,
    recorder: Recorder,
}

impl<G> TimedGh<G> {
    pub fn new(inner: G, recorder: Recorder) -> Self {
        Self { inner, recorder }
    }
}

#[async_trait]
impl<G: Gh> Gh for TimedGh<G> {
    async fn run(&self, args: &[String]) -> Result<String, GhError> {
        let name = match args.iter().any(|arg| arg == "graphql") {
            true => "github graphql",
            false => "github api",
        };
        measure(
            &self.recorder,
            name,
            None,
            self.inner.run(args),
            |answer, _| {
                Some(match answer {
                    Ok(_) => "ok",
                    Err(GhError::TimedOut) => "timeout",
                    Err(_) => "error",
                })
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests;
