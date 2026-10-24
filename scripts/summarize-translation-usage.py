#!/usr/bin/env python3
"""Summarize local pipeline journals without API calls or subtitle text output.

Semantic groups are not HTTP requests. Unknown usage is never replaced by zero.
This journal is not a complete invoice, especially for failed HTTP attempts.
"""
import argparse
import json
from pathlib import Path


def count(value):
    return value if isinstance(value, int) and not isinstance(value, bool) and value >= 0 else None


def summarize(job):
    report = {
        "schema_version": 1, "job": str(job),
        "semantic_group_attempts": 0, "groups_with_provider_response": 0, "groups_without_provider_response": 0,
        "local_result_cache_hits": 0, "unknown_scope_records": 0,
        "reported_input_tokens": None, "reported_output_tokens": None,
        "reported_cached_input_tokens": None, "reported_uncached_input_tokens": None,
        "groups_with_reported_input_usage": 0,
        "groups_with_both_prompt_cache_counts": 0,
        "prompt_cache_hit_ratio_for_reported_counts": None,
        "http_requests": None,
        "limits": ["Group attempts and cache files cannot determine HTTP/retry counts",
                   "Batch usage belongs to one recipient group; missing usage in other groups is expected",
                   "Unknown fields and failed-request costs are not zero; this is not a complete bill",
                   "This journal covers the early/final pipeline, not every editor retranslation"],
    }
    matched_hit = matched_miss = 0
    for file in sorted((job / "early-translation/attempts").glob("*.json")):
        record = json.loads(file.read_text())
        if record.get("attempt_scope") != "semantic_group":
            report["unknown_scope_records"] += 1
            continue
        report["semantic_group_attempts"] += 1
        response = record.get("response")
        if not isinstance(response, dict):
            report["groups_without_provider_response"] += 1
            continue
        report["groups_with_provider_response"] += 1
        usage = response.get("usage") or {}
        if usage.get("local_cache_hit") is True:
            raise ValueError(f"local reuse was recorded as a provider attempt: {file.name}")
        for source, target in [("input_tokens", "reported_input_tokens"),
                               ("output_tokens", "reported_output_tokens"),
                               ("cached_input_tokens", "reported_cached_input_tokens"),
                               ("uncached_input_tokens", "reported_uncached_input_tokens")]:
            value = count(usage.get(source))
            if value is not None:
                report[target] = (report[target] or 0) + value
        if count(usage.get("input_tokens")) is not None:
            report["groups_with_reported_input_usage"] += 1
        hit, miss = count(usage.get("cached_input_tokens")), count(usage.get("uncached_input_tokens"))
        if hit is not None and miss is not None:
            report["groups_with_both_prompt_cache_counts"] += 1
            matched_hit += hit
            matched_miss += miss
    for file in sorted((job / "early-translation/cache-hits").glob("*.json")):
        event = json.loads(file.read_text())
        if event.get("event_scope") == "local_result_cache_hit":
            report["local_result_cache_hits"] += 1
        else:
            report["unknown_scope_records"] += 1
    if matched_hit + matched_miss:
        report["prompt_cache_hit_ratio_for_reported_counts"] = matched_hit / (matched_hit + matched_miss)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("job", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if not args.job.is_dir():
        parser.error("job directory does not exist")
    report = summarize(args.job.resolve())
    with args.output.open("x") as stream:
        json.dump(report, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
