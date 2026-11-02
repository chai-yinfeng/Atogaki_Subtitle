#!/usr/bin/env python3
"""Private cancellation-only interposition; never used for performance evidence."""
import json
from pathlib import Path
import sys
import time
import replay_controlled
from controlled_engine import ControlledCpp, ControlledPython


def main():
    output = Path(sys.argv[sys.argv.index('--output-dir')+1])
    count = 0
    def wrap(original):
        def run(engine, final=False):
            nonlocal count
            count += 1
            with (output/'cancel-probe.jsonl').open('a') as f:
                f.write(json.dumps(dict(started_call=count, final=final, monotonic_s=time.monotonic()))+'\n')
            return original(engine, final)
        return run
    ControlledCpp.run = wrap(ControlledCpp.run)
    ControlledPython.run = wrap(ControlledPython.run)
    replay_controlled.main()


if __name__ == '__main__':
    main()
