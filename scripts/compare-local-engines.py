#!/usr/bin/env python3
"""Compare local runtimes using Atogaki's shared ASR/translation contracts.

No downloads, cloud requests, App SQLite access, or automatic recommendation.
Call with the explicit experimental Python environment containing psutil.
"""
import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import sys
import threading
import time
import urllib.request
import uuid


def digest(path):
    path = Path(path)
    if path.is_dir():
        value = hashlib.sha256()
        for child in sorted(path.iterdir()):
            if child.name in {".cache", ".git"}:
                continue
            if child.is_symlink() and child.is_dir():
                raise ValueError("symlinked model directory")
            value.update(child.name.encode())
            value.update(b"\0")
            value.update(digest(child).encode())
            value.update(b"\0")
        return value.hexdigest()
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for data in iter(lambda: stream.read(65536), b""):
            value.update(data)
    return value.hexdigest()


def versions(python):
    code = """import importlib.metadata as m,json
out={}
for name in ['mlx','mlx-whisper','mlx-lm','psutil']:
 try: out[name]=m.version(name)
 except m.PackageNotFoundError: out[name]=None
print(json.dumps(out))"""
    return json.loads(subprocess.check_output([str(python), "-c", code], text=True))


class Monitor:
    def __init__(self, process):
        self.process = process
        self.stop = threading.Event()
        self.peak = None
        self.thread = threading.Thread(target=self.sample, daemon=True)
        self.thread.start()

    def sample(self):
        try:
            import psutil
        except ImportError:
            return
        while not self.stop.is_set():
            try:
                process = psutil.Process(self.process.pid)
                total = process.memory_info().rss
                for child in process.children(recursive=True):
                    try:
                        total += child.memory_info().rss
                    except psutil.Error:
                        pass
                self.peak = max(self.peak or 0, total)
            except psutil.Error:
                pass
            self.stop.wait(0.05)

    def close(self):
        self.stop.set()
        self.thread.join()
        return self.peak


def measured(command, log, env):
    started = time.perf_counter()
    with log.open("w") as stream:
        process = subprocess.Popen(command, env=env, stdout=stream, stderr=subprocess.STDOUT)
        monitor = Monitor(process)
        try:
            returncode = process.wait()
        except BaseException:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            raise
        finally:
            peak = monitor.close()
    return {"elapsed_ms": round((time.perf_counter()-started)*1000),
            "sampled_peak_process_tree_rss_bytes": peak, "exit_code": returncode}


def server_run(command, cli_command, output, env):
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    started = time.perf_counter()
    env = dict(env, ATOGAKI_TRANSLATION_EVAL_API_KEY=secrets.token_hex(24))
    with output.with_suffix(".server.log").open("w") as stream:
        server = subprocess.Popen(command + ["--host", "127.0.0.1", "--port", str(port)],
                                  stdout=stream, stderr=subprocess.STDOUT, env=env)
        monitor = Monitor(server)
        try:
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
            deadline = time.monotonic() + 180
            while True:
                if server.poll() is not None:
                    raise RuntimeError("local model server stopped; inspect isolated server log")
                try:
                    opener.open(f"http://127.0.0.1:{port}/v1/models", timeout=1).close()
                    break
                except OSError:
                    if time.monotonic() >= deadline:
                        raise TimeoutError("local model load timed out")
                    time.sleep(0.2)
            load_ms = round((time.perf_counter()-started)*1000)
            result = measured(cli_command + ["--base-url", f"http://127.0.0.1:{port}/v1"], output.with_suffix(".client.log"), env)
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
            peak = monitor.close()
        result.update(cold_server_ready_ms=load_ms,
                      sampled_peak_server_tree_rss_bytes=peak,
                      server_reaped=True)
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["probe", "asr", "translation"])
    parser.add_argument("--python", type=Path, default=Path(sys.executable))
    parser.add_argument("--cli", type=Path, default=Path("target/debug/atogaki-subtitle"))
    parser.add_argument("--output", type=Path, default=Path("local-artifacts/mlx"))
    parser.add_argument("--input", type=Path)
    parser.add_argument("--model", type=Path)
    parser.add_argument("--baseline-model", type=Path)
    parser.add_argument("--ffmpeg", type=Path, default=Path("/Applications/Atogaki.app/Contents/MacOS/ffmpeg"))
    parser.add_argument("--whisper-cli", type=Path, default=Path("/Applications/Atogaki.app/Contents/MacOS/whisper-cli"))
    parser.add_argument("--llama-server", type=Path, default=Path("/Applications/Atogaki.app/Contents/MacOS/llama-server"))
    parser.add_argument("--source-language", default="ja", choices=["ja","en","ko"])
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--protected-term", action="append", default=[])
    args = parser.parse_args()
    output = args.output.resolve() / f"{args.mode}-{uuid.uuid4()}"
    output.mkdir(parents=True, exist_ok=False)
    runtime = versions(args.python)
    report = {"schema_version":1, "runtime_versions":runtime,
              "quality_conclusion":"not established; human review required",
              "runs":{}, "sampling_interval_ms":50}
    if args.mode != "probe":
        if not args.input or not args.model or not args.baseline_model:
            parser.error("input, model and baseline-model are required")
        for item in [args.input,args.model,args.baseline_model,args.cli]:
            if not item.exists():
                parser.error(f"missing local resource: {item}")
        report["identities"] = {"input":digest(args.input),"mlx_model":digest(args.model),"baseline_model":digest(args.baseline_model)}
        env = dict(os.environ, HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1")
        if args.mode == "asr":
            common = [str(args.cli),"--ffmpeg",str(args.ffmpeg),"--whisper-cli",str(args.whisper_cli),"transcribe",str(args.input),"--source-language",args.source_language]
            for label, model in [("cpp",args.baseline_model),("mlx",args.model)]:
                command = common + ["--model",str(model),"--output-dir",str(output/label)]
                if label == "mlx":
                    command += ["--engine","mlx-whisper","--python",str(args.python)]
                report["runs"][label] = measured(command,output/f"{label}.log",env)
                chunks = [json.loads(p.read_text()) for p in (output/label/"asr-checkpoint").glob("chunk-*/evidence.json")]
                report["runs"][label]["recognition_elapsed_ms"] = sum(c.get("recognition_elapsed_ms",0) for c in chunks) if chunks else None
                report["runs"][label]["completed_chunks"] = len(chunks)
                if args.reference and report["runs"][label]["exit_code"] == 0:
                    measured([str(args.cli),"evaluate-asr","--reference",str(args.reference),"--hypothesis",str(output/label/"segments.json"),"--output",str(output/f"{label}-quality.json")],output/f"{label}-quality.log",env)
        else:
            for label, model in [("cpp",args.baseline_model),("mlx",args.model)]:
                run_output = output/f"{label}.json"
                command=[str(args.cli),"evaluate-translation","--input",str(args.input),"--output",str(run_output),"--model",str(model),"--source-language",args.source_language,"--provider-name",label]
                for term in args.protected_term:
                    command += ["--protected-term",term]
                server = [str(args.llama_server),"--model",str(model),"--ctx-size","8192","--jinja","--n-gpu-layers","99"] if label=="cpp" else [str(args.python),"-m","mlx_lm.server","--model",str(model)]
                report["runs"][label] = server_run(server,command,run_output,env)
                report["runs"][label]["structure_passed"] = report["runs"][label]["exit_code"] == 0
            if all(r["structure_passed"] for r in report["runs"].values()):
                review = Path(__file__).with_name("prepare-translation-blind-review.py")
                subprocess.run([str(args.python),str(review),"--candidate",f"cpp={output/'cpp.json'}","--candidate",f"mlx={output/'mlx.json'}","--output",str(output/"blind-review.json"),"--answer-key",str(output/"answer-key.json")],check=True)
    (output/"report.json").write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding="utf-8")
    print(output/"report.json")


if __name__ == "__main__":
    main()
