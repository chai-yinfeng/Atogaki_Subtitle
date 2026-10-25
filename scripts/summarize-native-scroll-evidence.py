"""Summarize local xctrace exports and CUA input timing, without inventing frame P95.

Run after every export process has completed. The script neither records/controls
an app nor deletes traces. XML exports may include other system processes; the
report exposes only the attached target, table schemas and aggregate row counts.
"""
import argparse
import datetime
import json
from pathlib import Path
import xml.etree.ElementTree as ET

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('directory', type=Path)
parser.add_argument('--output', required=True, type=Path)
args = parser.parse_args()


def run_info(name):
    root = ET.parse(args.directory / name).getroot()
    summary = root.find('.//summary')
    target = root.find('.//target/process')
    start = summary.findtext('start-date')
    end = summary.findtext('end-date')
    return {
        'start': start, 'end': end,
        'start_ms': datetime.datetime.fromisoformat(start).timestamp() * 1000,
        'end_ms': datetime.datetime.fromisoformat(end).timestamp() * 1000,
        'duration_s': float(summary.findtext('duration')),
        'target_pid': int(target.get('pid')),
        'template': summary.findtext('template-name'),
    }


actions = [json.loads(line) for line in (args.directory / 'actions.jsonl').read_text().splitlines() if line]
info = run_info('input-toc.xml')
covered = [action for action in actions if info['start_ms'] <= action['start_ms'] <= action['end_ms'] <= info['end_ms']]
initial = run_info('webcontent-toc.xml')
initial_covered = [action for action in actions if initial['start_ms'] <= action['start_ms'] <= action['end_ms'] <= initial['end_ms']]
tables = {}
for name in ['potential-hangs', 'hitches-updates', 'hitches-renders', 'displayed-surfaces-interval', 'visual-connection', 'metal-known-compositor-process', 'hitches']:
    root = ET.parse(args.directory / (name + '.xml')).getroot()
    schema = root.find('.//schema')
    tables[name] = {
        'rows': sum(1 for _ in root.iter('row')),
        'columns': [{'name': col.findtext('mnemonic'), 'type': col.findtext('engineering-type')} for col in schema.findall('col')] if schema is not None else [],
    }

anchor_cues = [action["anchor_cue"] for action in covered if "anchor_cue" in action]
report = {
    'trace': info,
    'initial_diagnostic': {**initial, 'covered_scroll_calls': len(initial_covered)},
    'recorded_scroll_calls': len(actions),
    'covered_scroll_calls': len(covered),
    'covered_input_span_s': (covered[-1]['end_ms'] - covered[0]['start_ms']) / 1000 if covered else None,
    'directions': {direction: sum(action['direction'] == direction for action in covered) for direction in ['up', 'down']},
    'observed_virtual_row_counts': sorted(set(action['observed_rows'] for action in covered if 'observed_rows' in action)),
    'anchor_cue_range': [min(anchor_cues), max(anchor_cues)] if anchor_cues else None,
    'tables': tables,
    'presentation_interval_p95_ms': None,
    'metric_status': 'Not computed: display/surface records require verified target-process attribution; frame lifetime and render duration are not presentation intervals.',
    'limitations': ['CUA AX reads interleave with scrolling and add observation work.', 'No before/after native package comparison.', 'Paused media at 0; not a video decoding or playback-follow workload.', 'Empty potential-hangs/updates tables do not prove no jank or hangs.'],
}
args.output.parent.mkdir(parents=True, exist_ok=True)
with args.output.open('x') as stream:
    json.dump(report, stream, indent=2)
    stream.write('\n')
print(json.dumps({key: report[key] for key in ['covered_scroll_calls', 'covered_input_span_s', 'directions', 'observed_virtual_row_counts', 'anchor_cue_range', 'presentation_interval_p95_ms']}))
