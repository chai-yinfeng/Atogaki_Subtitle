# SPDX-License-Identifier: Apache-2.0
"""Experimental Qwen ASR block drafting; never imported by the production worker."""
from dataclasses import dataclass, asdict
import json
from pathlib import Path

import mlx.core as mx
import mlx.nn as nn
from mlx_qwen3_asr.audio import compute_features
from mlx_qwen3_asr.config import TextDecoderConfig
from mlx_qwen3_asr.tokenizer import parse_asr_output
from mlx_qwen3_asr.decoder import KVCache, TextDecoderLayer, _create_causal_mask_with_prefix
from mlx_qwen3_asr.generate import detect_repetition
from mlx_qwen3_asr.mrope import InterleavedMRoPE, apply_rotary_pos_emb


class Target:
    """Reuse the installed decoder layers, exposing their entering hidden states."""
    def __init__(self, model):
        self.model = model

    def forward(self, embeddings, cache, taps=(), *, last_only=False):
        decoder = self.model.model
        length = embeddings.shape[1]
        positions = mx.broadcast_to(mx.arange(cache.offset, cache.offset + length), (1, 3, length))
        cos, sin = decoder.rotary_emb(positions, dtype=embeddings.dtype)
        mask = (_create_causal_mask_with_prefix(length, cache.offset, embeddings.dtype)
                if length > 1 or cache.offset == 0 else None)
        hidden = embeddings
        captured = []
        for index, layer in enumerate(decoder.layers):
            if index in taps:
                captured.append(hidden)
            hidden = layer(hidden, cos, sin, mask=mask, cache=cache, layer_idx=index)
        hidden = decoder.norm(hidden)
        return self.model.lm_head(hidden[:, -1:] if last_only else hidden), captured

    def embed(self, tokens):
        return self.model.model.embed_tokens(mx.array([tokens], dtype=mx.int32))


class Draft(nn.Module):
    """Speculators DFlash/DSpark head with target-feature KV and causal blocks.

    Context projection and proposal alignment follow SGLang Omni's Apache-2.0
    dflash_draft.cpp at 4e03928295bba17c8196644f50d69e58facff6ea.
    DSpark's reduced-vocabulary Markov/confidence heads follow Speculators
    models/dspark at 05f9ba2324fef505cef8700669f2bdc20f8ae219 (Apache-2.0).
    """
    def __init__(self, raw, target_config):
        super().__init__()
        self.raw = raw
        self.algorithm = raw['speculators_model_type']
        if self.algorithm not in ('dflash', 'dspark'):
            raise ValueError('Only DFlash and DSpark heads are supported')
        if bool(raw.get('sample_from_anchor')) != (self.algorithm == 'dspark'):
            raise ValueError('Unexpected anchor-sampling alignment for this algorithm')
        if raw.get('sliding_window_non_causal'):
            raise ValueError('Bidirectional draft blocks are not supported')
        self.taps = tuple(raw['aux_hidden_state_layer_ids'])
        if not self.taps or self.taps != tuple(sorted(set(self.taps))):
            raise ValueError('Hidden-state taps must be nonempty, unique and ordered')
        if self.taps[0] < 0 or self.taps[-1] >= target_config.num_hidden_layers:
            raise ValueError('Hidden-state tap outside target decoder')
        config = dict(raw['transformer_layer_config'])
        config['rope_theta'] = config.get('rope_parameters', {}).get('rope_theta', 1000000.0)
        config = TextDecoderConfig.from_dict(config)
        if config.hidden_size != target_config.hidden_size:
            raise ValueError('Draft and target hidden sizes differ')
        self.block_size = raw['block_size']
        self.mask_token_id = raw['mask_token_id']
        if self.block_size < 2 or not 0 <= self.mask_token_id < target_config.vocab_size:
            raise ValueError('Invalid draft block size or mask token')
        self.config = config
        self.fc = nn.Linear(config.hidden_size * len(self.taps), config.hidden_size, bias=False)
        self.hidden_norm = nn.RMSNorm(config.hidden_size, eps=config.rms_norm_eps)
        self.layers = [TextDecoderLayer(config) for _ in range(config.num_hidden_layers)]
        for layer in self.layers:
            layer.self_attn.q_norm = nn.RMSNorm(config.head_dim, eps=config.rms_norm_eps)
            layer.self_attn.k_norm = nn.RMSNorm(config.head_dim, eps=config.rms_norm_eps)
        self.norm = nn.RMSNorm(config.hidden_size, eps=config.rms_norm_eps)
        self.lm_head = nn.Linear(config.hidden_size, raw['draft_vocab_size'], bias=False)
        self.rotary = InterleavedMRoPE(config.head_dim, config.rope_theta)
        if self.algorithm == 'dspark':
            if raw.get('markov_head_type') != 'vanilla' or raw.get('markov_rank', 0) < 1:
                raise ValueError('This experiment requires the vanilla DSpark Markov head')
            rank = raw['markov_rank']
            self.markov_head = dict(markov_w1=nn.Embedding(target_config.vocab_size, rank),
                                    markov_w2=nn.Linear(rank, raw['draft_vocab_size'], bias=False))
            if not raw.get('enable_confidence_head'):
                raise ValueError('The pinned DSpark experiment requires a confidence head')
            inputs = config.hidden_size + (rank if raw.get('confidence_head_with_markov') else 0)
            self.confidence_head = dict(proj=nn.Linear(inputs, 1))

    @classmethod
    def load(cls, directory, target_config, *, dtype=None):
        directory = Path(directory)
        raw = json.loads((directory / 'config.json').read_text())
        draft = cls(raw, target_config)
        weights = mx.load(str(directory / 'model.safetensors'))
        mapping = weights.pop('d2t')
        target_mapping = weights.pop('t2d', None)
        if target_mapping is not None and target_mapping.shape != (target_config.vocab_size,):
            raise ValueError('Invalid target vocabulary mapping shape')
        expected = (raw['draft_vocab_size'],)
        if mapping.shape != expected:
            raise ValueError('Invalid draft vocabulary mapping shape')
        target_ids = mapping.astype(mx.int32) + mx.arange(expected[0])
        if int(mx.min(target_ids).item()) < 0 or int(mx.max(target_ids).item()) >= target_config.vocab_size:
            raise ValueError('Draft vocabulary maps outside target vocabulary')
        ordered = mx.sort(target_ids)
        if not bool(mx.all(ordered[1:] > ordered[:-1]).item()):
            raise ValueError('Draft vocabulary mapping is not one-to-one')
        if target_mapping is not None and (target_mapping.dtype != mx.bool_ or
                int(mx.sum(target_mapping).item()) != expected[0] or
                not bool(mx.all(target_mapping[target_ids]).item())):
            raise ValueError('Target and draft vocabulary mappings disagree')
        if dtype is not None:
            weights = {name: value.astype(dtype) if mx.issubdtype(value.dtype, mx.floating) else value
                       for name, value in weights.items()}
        draft.load_weights(list(weights.items()), strict=True)
        draft.target_ids = target_ids
        mx.eval(draft.parameters())
        draft.eval()
        return draft

    def new_cache(self, capacity):
        return KVCache(len(self.layers), max_seq_len=capacity)

    def append_context(self, captured, count, cache):
        inputs = mx.concatenate([x[:, :count] for x in captured], axis=-1).astype(self.fc.weight.dtype)
        hidden = self.hidden_norm(self.fc(inputs))
        length = hidden.shape[1]
        positions = mx.broadcast_to(mx.arange(cache.offset, cache.offset + length), (1, 3, length))
        cos, sin = self.rotary(positions, dtype=hidden.dtype)
        for index, layer in enumerate(self.layers):
            attention = layer.self_attn
            keys = attention.k_proj(hidden).reshape(1, length, attention.num_kv_heads, attention.head_dim)
            keys = attention.k_norm(keys).transpose(0, 2, 1, 3)
            values = attention.v_proj(hidden).reshape(1, length, attention.num_kv_heads, attention.head_dim)
            _, keys = apply_rotary_pos_emb(keys, keys, cos, sin)
            cache.update(keys, values.transpose(0, 2, 1, 3), index)

    def propose(self, anchor, target, cache):
        hidden = target.embed([anchor] + [self.mask_token_id] * (self.block_size - 1)).astype(self.fc.weight.dtype)
        positions = mx.broadcast_to(mx.arange(cache.offset, cache.offset + self.block_size), (1, 3, self.block_size))
        cos, sin = self.rotary(positions, dtype=hidden.dtype)
        mask = _create_causal_mask_with_prefix(self.block_size, cache.offset, hidden.dtype)
        for index, layer in enumerate(self.layers):
            hidden = layer(hidden, cos, sin, mask=mask, cache=cache, layer_idx=index)
        cache.trim(self.block_size)
        hidden = self.norm(hidden if self.algorithm == 'dspark' else hidden[:, 1:])
        logits = self.lm_head(hidden)
        finite = mx.all(mx.isfinite(logits))
        if self.algorithm == 'dflash':
            return self.target_ids[mx.argmax(logits, axis=-1)].reshape(-1), None, finite
        previous = mx.array([anchor], dtype=mx.int32)
        proposals, previous_embeddings = [], []
        for index in range(self.block_size):
            embedding = self.markov_head['markov_w1'](previous)
            previous_embeddings.append(embedding)
            corrected = logits[:, index] + self.markov_head['markov_w2'](embedding)
            finite = finite & mx.all(mx.isfinite(corrected))
            previous = self.target_ids[mx.argmax(corrected, axis=-1)]
            proposals.append(previous)
        confidence_inputs = hidden
        if self.raw.get('confidence_head_with_markov'):
            confidence_inputs = mx.concatenate([hidden, mx.stack(previous_embeddings, axis=1)], axis=-1)
        confidence = self.confidence_head['proj'](confidence_inputs).reshape(-1)
        return mx.concatenate(proposals), confidence, finite & mx.all(mx.isfinite(confidence))


@dataclass
class DecodeResult:
    tokens: list[int]
    finish_reason: str
    verify_rounds: int
    proposed_tokens: int
    accepted_tokens: int
    target_cache_tokens: int
    draft_cache_tokens: int
    rejected_rounds: int
    min_confidence_logit: float | None = None
    max_confidence_logit: float | None = None


def decode(target, prompt_embeddings, max_new_tokens, draft=None, *, eos_token_ids=(151643, 151645)):
    """Greedy target verification with per-call caches and exact rejection rollback."""
    if max_new_tokens < 0:
        raise ValueError('max_new_tokens must be nonnegative')
    prompt_length = prompt_embeddings.shape[1]
    if draft and prompt_length + max_new_tokens + draft.block_size > draft.raw['transformer_layer_config'].get('sliding_window', 32768):
        raise ValueError('This short-crop experiment does not implement sliding-window eviction')
    if max_new_tokens == 0:
        return DecodeResult([], 'length', 0, 0, 0, 0, 0, 0)
    cache = target.model.create_cache(max_seq_len=prompt_length + max_new_tokens)
    taps = draft.taps if draft else ()
    logits, captured = target.forward(prompt_embeddings, cache, taps, last_only=True)
    draft_cache = draft.new_cache(prompt_length + max_new_tokens + draft.block_size) if draft else None
    if draft:
        draft.append_context(captured, prompt_length, draft_cache)
    tokens = []
    finish = None
    observed = 0
    rounds = proposed_count = accepted_count = rejected_rounds = 0
    confidence_values = []

    def commit(token):
        nonlocal finish, observed
        observed += 1
        if token in eos_token_ids:
            finish = 'eos'
        else:
            tokens.append(token)
            if observed >= max_new_tokens:
                finish = 'length'
            elif detect_repetition(tokens):
                finish = 'repetition'
        return finish is not None

    anchor = int(mx.argmax(logits[0, -1]).item())
    commit(anchor)
    while finish is None:
        if draft is None:
            logits, _ = target.forward(target.embed([anchor]), cache, last_only=True)
            anchor = int(mx.argmax(logits[0, -1]).item())
            commit(anchor)
            continue
        proposals, confidence, finite = draft.propose(anchor, target, draft_cache)
        verify_count = min(1 + proposals.size, max_new_tokens - observed)
        verify_ids = mx.concatenate([mx.array([anchor], dtype=mx.int32), proposals])[:verify_count]
        logits, captured = target.forward(target.model.model.embed_tokens(verify_ids[None]), cache, taps)
        predictions = mx.argmax(logits, axis=-1).reshape(-1)
        mx.eval(proposals, predictions, finite, *([confidence] if confidence is not None else []))
        if not bool(finite.item()):
            raise FloatingPointError('Draft logits are non-finite; keep the checkpoint BF16 precision')
        if confidence is not None:
            confidence_values.extend(confidence.tolist())
        proposed = proposals.tolist()
        predicted = predictions.tolist()
        rounds += 1
        proposed_count += verify_count - 1
        accepted = 0
        before = observed
        while accepted < verify_count - 1 and proposed[accepted] == predicted[accepted]:
            stopped = commit(proposed[accepted])
            accepted += 1
            if stopped:
                break
        accepted_count += accepted
        if finish is None:
            rejected_rounds += int(accepted < verify_count - 1)
            anchor = predicted[accepted]
            commit(anchor)
        keep = observed - before
        cache.trim(verify_count - keep)
        draft.append_context(captured, keep, draft_cache)
        if cache.offset != draft_cache.offset:
            raise AssertionError('Target and draft committed prefixes diverged')
    return DecodeResult(tokens, finish, rounds, proposed_count, accepted_count,
                        cache.offset, draft_cache.offset if draft_cache else 0, rejected_rounds,
                        min(confidence_values) if confidence_values else None,
                        max(confidence_values) if confidence_values else None)


def transcribe(session, target, draft, samples, max_new_tokens=512):
    """Run the unchanged short-crop audio path with the experimental decoder."""
    mel, lengths = compute_features(samples)
    features, _ = session.model.audio_tower(mel.astype(session.dtype), lengths)
    prompt = session.tokenizer.build_prompt_tokens(n_audio_tokens=features.shape[1], language='Japanese', context='')
    ids = mx.array([prompt], dtype=mx.int32)
    embeddings = session.model._embed_tokens(ids)
    embeddings = session.model._inject_audio_features(embeddings, features, ids == session.model.audio_token_id)
    result = decode(target, embeddings, max_new_tokens, draft)
    _, text = parse_asr_output(session.tokenizer.decode(result.tokens), user_language='Japanese')
    return dict(text=text, **asdict(result))
