"""Research import bridge to the shared online runtime implementation."""
import sys
import _worker_path
from online_worker import qwen_prefix_cache as _implementation

sys.modules[__name__] = _implementation
