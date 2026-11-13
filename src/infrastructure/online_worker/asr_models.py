"""CPU first-pass adapters; imports occur only in the owning worker."""
def zipformer(model_dir):
    import sherpa_onnx
    def pick(prefix, quantized=True):
        files = sorted(model_dir.glob(prefix + '*.int8.onnx')) if quantized else []
        if not files:
            files = sorted(p for p in model_dir.glob(prefix + '*.onnx') if '.int8.' not in p.name)
        if len(files) != 1:
            raise ValueError(f'expected one {prefix} model, found {files}')
        return str(files[0])
    return sherpa_onnx.OnlineRecognizer.from_transducer(
        tokens=str(model_dir / 'tokens.txt'), encoder=pick('encoder'),
        decoder=pick('decoder', False), joiner=pick('joiner'),
        num_threads=1, provider='cpu', enable_endpoint_detection=False)
