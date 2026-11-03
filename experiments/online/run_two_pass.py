#!/usr/bin/env python3
"""Replay entry point; the desktop and CLI use the same online worker."""
import _worker_path
from online_worker.worker import Events, main


if __name__ == '__main__':
    main()
