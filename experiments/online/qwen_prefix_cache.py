# SPDX-License-Identifier: Apache-2.0
"""Isolated immutable audio-prefix reuse for the pinned MLX/DFlash adapter."""
from dataclasses import asdict
import time
from types import SimpleNamespace

import mlx.core as mx
import numpy as np
from mlx_qwen3_asr.audio import compute_features
from mlx_qwen3_asr.streaming import _encode_with_prefix_cache, _prompt_audio_start
from mlx_qwen3_asr.tokenizer import parse_asr_output

from qwen_speculative import DecoderPrefix, decode


class CachedTranscriber:
    """One append-only audio crop; text is decoded afresh on every request.

    Reuse the installed 0.4.4 encoder cache and its mel-max/tail-frame guards.
    Target and DFlash KV snapshots end at the same complete audio block.
    No forced text prefix, attention-mask change, or mel normalization change.
    """
    def __init__(self, session, target, draft):
        self.session, self.target, self.draft = session, target, draft
        self.reset()

    def reset(self):
        self.audio = np.empty(0, dtype=np.float32)
        self.state = SimpleNamespace(_prefix_cache=None)
        self.decoder = DecoderPrefix()

    def transcribe(self, samples, max_new_tokens=512):
        timings = {}
        before = time.monotonic()
        samples = np.asarray(samples, dtype=np.float32)
        if samples.ndim != 1 or len(samples) > 30*16000:
            raise ValueError('Cache experiment requires mono crops of at most 30 seconds')
        if len(samples) < len(self.audio) or not np.array_equal(samples[:len(self.audio)], self.audio):
            raise ValueError('Reset the audio cache before replacing or shortening its input')
        mel, _ = compute_features(samples)
        mx.eval(mel)
        mx.synchronize()
        timings['features_s'] = time.monotonic()-before
        before = time.monotonic()
        old = self.state._prefix_cache
        audio_cache, features, new_blocks = _encode_with_prefix_cache(
            self.state, self.session.model, mel, self.session.dtype)
        invalidated = old is not None and old is not audio_cache and bool(old.blocks)
        if old is not audio_cache:
            self.decoder = DecoderPrefix()
        reused_blocks = len(audio_cache.blocks)-new_blocks
        mx.eval(features)
        mx.synchronize()
        timings['encoder_s'] = time.monotonic()-before
        reused_positions = self.decoder.target.offset if self.decoder.target is not None else 0
        total_tokens = reused_blocks*audio_cache.tokens_per_block + int(features.shape[0])
        prompt = self.session.tokenizer.build_prompt_tokens(
            n_audio_tokens=total_tokens, language='Japanese', context='')
        head = _prompt_audio_start(prompt, self.session.model.audio_token_id, total_tokens)
        retain = head+audio_cache.cached_tokens if audio_cache.blocks else 0
        if reused_positions != (head+reused_blocks*audio_cache.tokens_per_block if reused_blocks else 0):
            raise AssertionError('Encoder, target and draft prefix boundaries disagree')
        ids = mx.array([prompt[reused_positions:]], dtype=mx.int32)
        embeddings = self.session.model._embed_tokens(ids)
        embeddings = self.session.model._inject_audio_features(
            embeddings, features[None, :, :], ids == self.session.model.audio_token_id)
        result = decode(self.target, embeddings, max_new_tokens, self.draft,
                        prefix=self.decoder, retain_positions=retain, timings=timings)
        _, text = parse_asr_output(self.session.tokenizer.decode(result.tokens), user_language='Japanese')
        self.audio = samples.copy()
        return dict(text=text, **asdict(result), timings=timings,
                    audio_cache=dict(reused_blocks=reused_blocks, new_blocks=new_blocks,
                                     invalidated=invalidated, reused_positions=reused_positions,
                                     retained_positions=retain, mel_frames=int(mel.shape[-1]),
                                     encoded_mel_frames=int(mel.shape[-1])-reused_blocks*int(
                                         self.session.model.audio_tower.attention_window_frames)))
