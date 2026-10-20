//! Shared limits belong to execution stages, rather than whole media jobs.
use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::application::{
    TranslationFuture, TranslationProvider, TranslationProviderStatus, TranslationRequest,
};

pub const DEFAULT_ASR_CONCURRENCY: usize = 2;
pub const CLOUD_TRANSLATION_CONCURRENCY: usize = 4;

#[derive(Debug)]
pub struct StageScheduler {
    asr: Arc<Semaphore>,
    asr_capacity: u32,
    cloud: Arc<Semaphore>,
}

impl StageScheduler {
    pub fn new(asr_capacity: usize) -> Arc<Self> {
        assert!(asr_capacity > 0 && asr_capacity <= u32::MAX as usize);
        Arc::new(Self {
            asr: Arc::new(Semaphore::new(asr_capacity)),
            asr_capacity: asr_capacity as u32,
            cloud: Arc::new(Semaphore::new(CLOUD_TRANSLATION_CONCURRENCY)),
        })
    }

    pub async fn acquire_asr(&self) -> OwnedSemaphorePermit {
        self.asr
            .clone()
            .acquire_owned()
            .await
            .expect("ASR stage stays open")
    }

    pub fn translation_provider(
        self: &Arc<Self>,
        provider: Arc<dyn TranslationProvider>,
    ) -> Arc<dyn TranslationProvider> {
        Arc::new(ScheduledTranslationProvider {
            scheduler: self.clone(),
            provider,
        })
    }
}

#[derive(Debug)]
struct ScheduledTranslationProvider {
    scheduler: Arc<StageScheduler>,
    provider: Arc<dyn TranslationProvider>,
}

impl TranslationProvider for ScheduledTranslationProvider {
    fn status(&self) -> TranslationProviderStatus {
        self.provider.status()
    }

    fn execution_snapshot(&self) -> Option<Arc<dyn TranslationProvider>> {
        Some(
            self.scheduler.translation_provider(
                self.provider
                    .execution_snapshot()
                    .unwrap_or_else(|| self.provider.clone()),
            ),
        )
    }

    fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
        Box::pin(async move {
            // Local translation owns the device exclusively; cloud requests
            // overlap ASR and share one global request budget across jobs.
            let _permit = if self.provider.status().id == "hy-mt2-local" {
                self.scheduler
                    .asr
                    .clone()
                    .acquire_many_owned(self.scheduler.asr_capacity)
                    .await
                    .expect("local stage stays open")
            } else {
                self.scheduler
                    .cloud
                    .clone()
                    .acquire_owned()
                    .await
                    .expect("cloud stage stays open")
            };
            self.provider.translate(request).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{TranslationOptions, TranslationResponse, TranslationUsage};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct Provider {
        id: &'static str,
        started: tokio::sync::mpsc::UnboundedSender<()>,
        release: Arc<Semaphore>,
        peak: Arc<AtomicUsize>,
        active: AtomicUsize,
    }
    impl TranslationProvider for Provider {
        fn status(&self) -> TranslationProviderStatus {
            TranslationProviderStatus {
                id: self.id.into(),
                name: self.id.into(),
                configured: true,
                model: None,
                endpoint_kind: "test".into(),
                configuration_hint: None,
            }
        }
        fn translate<'a>(&'a self, _: TranslationRequest) -> TranslationFuture<'a> {
            Box::pin(async move {
                let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
                self.peak.fetch_max(active, Ordering::SeqCst);
                self.started.send(()).unwrap();
                self.release.acquire().await.unwrap().forget();
                self.active.fetch_sub(1, Ordering::SeqCst);
                Ok(TranslationResponse {
                    translations: vec![],
                    model: None,
                    usage: TranslationUsage::default(),
                })
            })
        }
    }
    fn request() -> TranslationRequest {
        TranslationRequest {
            options: TranslationOptions::default(),
            before_context: vec![],
            targets: vec![],
            after_context: vec![],
            style_instruction: None,
        }
    }

    #[tokio::test]
    async fn two_asr_slots_and_four_cloud_requests_are_independent_and_cancel_safe() {
        let scheduler = StageScheduler::new(2);
        let first = scheduler.acquire_asr().await;
        let second = scheduler.acquire_asr().await;
        assert!(scheduler.asr.try_acquire().is_err());
        let (started, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let release = Arc::new(Semaphore::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let provider = scheduler.translation_provider(Arc::new(Provider {
            id: "deepseek",
            started,
            release: release.clone(),
            peak: peak.clone(),
            active: AtomicUsize::new(0),
        }));
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..8 {
            let provider = provider.execution_snapshot().unwrap();
            tasks.spawn(async move {
                provider.translate(request()).await.unwrap();
            });
        }
        for _ in 0..4 {
            tokio::time::timeout(std::time::Duration::from_secs(2), receiver.recv())
                .await
                .unwrap()
                .unwrap();
        }
        assert_eq!(scheduler.cloud.available_permits(), 0);
        drop(first);
        let asr = scheduler.acquire_asr().await;
        release.add_permits(8);
        while let Some(result) = tasks.join_next().await {
            result.unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 4);
        drop((asr, second));
        assert_eq!(scheduler.asr.available_permits(), 2);
        assert_eq!(scheduler.cloud.available_permits(), 4);
    }

    #[tokio::test]
    async fn local_translation_waits_for_asr_and_cancel_releases_device() {
        let scheduler = StageScheduler::new(2);
        let asr = scheduler.acquire_asr().await;
        let (started, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let provider = scheduler.translation_provider(Arc::new(Provider {
            id: "hy-mt2-local",
            started,
            release: Arc::new(Semaphore::new(0)),
            peak: Arc::new(AtomicUsize::new(0)),
            active: AtomicUsize::new(0),
        }));
        let task = tokio::spawn(async move { provider.translate(request()).await });
        tokio::task::yield_now().await;
        assert!(receiver.try_recv().is_err());
        drop(asr);
        tokio::time::timeout(std::time::Duration::from_secs(2), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(scheduler.asr.available_permits(), 0);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(scheduler.asr.available_permits(), 2);
    }
}
