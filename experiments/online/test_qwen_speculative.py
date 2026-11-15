"""Contract checks for the experimental block-verification adapter."""
from types import SimpleNamespace
from dataclasses import asdict
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

try:
    import mlx.core as mx
    import mlx.nn as nn
    from mlx.utils import tree_flatten
    from mlx_qwen3_asr.config import TextDecoderConfig
    from mlx_qwen3_asr.decoder import TextDecoder, KVCache
    from qwen_speculative import Target, Draft, decode
    HAS_MLX = True
except ImportError:
    HAS_MLX = False


@unittest.skipUnless(HAS_MLX, 'requires the isolated MLX ASR environment')
class SpeculativeTests(unittest.TestCase):
    def test_target_taps_preserve_forward_and_rollback(self):
        mx.random.seed(7)
        config = TextDecoderConfig(vocab_size=32, hidden_size=128, intermediate_size=256,
                                  num_hidden_layers=3, num_attention_heads=1, num_key_value_heads=1)
        model = SimpleNamespace(model=TextDecoder(config), lm_head=nn.Linear(128, 32),
                                config=SimpleNamespace(text_config=config))
        target = Target(model)
        embeddings = target.embed([1, 2, 3])
        positions = mx.broadcast_to(mx.arange(3), (1, 3, 3))
        actual, taps = target.forward(embeddings, KVCache(3, 16), (0, 2))
        expected = model.lm_head(model.model(inputs_embeds=embeddings, position_ids=positions, cache=KVCache(3, 16)))
        self.assertTrue(bool(mx.allclose(actual, expected, atol=1e-6).item()))
        self.assertEqual(len(taps), 2)
        cache = KVCache(3, 16)
        target.forward(embeddings, cache)
        target.forward(target.embed([4, 5, 6]), cache)
        cache.trim(2)
        repaired, _ = target.forward(target.embed([7]), cache)
        clean = KVCache(3, 16)
        target.forward(target.embed([1, 2, 3, 4]), clean)
        expected, _ = target.forward(target.embed([7]), clean)
        self.assertTrue(bool(mx.allclose(repaired, expected, atol=1e-6).item()))
        self.assertEqual(cache.offset, clean.offset)

    def test_accept_reject_budget_and_eos(self):
        class FakeTarget:
            def __init__(self):
                self.model = SimpleNamespace(create_cache=lambda max_seq_len: KVCache(1, max_seq_len),
                                             model=SimpleNamespace(embed_tokens=lambda ids: ids[..., None]))
            def embed(self, tokens):
                return mx.array([tokens], dtype=mx.int32)[..., None]
            def forward(self, embeddings, cache, taps=(), last_only=False):
                cache.offset += embeddings.shape[1]
                ids = (embeddings[..., 0].astype(mx.int32) + 1) % 32
                return mx.eye(32)[ids], [embeddings]
        class FakeDraft:
            taps = (0,)
            block_size = 4
            raw = {'transformer_layer_config': {'sliding_window': 2048}}
            def __init__(self, mode):
                self.mode = mode
            def new_cache(self, capacity):
                return KVCache(1, capacity)
            def append_context(self, captured, count, cache):
                cache.offset += count
            def propose(self, anchor, target, cache):
                proposals = [anchor + 1, anchor + 2, anchor + 3]
                if self.mode == 'reject':
                    proposals[0] = 31
                if self.mode == 'partial':
                    proposals[1] = 31
                return mx.array(proposals, dtype=mx.int32), None, mx.array(self.mode != 'invalid')
        for mode in ['accept', 'reject', 'partial']:
            for budget in [0, 1, 2, 5, 12]:
                with self.subTest(mode=mode, budget=budget):
                    target = FakeTarget()
                    plain = decode(target, target.embed([2]), budget, eos_token_ids=(9,))
                    spec = decode(target, target.embed([2]), budget, FakeDraft(mode), eos_token_ids=(9,))
                    self.assertEqual((spec.tokens, spec.finish_reason), (plain.tokens, plain.finish_reason))
                    self.assertEqual(spec.target_cache_tokens, plain.target_cache_tokens)
                    if budget:
                        self.assertEqual(spec.target_cache_tokens, spec.draft_cache_tokens)
                    if mode == 'reject' and budget > 2:
                        self.assertGreater(spec.rejected_rounds, 0)
        target = FakeTarget()
        with self.assertRaises(FloatingPointError):
            decode(target, target.embed([2]), 10, FakeDraft('invalid'))
        first_eos = decode(target, target.embed([8]), 10, FakeDraft('accept'), eos_token_ids=(9,))
        self.assertEqual((first_eos.tokens, first_eos.finish_reason, first_eos.verify_rounds), ([], 'eos', 0))
        with self.assertRaises(ValueError):
            decode(target, target.embed([2]), -1)
        class RepeatingTarget(FakeTarget):
            def forward(self, embeddings, cache, taps=(), last_only=False):
                logits, captured = super().forward(embeddings, cache, taps, last_only=last_only)
                return mx.broadcast_to(mx.eye(32)[5], logits.shape), captured
        target = RepeatingTarget()
        with patch('qwen_speculative.detect_repetition', side_effect=lambda tokens: len(tokens) >= 3):
            for budget, reason in [(2, 'length'), (3, 'length'), (4, 'repetition')]:
                result = decode(target, target.embed([2]), budget, FakeDraft('reject'))
                self.assertEqual(result.finish_reason, reason)


    def test_reduced_vocabulary_loader_and_dspark_heads(self):
        config = TextDecoderConfig(vocab_size=32, hidden_size=128, intermediate_size=256,
                                  num_hidden_layers=3, num_attention_heads=1, num_key_value_heads=1)
        class EmbedTarget:
            def embed(self, tokens):
                return mx.zeros((1, len(tokens), 128))
        for algorithm in ['dflash', 'dspark']:
            raw = dict(speculators_model_type=algorithm, sample_from_anchor=algorithm == 'dspark',
                       block_size=4, mask_token_id=31, aux_hidden_state_layer_ids=[0, 2],
                       draft_vocab_size=16, sliding_window_non_causal=False,
                       transformer_layer_config={**asdict(config), 'num_hidden_layers': 2},
                       markov_head_type='vanilla', markov_rank=4, enable_confidence_head=True,
                       confidence_head_with_markov=True)
            head = Draft(raw, config)
            head.lm_head.weight = mx.zeros_like(head.lm_head.weight)
            if algorithm == 'dspark':
                head.markov_head['markov_w1'].weight = mx.ones((32, 4))
                weights = mx.zeros((16, 4))
                weights[3] = 1
                head.markov_head['markov_w2'].weight = weights
                head.confidence_head['proj'].weight = mx.zeros((1, 132))
                head.confidence_head['proj'].bias = mx.array([0.25])
            stored = dict(tree_flatten(head.parameters()))
            stored.update(d2t=mx.arange(16), t2d=mx.arange(32) % 2 == 0)
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory)
                (path / 'config.json').write_text(json.dumps(raw))
                mx.save_safetensors(str(path / 'model.safetensors'), stored)
                loaded = Draft.load(path, config)
                cache = loaded.new_cache(32)
                loaded.append_context([mx.zeros((1, 3, 128))] * 2, 3, cache)
                proposals, confidence, finite = loaded.propose(2, EmbedTarget(), cache)
                self.assertEqual(cache.offset, 3)
                self.assertTrue(bool(finite.item()))
                self.assertEqual(proposals.tolist(), [6] * 4 if algorithm == 'dspark' else [0] * 3)
                if algorithm == 'dspark':
                    self.assertEqual(confidence.tolist(), [0.25] * 4)
                stored['t2d'] = mx.ones((32,), dtype=mx.bool_)
                mx.save_safetensors(str(path / 'model.safetensors'), stored)
                with self.assertRaises(ValueError):
                    Draft.load(path, config)

    def test_draft_rejects_incompatible_configuration(self):
        config = TextDecoderConfig()
        with self.assertRaises(ValueError):
            Draft({'speculators_model_type': 'dspark'}, config)
        with self.assertRaises(ValueError):
            Draft({'speculators_model_type': 'dflash', 'sliding_window_non_causal': True}, config)


if __name__ == '__main__':
    unittest.main()
