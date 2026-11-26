//! An answer kept for each team and served at once, then refreshed in the background (stale-while-
//! revalidate), so a read never waits for a team's runtime process unless nothing usable is in memory.
//! At most one run per team is in flight; readers that have to wait share it.
use agency_core::config::HostedTeam;
use agency_proto::runtime::RuntimeError;
use agency_proto::Id;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Team = (Id, Id);

/// One run of a team's command, made again for each refresh. It owns what it needs.
pub(crate) type Run<T> =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<T, RuntimeError>> + Send>> + Send + Sync>;

/// How many reads were answered from a young answer, from an old one (and started a refresh), and had to
/// wait for a run (nothing in memory, or too old), so cold reads can be told from warm ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadCounts {
    pub fresh: u64,
    pub stale: u64,
    pub cold: u64,
}

struct Kept<T> {
    at: Instant,
    answer: Result<T, RuntimeError>,
}

enum Look<T> {
    Fresh(Result<T, RuntimeError>),
    Stale(Result<T, RuntimeError>),
    Nothing,
}

struct Slot<T> {
    kept: Mutex<Option<Kept<T>>>,
    /// Held while a run is in flight: one run per team at a time, and a reader that must wait queues here.
    flight: Arc<tokio::sync::Mutex<()>>,
    /// Counts decisions: an answer from a run that began before one is not kept.
    epoch: AtomicU64,
}

impl<T: Clone + Send + 'static> Slot<T> {
    fn new() -> Self {
        Self {
            kept: Mutex::new(None),
            flight: Arc::new(tokio::sync::Mutex::new(())),
            epoch: AtomicU64::new(0),
        }
    }

    fn kept(&self) -> std::sync::MutexGuard<'_, Option<Kept<T>>> {
        self.kept
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn look(&self, fresh: Duration, max_stale: Duration) -> Look<T> {
        let kept = self.kept();
        let Some(one) = kept.as_ref() else {
            return Look::Nothing;
        };
        let age = one.at.elapsed();
        if age < fresh {
            Look::Fresh(one.answer.clone())
        } else if age < max_stale && one.answer.is_ok() {
            Look::Stale(one.answer.clone())
        } else {
            Look::Nothing
        }
    }

    /// Keeps `answer`, except that a failure does not replace a good answer: that one is served until it
    /// is too old.
    fn keep(&self, answer: Result<T, RuntimeError>) {
        let mut kept = self.kept();
        let has_good = kept.as_ref().is_some_and(|one| one.answer.is_ok());
        if answer.is_err() && has_good {
            return;
        }
        *kept = Some(Kept {
            at: Instant::now(),
            answer,
        });
    }

    /// Runs the command and keeps its answer, unless a decision came meanwhile and made it out of date.
    /// The caller holds `flight`.
    async fn run_and_keep(&self, run: &Run<T>) -> Result<T, RuntimeError> {
        let epoch = self.epoch.load(Ordering::SeqCst);
        let answer = run().await;
        if self.epoch.load(Ordering::SeqCst) == epoch {
            self.keep(answer.clone());
        }
        answer
    }

    /// Starts a refresh unless a run is already in flight.
    fn refresh_in_background(self: &Arc<Self>, run: Run<T>) {
        let Ok(flight) = self.flight.clone().try_lock_owned() else {
            return;
        };
        let slot = self.clone();
        tokio::spawn(async move {
            let _ = slot.run_and_keep(&run).await;
            drop(flight);
        });
    }
}

pub(crate) struct Swr<T> {
    slots: Mutex<HashMap<Team, Arc<Slot<T>>>>,
    /// An answer younger than this is served as it is.
    fresh: Duration,
    /// An answer younger than this and older than `fresh` is served at once and refreshed in the
    /// background; an older one is not served.
    max_stale: Duration,
    counts: Mutex<ReadCounts>,
}

impl<T: Clone + Send + 'static> Swr<T> {
    pub fn new(fresh: Duration, max_stale: Duration) -> Self {
        Self {
            slots: Mutex::default(),
            fresh,
            max_stale,
            counts: Mutex::default(),
        }
    }

    pub fn fresh(&self) -> Duration {
        self.fresh
    }

    fn slot(&self, team: &HostedTeam) -> Arc<Slot<T>> {
        let mut slots = self
            .slots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let key = (team.org.clone(), team.team.clone());
        slots
            .entry(key)
            .or_insert_with(|| Arc::new(Slot::new()))
            .clone()
    }

    fn count(&self, change: impl FnOnce(&mut ReadCounts)) {
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        change(&mut counts);
    }

    pub fn counts(&self) -> ReadCounts {
        *self
            .counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The answer for `team`: a young one as it is; an old one at once, with a refresh started; and, only
    /// with nothing usable in memory, one from a run that this read waits for (or joins, if one is in flight).
    pub async fn get(&self, team: &HostedTeam, run: Run<T>) -> Result<T, RuntimeError> {
        let slot = self.slot(team);
        match slot.look(self.fresh, self.max_stale) {
            Look::Fresh(answer) => {
                self.count(|counts| counts.fresh += 1);
                return answer;
            }
            Look::Stale(answer) => {
                self.count(|counts| counts.stale += 1);
                slot.refresh_in_background(run);
                return answer;
            }
            Look::Nothing => self.count(|counts| counts.cold += 1),
        }
        let _flight = slot.flight.lock().await;
        if let Look::Fresh(answer) = slot.look(self.fresh, self.max_stale) {
            return answer;
        }
        slot.run_and_keep(&run).await
    }

    /// Drops what is kept for `team` after a decision, and starts a run at once, so the read that follows
    /// finds a fresh answer or joins the run already in flight.
    pub fn invalidate_and_refresh(&self, team: &HostedTeam, run: Run<T>) {
        let slot = self.slot(team);
        slot.epoch.fetch_add(1, Ordering::SeqCst);
        *slot.kept() = None;
        let flight = slot.flight.clone();
        tokio::spawn(async move {
            let _flight = flight.lock().await;
            let _ = slot.run_and_keep(&run).await;
        });
    }
}
