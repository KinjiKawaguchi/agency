//! The node's own measurements (docs/protocol.md, "Metrics"): each request the node answers and each call it
//! makes is added to a histogram, by hour, and the hours are appended to files in `<data dir>/metrics`, so
//! that they outlive the process and only the owner can read them. A name is a route template or a fixed
//! phrase, never a path with ids in it, and nothing holds a body, a token, an address, or a query string.
//! The cost of a measurement is one addition to a small map.

mod store;

use agency_core::ports::Clock;
use agency_proto::metrics::{
    bucket_of, group_of, is_error_status, is_in_node_read, percentile_ms, valid_name, valid_status,
    Budget, HourSeries, Metrics, Series, SeriesKind, BUCKETS, CLIENTS, INVALID,
    IN_NODE_READ_P95_BUDGET_MS, OVERFLOW,
};
use agency_proto::{clock, Timestamp};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

/// How many days of measurements are kept.
pub const KEPT_DAYS: u32 = 30;
/// The most distinct series an hour may hold. A name is a template or a fixed phrase, so there are far
/// fewer; this bounds the memory if one is ever not. A series past it is recorded under the one series
/// `overflow` of its kind, status, and client, which have no limit of their own because there are few.
pub const MOST_SERIES_IN_AN_HOUR: usize = 500;
/// The most distinct series the totals since the node started hold, bounded the same way.
pub const MOST_SERIES_IN_THE_TOTALS: usize = 500;

/// Which series a measurement belongs to.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SeriesKey {
    pub kind: SeriesKind,
    pub name: String,
    pub status: String,
    pub peer: Option<String>,
    pub client: Option<String>,
}

impl SeriesKey {
    pub fn new(kind: SeriesKind, name: impl Into<String>, status: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            status: status.into(),
            peer: None,
            client: None,
        }
    }

    /// The same series for a call to a peer (the host of its origin).
    pub fn with_peer(self, peer: impl Into<String>) -> Self {
        Self {
            peer: Some(peer.into()),
            ..self
        }
    }

    /// The same series for what a client (`web` or `ios`) reported.
    pub fn with_client(self, client: impl Into<String>) -> Self {
        Self {
            client: Some(client.into()),
            ..self
        }
    }
}

/// The numbers of one series in one hour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Counts {
    pub count: u64,
    pub errors: u64,
    pub sum_ms: u64,
    pub buckets: [u64; BUCKETS],
}

impl Default for Counts {
    fn default() -> Self {
        Self {
            count: 0,
            errors: 0,
            sum_ms: 0,
            buckets: [0; BUCKETS],
        }
    }
}

impl State {
    /// The series of the totals a measurement of `key` goes in: its own, or `overflow` when the totals hold
    /// as many series as they may.
    fn total_series_for(&self, key: &SeriesKey) -> SeriesKey {
        if self.totals.contains_key(key) || self.totals.len() < MOST_SERIES_IN_THE_TOTALS {
            return key.clone();
        }
        SeriesKey {
            name: OVERFLOW.to_owned(),
            peer: None,
            ..key.clone()
        }
    }

    /// The series a measurement of `key` goes in for this hour: its own, or `overflow` when the hour has
    /// no room for another. A series that is new is counted against the hour.
    fn series_for(&mut self, hour: Hour, key: SeriesKey) -> SeriesKey {
        if self.pending.contains_key(&(hour, key.clone())) {
            return key;
        }
        let in_this_hour = self.series_in_hour.get(&hour).copied().unwrap_or(0);
        let key = if in_this_hour >= MOST_SERIES_IN_AN_HOUR && key.name != OVERFLOW {
            SeriesKey {
                name: OVERFLOW.to_owned(),
                peer: None,
                ..key
            }
        } else {
            key
        };
        if !self.pending.contains_key(&(hour, key.clone())) {
            *self.series_in_hour.entry(hour).or_default() += 1;
        }
        key
    }
}

impl Counts {
    fn add(&mut self, millis: u64, is_error: bool) {
        self.count += 1;
        self.errors += u64::from(is_error);
        self.sum_ms = self.sum_ms.saturating_add(millis);
        self.buckets[bucket_of(millis)] += 1;
    }

    pub(crate) fn merge(&mut self, other: &Counts) {
        self.count += other.count;
        self.errors += other.errors;
        self.sum_ms = self.sum_ms.saturating_add(other.sum_ms);
        for (mine, theirs) in self.buckets.iter_mut().zip(other.buckets) {
            *mine += theirs;
        }
    }
}

/// The start of an hour, in Unix seconds.
type Hour = i64;

#[derive(Default)]
struct State {
    /// What was recorded since the last flush, by hour.
    pending: BTreeMap<(Hour, SeriesKey), Counts>,
    /// How many series each hour of `pending` has, so that a new one can be refused past the limit.
    series_in_hour: BTreeMap<Hour, usize>,
    /// Everything since the node started, for the cumulative read.
    totals: BTreeMap<SeriesKey, Counts>,
}

/// Records measurements and reads them back. Cloning shares the state.
#[derive(Clone)]
pub struct Recorder {
    inner: Arc<Inner>,
}

struct Inner {
    dir: PathBuf,
    clock: Arc<dyn Clock>,
    state: Mutex<State>,
    /// Held for the length of a flush, so that two (the timer's and the one at a stop) do not rewrite a
    /// file at once.
    flush_lock: Mutex<()>,
}

fn hour_of(unix_seconds: i64) -> Hour {
    unix_seconds - unix_seconds.rem_euclid(3600)
}

impl Recorder {
    /// A recorder that keeps its files in `dir`, which is created when the first hour is flushed.
    pub fn new(dir: PathBuf, clock: Arc<dyn Clock>) -> Self {
        Self {
            inner: Arc::new(Inner {
                dir,
                clock,
                state: Mutex::new(State::default()),
                flush_lock: Mutex::new(()),
            }),
        }
    }

    /// Adds one measurement of a series. Nothing a requester could choose gets into a name: a name a series
    /// of its kind may not have is recorded under `invalid`, and a series past the limit of an hour under
    /// `overflow`. A status that is not one of its kind, or a client that is not `web` or `ios` for what a
    /// client reported, is a mistake of the caller: the measurement is not recorded. Whether it was an
    /// error follows from the status.
    pub fn record(&self, key: SeriesKey, elapsed: Duration) {
        let Some(key) = normalized(key) else {
            return;
        };
        let is_error = is_error_status(key.kind, &key.status);
        let millis = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        let hour = hour_of(self.inner.clock.now().unix_timestamp());
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let total_key = state.total_series_for(&key);
        state
            .totals
            .entry(total_key)
            .or_default()
            .add(millis, is_error);
        let key = state.series_for(hour, key);
        state
            .pending
            .entry((hour, key))
            .or_default()
            .add(millis, is_error);
    }

    /// Writes what was recorded since the last flush to the files, and deletes the files that are past
    /// the days kept. What cannot be written is kept for the next flush.
    pub fn flush(&self) {
        let _one_at_a_time = self
            .inner
            .flush_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let pending = {
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            state.series_in_hour.clear();
            std::mem::take(&mut state.pending)
        };
        if !pending.is_empty() {
            if let Err(unwritten) = store::append(&self.inner.dir, pending) {
                let mut state = self
                    .inner
                    .state
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                for (key, counts) in unwritten {
                    if !state.pending.contains_key(&key) {
                        *state.series_in_hour.entry(key.0).or_default() += 1;
                    }
                    state.pending.entry(key).or_default().merge(&counts);
                }
            }
        }
        store::prune(
            &self.inner.dir,
            self.inner.clock.now().unix_timestamp(),
            KEPT_DAYS,
        );
    }

    /// Everything recorded since the node started, by series: what a cumulative reader (a Prometheus
    /// scrape) wants.
    pub fn totals(&self) -> Vec<(SeriesKey, Counts)> {
        let state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        state
            .totals
            .iter()
            .map(|(key, counts)| (key.clone(), counts.clone()))
            .collect()
    }

    /// The measurements of the last `days` days (the hours of them, up to this one), from the files and
    /// from what is not flushed yet, optionally of one kind and one name.
    pub fn read(&self, days: u32, kind: Option<SeriesKind>, name: Option<&str>) -> Metrics {
        let now = self.inner.clock.now();
        let this_hour = hour_of(now.unix_timestamp());
        let first = this_hour - (i64::from(days) * 24 - 1) * 3600;
        let mut hours: BTreeMap<(Hour, SeriesKey), Counts> = store::read(&self.inner.dir, first);
        let pending = self
            .inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pending
            .clone();
        for (key, counts) in pending.iter().filter(|((hour, _), _)| *hour >= first) {
            hours.entry(key.clone()).or_default().merge(counts);
        }
        let wanted = |key: &SeriesKey| {
            kind.is_none_or(|k| key.kind == k) && name.is_none_or(|n| key.name == n)
        };
        Metrics {
            from: timestamp(first),
            until: timestamp(this_hour + 3600),
            generated_at: timestamp(now.unix_timestamp()),
            budget: Budget {
                in_node_read_p95_ms: IN_NODE_READ_P95_BUDGET_MS,
            },
            series: series_of(hours, wanted),
        }
    }
}

/// The key as a series may have it, or `None` for a measurement that is not one.
fn normalized(key: SeriesKey) -> Option<SeriesKey> {
    if !valid_status(key.kind, &key.status) {
        return None;
    }
    let client_is_right = match key.kind {
        SeriesKind::Client => key
            .client
            .as_deref()
            .is_some_and(|client| CLIENTS.contains(&client)),
        _ => key.client.is_none(),
    };
    if !client_is_right {
        return None;
    }
    let name = if valid_name(key.kind, &key.name) {
        key.name
    } else {
        INVALID.to_owned()
    };
    // The host of a peer is the node's own configuration, but it is held to the same rule as a name.
    let peer = match (key.kind, key.peer) {
        (SeriesKind::Call, Some(host)) if valid_name(SeriesKind::Call, &host) => Some(host),
        (SeriesKind::Call, Some(_)) => Some(INVALID.to_owned()),
        _ => None,
    };
    Some(SeriesKey { name, peer, ..key })
}

fn timestamp(unix_seconds: i64) -> Timestamp {
    let at = clock::OffsetDateTime::from_unix_timestamp(unix_seconds)
        .unwrap_or(clock::OffsetDateTime::UNIX_EPOCH);
    Timestamp::parse(&clock::format(at)).expect("a time in RFC 3339")
}

/// The series of `hours`, ordered by kind, name, and status, each with its hours.
fn series_of(
    hours: BTreeMap<(Hour, SeriesKey), Counts>,
    wanted: impl Fn(&SeriesKey) -> bool,
) -> Vec<Series> {
    let mut by_series: BTreeMap<SeriesKey, Vec<(Hour, Counts)>> = BTreeMap::new();
    for ((hour, key), counts) in hours {
        if wanted(&key) {
            by_series.entry(key).or_default().push((hour, counts));
        }
    }
    let mut series: Vec<Series> = by_series
        .into_iter()
        .map(|(key, hours)| {
            let mut total = Counts::default();
            for (_, counts) in &hours {
                total.merge(counts);
            }
            let percentile = |buckets: &[u64], percent| percentile_ms(buckets, percent);
            Series {
                kind: key.kind,
                name: key.name.clone(),
                status: key.status.clone(),
                peer: key.peer.clone(),
                client: key.client.clone(),
                group: group_of(key.kind, &key.name).to_owned(),
                is_in_node_read: is_in_node_read(key.kind, &key.name),
                count: total.count,
                errors: total.errors,
                sum_ms: total.sum_ms,
                buckets: total.buckets.to_vec(),
                p50_ms: percentile(&total.buckets, 50),
                p95_ms: percentile(&total.buckets, 95),
                p99_ms: percentile(&total.buckets, 99),
                by_hour: hours
                    .iter()
                    .map(|(hour, counts)| HourSeries {
                        hour: timestamp(*hour),
                        count: counts.count,
                        errors: counts.errors,
                        p95_ms: percentile(&counts.buckets, 95),
                    })
                    .collect(),
            }
        })
        .collect();
    series.sort_by(|a, b| {
        (a.kind, &a.name, &a.status, &a.peer, &a.client)
            .cmp(&(b.kind, &b.name, &b.status, &b.peer, &b.client))
    });
    series
}

#[cfg(test)]
mod tests;
