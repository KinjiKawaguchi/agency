//! Keeps what GitHub said for a short while, so that a screen that is read often does not spend the rate
//! limit of the machine's `gh` login. Nothing is written to disk.
use agency_core::delivery::DeliveryFacts;
use agency_core::ports::{DeliveryError, DeliverySource};
use agency_proto::clock::OffsetDateTime;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

type Key = (Vec<String>, i64);

pub struct CachedDelivery<D> {
    inner: D,
    kept_for: Duration,
    kept: Mutex<HashMap<Key, (Instant, DeliveryFacts)>>,
}

impl<D> CachedDelivery<D> {
    pub fn new(inner: D, kept_for: Duration) -> Self {
        Self {
            inner,
            kept_for,
            kept: Mutex::default(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Key, (Instant, DeliveryFacts)>> {
        self.kept
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl<D: DeliverySource> DeliverySource for CachedDelivery<D> {
    async fn facts(
        &self,
        repos: &[String],
        since: OffsetDateTime,
    ) -> Result<DeliveryFacts, DeliveryError> {
        let key = (repos.to_vec(), since.unix_timestamp());
        if let Some((at, facts)) = self.lock().get(&key) {
            if at.elapsed() < self.kept_for {
                return Ok(facts.clone());
            }
        }
        let facts = self.inner.facts(repos, since).await?;
        let mut kept = self.lock();
        kept.retain(|_, (at, _)| at.elapsed() < self.kept_for);
        kept.insert(key, (Instant::now(), facts.clone()));
        Ok(facts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryDelivery;
    use agency_proto::clock;

    fn at(raw: &str) -> OffsetDateTime {
        clock::parse(raw).unwrap()
    }

    fn cached(kept_for: Duration) -> (CachedDelivery<MemoryDelivery>, MemoryDelivery) {
        let memory = MemoryDelivery::default();
        memory.0.lock().unwrap().facts = Some(Ok(DeliveryFacts::default()));
        (CachedDelivery::new(memory.clone(), kept_for), memory)
    }

    fn asked(memory: &MemoryDelivery) -> usize {
        memory.0.lock().unwrap().asked.len()
    }

    #[tokio::test]
    async fn answers_the_same_question_from_memory_for_a_while() {
        let (delivery, memory) = cached(Duration::from_secs(60));
        let repos = vec!["acme/app".to_string()];
        let since = at("2026-11-10T00:00:00Z");
        delivery.facts(&repos, since).await.unwrap();
        delivery.facts(&repos, since).await.unwrap();
        assert_eq!(asked(&memory), 1);
    }

    #[tokio::test]
    async fn another_repository_or_period_is_another_question() {
        let (delivery, memory) = cached(Duration::from_secs(60));
        let (app, web) = (vec!["acme/app".to_string()], vec!["acme/web".to_string()]);
        delivery
            .facts(&app, at("2026-11-10T00:00:00Z"))
            .await
            .unwrap();
        delivery
            .facts(&web, at("2026-11-10T00:00:00Z"))
            .await
            .unwrap();
        delivery
            .facts(&app, at("2026-11-11T00:00:00Z"))
            .await
            .unwrap();
        assert_eq!(asked(&memory), 3);
    }

    #[tokio::test]
    async fn asks_again_when_what_it_kept_is_too_old_and_never_keeps_a_failure() {
        let (delivery, memory) = cached(Duration::from_millis(30));
        let repos = vec!["acme/app".to_string()];
        let since = at("2026-11-10T00:00:00Z");
        delivery.facts(&repos, since).await.unwrap();
        tokio::time::sleep(Duration::from_millis(60)).await;
        delivery.facts(&repos, since).await.unwrap();
        assert_eq!(asked(&memory), 2);

        memory.0.lock().unwrap().facts = Some(Err("GitHub is down".into()));
        let other = at("2026-11-12T00:00:00Z");
        assert_eq!(
            delivery.facts(&repos, other).await.unwrap_err().0,
            "GitHub is down"
        );
        memory.0.lock().unwrap().facts = Some(Ok(DeliveryFacts::default()));
        delivery.facts(&repos, other).await.unwrap();
        assert_eq!(asked(&memory), 4, "the failure was not kept");
    }
}
