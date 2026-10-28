"""Regression for a restarted WebContent reusing a sampled PID/command."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    'sample_editor_rss', Path(__file__).with_name('sample-editor-rss.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
parse_processes = module.parse_processes
changed_processes = module.changed_processes


class ProcessIdentityTests(unittest.TestCase):
    def test_same_command_with_reused_pid_is_not_a_memory_drop(self):
        before = parse_processes('42 100 Tue Oct 27 10:00:00 2026 /System/WebKit.WebContent\n')
        restarted = parse_processes('42 10 Tue Oct 27 10:01:00 2026 /System/WebKit.WebContent\n')
        self.assertEqual(changed_processes(before, restarted), [42])
        same = parse_processes('42 90 Tue Oct 27 10:00:00 2026 /System/WebKit.WebContent\n')
        self.assertEqual(changed_processes(before, same), [])

    def test_spaced_path_and_date_padding_keep_identity(self):
        rows = parse_processes('  43  120 Tue Oct  6 10:00:00 2026 /tmp/Atogaki QA.app/Contents/MacOS/atogaki-desktop\n\n')
        self.assertEqual(rows[43]['command'], '/tmp/Atogaki QA.app/Contents/MacOS/atogaki-desktop')
        self.assertEqual(rows[43]['started_at'], 'Tue Oct 6 10:00:00 2026')
        self.assertEqual(rows[43]['rss_kib'], 120)


if __name__ == '__main__':
    unittest.main()
