//! Coalesce independent semantic requests without changing their cache identity.
use super::translation_provider::TranslationSemanticGroup;
use super::translation_use_case::validate_translation_results;
use super::{
    TranslationFuture, TranslationProvider, TranslationProviderStatus, TranslationRequest,
    TranslationResponse, TranslationUsage,
};
use anyhow::{Result, anyhow};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::oneshot;

const MAX_TARGETS: usize = 12;
const MAX_TARGET_CHARS: usize = 1800;
const MAX_INPUT_BYTES: usize = 32_000;
type Pending = (
    TranslationRequest,
    oneshot::Sender<Result<TranslationResponse>>,
);

#[derive(Debug)]
pub struct BatchingProvider {
    inner: Arc<dyn TranslationProvider>,
    pending: Arc<Mutex<Vec<Pending>>>,
}
impl BatchingProvider {
    pub fn new(inner: Arc<dyn TranslationProvider>) -> Self {
        Self {
            inner,
            pending: Arc::new(Mutex::new(Vec::new())),
        }
    }
}
impl TranslationProvider for BatchingProvider {
    fn status(&self) -> TranslationProviderStatus {
        self.inner.status()
    }
    fn group_concurrency(&self) -> usize {
        MAX_TARGETS * super::stage_scheduler::CLOUD_TRANSLATION_CONCURRENCY
    }
    fn execution_snapshot(&self) -> Option<Arc<dyn TranslationProvider>> {
        // Scheduled provider rebuilds batching around its frozen selection.
        self.inner.execution_snapshot()
    }

    fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
        Box::pin(async move {
            // Empty capability probes do not enter the batch contract.
            if request.targets.is_empty() {
                return self.inner.translate(request).await;
            }
            let (sender, receiver) = oneshot::channel();
            let launch = {
                let mut pending = self.pending.lock().unwrap();
                let launch = pending.is_empty();
                pending.push((request, sender));
                launch
            };
            if launch {
                let queue = self.pending.clone();
                let provider = self.inner.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(8)).await;
                    let pending = std::mem::take(&mut *queue.lock().unwrap());
                    for batch in pack(pending) {
                        let provider = provider.clone();
                        tokio::spawn(send_batch(provider, batch));
                    }
                });
            }
            receiver
                .await
                .map_err(|_| anyhow!("translation batch worker stopped"))?
        })
    }
}

// Repeated IDs from another task, or a different source revision, never mix.
fn compatible(requests: &[Pending], next: &TranslationRequest) -> bool {
    let Some((first, _)) = requests.first() else {
        return true;
    };
    if first.options != next.options || first.style_instruction != next.style_instruction {
        return false;
    }
    let targets = requests
        .iter()
        .flat_map(|(r, _)| &r.targets)
        .collect::<Vec<_>>();
    if targets.len() + next.targets.len() > MAX_TARGETS
        || targets
            .iter()
            .map(|t| t.source_text.chars().count())
            .sum::<usize>()
            + next
                .targets
                .iter()
                .map(|t| t.source_text.chars().count())
                .sum::<usize>()
            > MAX_TARGET_CHARS
        || requests
            .iter()
            .map(|(r, _)| serde_json::to_vec(r).unwrap().len())
            .sum::<usize>()
            + serde_json::to_vec(next).unwrap().len()
            > MAX_INPUT_BYTES
    {
        return false;
    }
    let ids = targets
        .iter()
        .map(|t| t.segment_id.as_str())
        .collect::<HashSet<_>>();
    if next
        .targets
        .iter()
        .any(|t| ids.contains(t.segment_id.as_str()))
    {
        return false;
    }
    let mut texts = HashMap::new();
    for r in requests.iter().map(|(r, _)| r).chain(std::iter::once(next)) {
        for (id, text) in r
            .before_context
            .iter()
            .chain(&r.after_context)
            .map(|c| (&c.segment_id, &c.source_text))
            .chain(r.targets.iter().map(|t| (&t.segment_id, &t.source_text)))
        {
            if texts.insert(id, text).is_some_and(|old| old != text) {
                return false;
            }
        }
    }
    true
}
fn pack(pending: Vec<Pending>) -> Vec<Vec<Pending>> {
    let mut batches = Vec::new();
    let mut current = Vec::new();
    for item in pending.into_iter().filter(|(_, s)| !s.is_closed()) {
        if !compatible(&current, &item.0) {
            batches.push(std::mem::take(&mut current));
        }
        current.push(item);
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}
fn combined(batch: &[Pending]) -> TranslationRequest {
    let mut request = batch[0].0.clone();
    if batch.len() == 1 {
        return request;
    }
    request.targets = batch.iter().flat_map(|(r, _)| r.targets.clone()).collect();
    let mut seen = request
        .targets
        .iter()
        .map(|t| t.segment_id.clone())
        .collect::<HashSet<_>>();
    request.before_context = batch
        .iter()
        .flat_map(|(r, _)| r.before_context.iter().chain(&r.after_context))
        .filter(|c| seen.insert(c.segment_id.clone()))
        .cloned()
        .collect();
    request.after_context.clear();
    request.semantic_groups = batch
        .iter()
        .map(|(r, _)| TranslationSemanticGroup {
            before_context_ids: r
                .before_context
                .iter()
                .map(|c| c.segment_id.clone())
                .collect(),
            target_ids: r.targets.iter().map(|t| t.segment_id.clone()).collect(),
            after_context_ids: r
                .after_context
                .iter()
                .map(|c| c.segment_id.clone())
                .collect(),
        })
        .collect();
    request
}
async fn send_batch(provider: Arc<dyn TranslationProvider>, batch: Vec<Pending>) {
    let request = combined(&batch);
    let ids = request
        .targets
        .iter()
        .map(|t| t.segment_id.clone())
        .collect::<Vec<_>>();
    let call = provider.translate(request);
    // Abort the HTTP future and release its stage permit when every caller left.
    let cancelled = async {
        loop {
            if batch.iter().all(|(_, s)| s.is_closed()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let outcome = tokio::select! {
        result = call => result.and_then(|r| { validate_translation_results(&provider.status().name, &ids, &r.translations)?; Ok(r) }),
        _ = cancelled => return,
    };
    let mut usage_recorded = false;
    for (request, sender) in batch {
        let result = match &outcome {
            Ok(response) => {
                let ids = request
                    .targets
                    .iter()
                    .map(|t| &t.segment_id)
                    .collect::<HashSet<_>>();
                let usage = if !usage_recorded && !sender.is_closed() {
                    usage_recorded = true;
                    response.usage.clone()
                } else {
                    TranslationUsage::default()
                };
                Ok(TranslationResponse {
                    translations: response
                        .translations
                        .iter()
                        .filter(|t| ids.contains(&t.segment_id))
                        .cloned()
                        .collect(),
                    model: response.model.clone(),
                    usage,
                })
            }
            Err(error) => Err(anyhow!("translation request batch failed: {error:#}")),
        };
        let _ = sender.send(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        MutableTranslationProvider, TranslationContextSegment, TranslationOptions,
        TranslationResult, TranslationTargetSegment,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Default)]
    struct Echo {
        calls: Mutex<Vec<TranslationRequest>>,
        active: AtomicUsize,
    }
    impl TranslationProvider for Echo {
        fn status(&self) -> TranslationProviderStatus {
            TranslationProviderStatus {
                id: "deepseek".into(),
                name: "test".into(),
                configured: true,
                model: None,
                endpoint_kind: "test".into(),
                configuration_hint: None,
            }
        }
        fn translate<'a>(&'a self, request: TranslationRequest) -> TranslationFuture<'a> {
            Box::pin(async move {
                self.calls.lock().unwrap().push(request.clone());
                self.active.fetch_add(1, Ordering::SeqCst);
                tokio::task::yield_now().await;
                self.active.fetch_sub(1, Ordering::SeqCst);
                Ok(TranslationResponse {
                    translations: request
                        .targets
                        .iter()
                        .rev()
                        .map(|t| TranslationResult {
                            segment_id: t.segment_id.clone(),
                            translated_text: format!("译:{}", t.source_text),
                        })
                        .collect(),
                    model: None,
                    usage: TranslationUsage {
                        input_tokens: Some(10),
                        output_tokens: Some(20),
                    },
                })
            })
        }
    }
    fn request(i: usize) -> TranslationRequest {
        TranslationRequest {
            semantic_groups: vec![],
            options: TranslationOptions::default(),
            before_context: vec![TranslationContextSegment {
                segment_id: "context".into(),
                source_text: "shared context".into(),
            }],
            targets: vec![TranslationTargetSegment {
                segment_id: format!("s{i}"),
                source_text: format!("Sentence {i}."),
            }],
            after_context: vec![],
            style_instruction: None,
        }
    }
    #[tokio::test]
    async fn batches_short_groups_and_routes_out_of_order_results_and_usage_once() {
        let inner = Arc::new(Echo::default());
        let mutable = MutableTranslationProvider::new(Arc::new(
            super::super::UnconfiguredTranslationProvider,
        ));
        let scheduler = super::super::stage_scheduler::StageScheduler::new(2);
        let provider = scheduler.translation_provider(Arc::new(mutable.clone()));
        mutable.replace(inner.clone());
        let frozen = provider.execution_snapshot().unwrap();
        mutable.replace(Arc::new(super::super::UnconfiguredTranslationProvider));
        assert_eq!(frozen.group_concurrency(), 48);
        let mut tasks = tokio::task::JoinSet::new();
        for i in 0..48 {
            let provider = frozen.clone();
            tasks.spawn(async move {
                let r = provider.translate(request(i)).await.unwrap();
                assert_eq!(r.translations.len(), 1);
                assert_eq!(r.translations[0].segment_id, format!("s{i}"));
                r.usage.input_tokens.unwrap_or_default()
            });
        }
        let mut tokens = 0;
        while let Some(result) = tasks.join_next().await {
            tokens += result.unwrap();
        }
        assert_eq!(tokens, 40);
        let calls = inner.calls.lock().unwrap();
        assert_eq!(calls.len(), 4);
        for call in calls.iter() {
            assert_eq!(call.targets.len(), 12);
            assert_eq!(call.before_context.len(), 1);
            assert_eq!(call.semantic_groups.len(), 12);
            assert!(
                call.semantic_groups
                    .iter()
                    .all(|g| g.before_context_ids == ["context"] && g.target_ids.len() == 1)
            );
            assert!(
                call.style_instruction.is_none(),
                "keep configured adapter style"
            );
        }
    }
    #[test]
    fn packing_is_bounded_and_does_not_mix_revisions_options_or_duplicate_ids() {
        let (s, _r) = oneshot::channel();
        let first = vec![(request(0), s)];
        assert!(!compatible(&first, &request(0)));
        let mut changed = request(1);
        changed.before_context[0].source_text = "edited".into();
        assert!(!compatible(&first, &changed));
        changed = request(1);
        changed.style_instruction = Some("another style".into());
        assert!(!compatible(&first, &changed));
        changed = request(1);
        changed.targets[0].source_text = "x".repeat(MAX_TARGET_CHARS);
        assert!(!compatible(&first, &changed));
        let (s, r) = oneshot::channel();
        drop(r);
        assert!(pack(vec![(request(0), s)]).is_empty());
    }
    #[tokio::test]
    async fn cancelling_all_callers_drops_the_inflight_request_and_releases_the_slot() {
        #[derive(Debug)]
        struct Block {
            started: tokio::sync::Notify,
            dropped: Arc<AtomicUsize>,
        }
        struct Guard(Arc<AtomicUsize>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        impl TranslationProvider for Block {
            fn status(&self) -> TranslationProviderStatus {
                Echo::default().status()
            }
            fn translate<'a>(&'a self, _: TranslationRequest) -> TranslationFuture<'a> {
                Box::pin(async move {
                    let _guard = Guard(self.dropped.clone());
                    self.started.notify_one();
                    std::future::pending().await
                })
            }
        }
        let dropped = Arc::new(AtomicUsize::new(0));
        let inner = Arc::new(Block {
            started: tokio::sync::Notify::new(),
            dropped: dropped.clone(),
        });
        let provider = Arc::new(BatchingProvider::new(inner.clone()));
        let task = tokio::spawn(async move { provider.translate(request(0)).await });
        tokio::time::timeout(Duration::from_secs(2), inner.started.notified())
            .await
            .unwrap();
        task.abort();
        let _ = task.await;
        tokio::time::timeout(Duration::from_secs(2), async {
            while dropped.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
}
