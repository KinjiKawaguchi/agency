//! How often one client address may try something that is guarded by a secret (the app's sign-in code, a claim
//! link's code): a few tries a minute for each address, and a table that cannot be filled to lock anyone out.
use agency_proto::clock::{Duration, OffsetDateTime};
use std::collections::HashMap;

/// How many client addresses are remembered; beyond that, the one that has been quiet the longest is forgotten.
pub const MOST_CLIENTS: usize = 1024;
/// The window a try counts in.
const WINDOW_SECONDS: i64 = 60;

/// When each client address last tried, within the minute.
#[derive(Default)]
pub struct Attempts(HashMap<String, Vec<OffsetDateTime>>);

impl Attempts {
    /// Counts a try of `client` at `now`: `Ok` when it has had fewer than `per_minute` in the last minute, else the
    /// seconds until one more is allowed. A full table forgets the address that has been quiet the longest, so
    /// that filling it can never lock a real person out: a forgotten address starts with its full allowance
    /// again, and what each try needs (a secret code, tried once) is what protects it, not the count.
    pub fn try_once(
        &mut self,
        now: OffsetDateTime,
        client: &str,
        per_minute: usize,
    ) -> Result<(), i64> {
        let window = Duration::seconds(WINDOW_SECONDS);
        self.0.retain(|_, times| {
            times.retain(|at| now - *at < window);
            !times.is_empty()
        });
        if !self.0.contains_key(client) && self.0.len() >= MOST_CLIENTS {
            let quietest = self
                .0
                .iter()
                .min_by_key(|(_, times)| times.iter().max().copied())
                .map(|(address, _)| address.clone());
            if let Some(address) = quietest {
                self.0.remove(&address);
            }
        }
        let times = self.0.entry(client.to_owned()).or_default();
        if times.len() >= per_minute {
            let oldest = times.iter().min().copied().unwrap_or(now);
            return Err((window - (now - oldest)).whole_seconds().max(1));
        }
        times.push(now);
        Ok(())
    }
}
