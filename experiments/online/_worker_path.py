"""Locate the shared source worker when running research entry points."""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "src" / "infrastructure"))
