//! What a running node does on its own, on three schedules that never wait for one another: re-reading what other
//! nodes hold (replicas, peers, definitions, held joins), telling the owner and guests what is new (push), and
//! delivering the outbox. A team node that does not answer slows only the first: notifications and queued
//! decisions keep their pace.
use agency_adapters::logging::Logger;
use agency_core::invites::finish_joins;
use agency_core::outbox::deliver_due;
use agency_core::push::notify_waiting;
use agency_core::ships::record_all;
use agency_core::views::refresh;
use agency_core::Node;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

/// How often each kind of work runs. The pause is counted from the end of a run, so a slow run never overlaps
/// the next one of its own kind.
#[derive(Clone, Copy, Debug)]
pub struct Schedule {
    pub replicas: Duration,
    pub push: Duration,
    pub outbox: Duration,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            replicas: Duration::from_secs(30),
            push: Duration::from_secs(30),
            outbox: Duration::from_secs(15),
        }
    }
}

/// Starts the three schedules; each runs until the process ends.
pub fn start(node: Arc<Node>, logger: Logger, schedule: Schedule) {
    every(schedule.replicas, node.clone(), logger.clone(), replicas);
    every(schedule.push, node.clone(), logger.clone(), push);
    every(schedule.outbox, node, logger, outbox);
}

fn every<F, Fut>(pause: Duration, node: Arc<Node>, logger: Logger, work: F)
where
    F: Fn(Arc<Node>, Logger) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            work(node.clone(), logger.clone()).await;
            tokio::time::sleep(pause).await;
        }
    });
}

async fn replicas(node: Arc<Node>, logger: Logger) {
    // Expired held changes are dropped, and a held join's invite freed, even when nobody looks.
    if let Err(error) = agency_core::pending_root::waiting(&node) {
        logger.warn("root", &format!("sweeping held changes failed: {error}"));
    }
    if let Err(error) = finish_joins(&node).await {
        logger.warn("joins", &format!("asking for held joins failed: {error}"));
    }
    if let Err(error) = refresh(&node).await {
        logger.warn(
            "replicas",
            &format!("re-reading the replicas failed: {error}"),
        );
    }
    if let Err(error) = record_all(&node).await {
        logger.warn(
            "ships",
            &format!("remembering the days epics shipped failed: {error}"),
        );
    }
}

async fn push(node: Arc<Node>, logger: Logger) {
    match notify_waiting(&node).await {
        Ok(pass) => {
            for recovery in pass.recoveries {
                logger.warn("recovery", &recovery);
                eprintln!("agency: {recovery}");
            }
            for failure in pass.failures {
                logger.warn(
                    "push",
                    &format!("a push notification was not sent: {failure}"),
                );
            }
        }
        Err(error) => logger.warn(
            "requests",
            &format!("checking for new requests failed: {error}"),
        ),
    }
}

async fn outbox(node: Arc<Node>, logger: Logger) {
    if let Err(error) = deliver_due(&node).await {
        logger.warn("outbox", &format!("retrying the outbox failed: {error}"));
    }
}
