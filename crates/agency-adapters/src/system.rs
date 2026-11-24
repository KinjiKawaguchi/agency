//! The real clock, the operating system's randomness, and an async write lock.
use agency_core::ports::{Clock, Random, WriteLock};
use agency_proto::clock::OffsetDateTime;
use async_trait::async_trait;

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

pub struct OsRandom;

impl Random for OsRandom {
    fn bytes16(&self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut bytes);
        bytes
    }
}

#[derive(Default)]
pub struct TokioWriteLock(tokio::sync::Mutex<()>);

#[async_trait]
impl WriteLock for TokioWriteLock {
    async fn lock(&self) -> Box<dyn Send + '_> {
        Box::new(self.0.lock().await)
    }
}
