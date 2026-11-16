"""Immutable-prefix contracts, including target/draft parity after tail changes."""
from types import SimpleNamespace
import unittest
from unittest.mock import patch

try:
    import mlx.core as mx
    import mlx.nn as nn
    import numpy as np
    from mlx_qwen3_asr.config import TextDecoderConfig
    from mlx_qwen3_asr.decoder import TextDecoder, KVCache
    from qwen_speculative import Target, Draft, DecoderPrefix, decode, transcribe
    from qwen_prefix_cache import CachedTranscriber
    HAS_MLX = True
except ImportError:
    HAS_MLX = False


@unittest.skipUnless(HAS_MLX, 'requires the isolated MLX ASR environment')
class PrefixCacheTests(unittest.TestCase):
    def runtime(self):
        mx.random.seed(31)
        config = TextDecoderConfig(vocab_size=32, hidden_size=128, intermediate_size=256,
                                  num_hidden_layers=3, num_attention_heads=1, num_key_value_heads=1)
        class Encoder:
            attention_window_frames = 8
            def encode_frames(self, mel):
                return mx.broadcast_to(mel.T, (mel.shape[-1], 128))
            def get_output_lengths(self, lengths):
                return lengths
            def __call__(self, mel, lengths):
                return self.encode_frames(mel[0])[None], lengths
        model = SimpleNamespace(model=TextDecoder(config), lm_head=nn.Linear(128, 32),
                                audio_tower=Encoder(), audio_token_id=31)
        model.create_cache = lambda max_seq_len: KVCache(3, max_seq_len)
        model._embed_tokens = model.model.embed_tokens
        def inject(embeddings, features, audio_mask):
            embeddings[:, 1:1+features.shape[1]] = features
            return embeddings
        model._inject_audio_features = inject
        tokenizer = SimpleNamespace(build_prompt_tokens=lambda n_audio_tokens, **kw: [1]+[31]*n_audio_tokens+[2],
                                    decode=lambda ids: ','.join(map(str, ids)))
        session = SimpleNamespace(model=model, dtype=mx.float32, tokenizer=tokenizer)
        raw = dict(speculators_model_type='dflash', sample_from_anchor=False,
                   block_size=4, mask_token_id=31, aux_hidden_state_layer_ids=[0, 2],
                   draft_vocab_size=32, sliding_window_non_causal=False,
                   transformer_layer_config={**config.__dict__, 'num_hidden_layers': 2})
        draft = Draft(raw, config)
        draft.target_ids = mx.arange(32)
        return session, Target(model), draft

    def test_cached_target_and_draft_follow_changed_tail_without_retaining_text(self):
        session, target, draft = self.runtime()
        prefix = DecoderPrefix()
        for ids, keep in [([1, 2, 3, 4, 5], 3), ([1, 2, 3, 7, 8, 9], 4),
                          ([1, 2, 3, 7, 11], 4)]:
            reused = prefix.target.offset if prefix.target else 0
            cached = decode(target, target.embed(ids[reused:]), 8, draft,
                            eos_token_ids=(), prefix=prefix, retain_positions=keep)
            plain = decode(target, target.embed(ids), 8, draft, eos_token_ids=())
            self.assertEqual(cached.tokens, plain.tokens)
            self.assertEqual(cached.finish_reason, plain.finish_reason)
            self.assertEqual((prefix.target.offset, prefix.draft.offset), (keep, keep))
            self.assertEqual(cached.target_cache_tokens, cached.draft_cache_tokens)
        with self.assertRaises(ValueError):
            decode(target, target.embed([8]), 8, draft, prefix=prefix, retain_positions=2)
        prefix.draft.offset -= 1
        with self.assertRaises(ValueError):
            decode(target, target.embed([8]), 8, draft, prefix=prefix, retain_positions=4)

    def test_profile_measures_materialized_stages_with_valid_durations(self):
        import time
        session, target, draft = self.runtime()
        timings = {}
        before = time.monotonic()
        decode(target, target.embed([1, 2, 3]), 8, draft, eos_token_ids=(), timings=timings)
        elapsed = time.monotonic()-before
        self.assertEqual(set(timings), {'prefill_s', 'generation_s'})
        self.assertTrue(all(0 <= value <= elapsed for value in timings.values()))
        self.assertLessEqual(sum(timings.values()), elapsed)

    def test_audio_block_reuse_invalidates_both_caches_on_global_mel_change(self):
        session, target, draft = self.runtime()
        # The test injector also handles prompt tails with no text head.
        def inject(embeddings, features, mask):
            start = int(mx.argmax(mask[0]).item())
            embeddings[:, start:start+features.shape[1]] = features
            return embeddings
        session.model._inject_audio_features = inject
        cached = CachedTranscriber(session, target, draft)
        for length, maximum, reused, invalidated in [(10, 1, 0, False), (12, 1, 1, False), (14, 2, 0, True)]:
            samples = np.arange(length, dtype=np.float32)
            mel = mx.ones((1, 1, length))*maximum
            feature_result = (mel, mx.array([length]))
            with patch('qwen_prefix_cache.compute_features', return_value=feature_result), \
                 patch('qwen_speculative.compute_features', return_value=feature_result):
                actual = cached.transcribe(samples, 8)
                expected = transcribe(session, target, draft, samples, 8)
            self.assertEqual(actual['tokens'], expected['tokens'])
            self.assertEqual(actual['audio_cache']['reused_blocks'], reused)
            self.assertEqual(actual['audio_cache']['invalidated'], invalidated)
            self.assertEqual(cached.decoder.target.offset, cached.decoder.draft.offset)
        with self.assertRaises(ValueError):
            cached.transcribe(np.arange(13, dtype=np.float32), 8)
        changed = np.arange(15, dtype=np.float32)
        changed[0] = -1
        with self.assertRaises(ValueError):
            cached.transcribe(changed, 8)
        cached.reset()
        self.assertIsNone(cached.decoder.target)
        self.assertIsNone(cached.state._prefix_cache)
        self.assertEqual(len(cached.audio), 0)


if __name__ == '__main__':
    unittest.main()
