import test from 'node:test';
import assert from 'node:assert/strict';
import { translationUsageLabel } from '../src/subtitle-translation-usage.ts';

test('local reuse is distinguished from cloud cache tokens and an entire-task bill', () => {
  assert.equal(translationUsageLabel({input_tokens:0,output_tokens:0,plan_json:JSON.stringify({execution_usage:{local_cache_hit:true,cached_input_tokens:0,uncached_input_tokens:0}})}), '本地缓存复用（本次无新增 API 消耗）');
  assert.equal(translationUsageLabel({input_tokens:120,output_tokens:18,plan_json:JSON.stringify({execution_usage:{cached_input_tokens:96,uncached_input_tokens:24}})}), '本次 Token 120 入 / 18 出 · 云端输入缓存 96 命中 / 24 未命中');
});

test('legacy, absent and invalid optional counts do not become invented zeros', () => {
  assert.equal(translationUsageLabel({input_tokens:120,output_tokens:null}), 'Token 120 入 / ? 出');
  assert.equal(translationUsageLabel({input_tokens:null,output_tokens:null}), '');
  assert.equal(translationUsageLabel({input_tokens:12,output_tokens:3,plan_json:'invalid'}), 'Token 12 入 / 3 出');
  assert.equal(translationUsageLabel({input_tokens:null,output_tokens:null,plan_json:JSON.stringify({execution_usage:{cached_input_tokens:-1,uncached_input_tokens:'12'}})}), '');
});
