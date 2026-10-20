import test from 'node:test';
import assert from 'node:assert/strict';
import { executionSeconds, earlyTranslationLabel } from '../src/job-progress.ts';

test('execution time freezes when paused or failed and excludes stopped and queue intervals', () => {
  const stopped = { elapsed_seconds: 30, active_since_unix: null };
  assert.equal(executionSeconds('failed', stopped, 3600), 30);
  assert.equal(executionSeconds('queued', stopped, 4000), 30);
  assert.equal(executionSeconds('transcribing', { elapsed_seconds: 30, active_since_unix: 4000 }, 4010), 40);
  assert.equal(executionSeconds('translating', { elapsed_seconds: 40, active_since_unix: 4010 }, 4030), 60);
  assert.equal(executionSeconds('done', { elapsed_seconds: 60, active_since_unix: null }, 9000), 60);
  assert.equal(executionSeconds('failed', { elapsed_seconds: 30, active_since_unix: 100 }, 9000), 30);
  assert.equal(executionSeconds('done', null, 9000), null);
});

test('remaining translation is a running phase, not an error from an older prefix attempt', () => {
  assert.equal(earlyTranslationLabel('translating', null), ' · 识别完成，正在完成翻译');
  assert.equal(earlyTranslationLabel('translating', 'old failure'), ' · 识别完成，正在完成翻译');
  assert.match(earlyTranslationLabel('transcribing', 'empty response'), /已停止，识别继续/);
  assert.equal(earlyTranslationLabel('failed', 'empty response'), ' · 翻译待继续');
});
