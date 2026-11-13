"""Local, reproducible runtime compatibility shims; upstream trees remain untouched."""
import importlib
import json
import os
import sys
import time
from dataclasses import asdict
from replay_asr import PythonEngine, CppEngine


class ControlledPython(PythonEngine):
    def __init__(self, args):
        self.preprocessing_s = 0.0
        if args.backend == 'simulstreaming':
            if os.environ.get('PYTORCH_ENABLE_MPS_FALLBACK') not in (None, '0'):
                raise ValueError('implicit MPS CPU fallback must be disabled before importing torch')
            sys.path.insert(0, str(args.checkout))
            import torch
            if args.device == 'mps' and not torch.backends.mps.is_available():
                raise RuntimeError('MPS unavailable; explicit CPU configuration required')
            module = importlib.import_module('simulstreaming.whisper.simul_whisper.simul_whisper')
            original_load, original_mel = module.load_model, module.log_mel_spectrogram
            dtype = torch.float16 if args.device == 'mps' else torch.float32
            def load(*a, **kw):
                kw['device'] = 'cpu'
                return original_load(*a, **kw).to(device=args.device, dtype=dtype)
            def mel(audio, *a, **kw):
                before = time.monotonic()
                kw['device'] = 'cpu'
                result = original_mel(torch.as_tensor(audio, device='cpu'), *a, **kw).to(device=args.device, dtype=dtype)
                self.preprocessing_s += time.monotonic() - before
                return result
            module.load_model, module.log_mel_spectrogram = load, mel
            args.frame_threshold = int(args.frame_threshold)
            # Construct before hooks/context, not a late model.to() after initialization.
            from simulstreaming_whisper import SimulWhisperASR, SimulWhisperOnline
            self.asr = SimulWhisperASR(language=args.language, model_path=str(args.model),
                    cif_ckpt_path=None, frame_threshold=args.frame_threshold, audio_max_len=30,
                    audio_min_len=0, segment_length=args.step, beams=1, task='transcribe',
                    decoder_type='greedy', never_fire=False, init_prompt=None,
                    static_init_prompt=None, max_context_tokens=None, logdir=None)
            self.online = SimulWhisperOnline(self.asr)
            self.backend, self.committed = args.backend, ''
            self.activation_dtypes = {}
            def observe(name):
                def hook(module, inputs):
                    self.activation_dtypes[name] = str(inputs[0].dtype)
                return hook
            self.asr.model.model.encoder.register_forward_pre_hook(observe('encoder_input'))
            self.asr.model.model.decoder.register_forward_pre_hook(observe('decoder_tokens'))
            self.asr.model.model.encoder.register_forward_hook(lambda m,i,o: self.activation_dtypes.update(encoder_output=str(o.dtype)))
            self.asr.model.model.decoder.register_forward_hook(lambda m,i,o: self.activation_dtypes.update(decoder_logits=str(o.dtype)))
            self.audit = dict(native_finish_effects='SS finish runs last decode AND refresh_segment(complete=True), clearing native context/audio; explicit reset also resets online timestamps/display', activation_dtypes=self.activation_dtypes, device=str(self.asr.model.model.device),
                              weights=sorted(set(str(p.dtype) for p in self.asr.model.model.parameters())),
                              mel_dtype=str(dtype), preprocessing='CPU float32 STFT/mel then explicit device/dtype transfer',
                              accumulation='native torch kernels; probability softmax explicitly float32; not bit-equivalent across runtimes',
                              native_config=asdict(self.asr.model.cfg), implicit_cpu_fallback=False)
        else:
            super().__init__(args)
            import mlx.core as mx
            holder = importlib.import_module('mlx_whisper.transcribe').ModelHolder
            provenance = json.loads((args.model_root / 'provenance.json').read_text())
            holder.model.set_alignment_heads(provenance['alignment_heads'].encode())
            mx.eval(holder.model.parameters())
            self.online.buffer_trimming_sec = args.trimming
            self.asr.transcribe_kargs.update(temperature=0.0, fp16=True)
            self.audit = dict(device='MLX/Metal', weights='fp16 conversion', mel_dtype='runtime native',
                              accumulation='MLX native kernels; not asserted identical to torch/GGML',
                              agreement=2, trimming=args.trimming, decode_options=self.asr.transcribe_kargs)

    def sync(self):
        if self.backend == 'simulstreaming':
            import torch
            if self.asr.model.model.device.type == 'mps':
                torch.mps.synchronize()
        else:
            import mlx.core as mx
            mx.synchronize()

    def run(self, final=False):
        self.sync()
        if final and self.backend == 'whisper-streaming':
            start, end, text = self.online.finish()
            # finish previews pending text; preserving state must not append it
            # again when LocalAgreement later confirms the same buffer.
            result = dict(text=self.committed + text, append=text, start_s=start, end_s=end,
                          stage='flushed', rollover=False)
        else:
            result = super().run(final)
        self.sync()
        return result


class ControlledCpp(CppEngine):
    audit = dict(device='GGML/Metal', weights='f16 with converter-designated fp32 tensors',
                 accumulation='native GGML kernels, flash-attention; not bit-identical to torch/MLX',
                 context=False, greedy=True, threads=4, no_timestamps=True)
    preprocessing_s = 0.0
