import unittest
import hashlib
from pathlib import Path
import shutil
import subprocess
import tempfile

from burned_reference import classify, extract, merge_frames


def line(text='確認する', x=.2, y=.5, ink=30):
    return dict(text=text, box=[x, y, .25, .04], confidence=.7, ink_luma=ink)


def frame(time, lines, source='atogaki-vol01', status='completed'):
    return dict(case_id='case', source_id=source, media_time_s=time,
                frame_id=str(time), image_path='frame.png', lines=lines, status=status)


class BurnedReferenceTests(unittest.TestCase):
    def test_page_history_is_not_current_even_when_dark(self):
        rows = classify(frame(1, [line('过去的内容', ink=80), line('当前内容', y=.7, ink=30)],
                              source='atogaki-vol35'))
        self.assertEqual([state for _, state in rows], ['context_only', 'current_candidate'])

    def test_faded_future_japanese_is_not_promoted(self):
        units = merge_frames([frame(1, [line('まだ話していない', ink=175), line('今の発言')])])
        self.assertEqual([u['text'] for u in units], ['今の発言'])

    def test_identical_speech_after_a_gap_remains_separate(self):
        units = merge_frames([frame(1, [line()]), frame(1.5, []), frame(2, [line()])])
        self.assertEqual(len(units), 2)
        self.assertEqual(units[0]['disappearance_bracket_s'], [1, 1.5])
        self.assertEqual(units[1]['appearance_bracket_s'], [1.5, 2])

    def test_same_words_on_two_sides_do_not_collapse(self):
        units = merge_frames([frame(t, [line(x=.2), line(x=.65)]) for t in [1, 1.5]])
        self.assertEqual(len(units), 2)
        self.assertTrue(all(u['observations'] == 2 for u in units))

    def test_nearby_identical_rows_have_one_to_one_matches(self):
        units = merge_frames([frame(1, [line(y=.5), line(y=.52)]),
                              frame(1.5, [line(y=.52), line(y=.5)])])
        self.assertEqual(len(units), 2)
        self.assertTrue(all(u['observations'] == 2 for u in units))

    def test_negation_change_is_not_fuzzy_merged(self):
        units = merge_frames([frame(1, [line('これは必要です')]),
                              frame(1.5, [line('これは必要ではないです')])])
        self.assertEqual(len(units), 2)

    def test_roman_proper_names_survive_but_speaker_badges_do_not(self):
        units = merge_frames([frame(1, [line('BUMP OF CHICKEN'), line('suis', x=.92)])])
        self.assertEqual([u['text'] for u in units], ['BUMP OF CHICKEN'])

    def test_failed_frame_does_not_invent_a_known_boundary(self):
        units = merge_frames([frame(1, [line()]), frame(1.5, [], status='failed'),
                              frame(2, [line()])])
        self.assertEqual(len(units), 2)
        self.assertEqual(units[0]['disappearance_bracket_s'], [1, None])
        self.assertEqual(units[1]['appearance_bracket_s'], [None, 2])

    def test_first_and_last_visibility_are_open_ended(self):
        unit = merge_frames([frame(10, [line()])])[0]
        self.assertEqual(unit['appearance_bracket_s'], [None, 10])
        self.assertEqual(unit['disappearance_bracket_s'], [10, None])
        self.assertEqual(unit['timing_status'], 'sampled_display_interval_not_acoustic')

    @unittest.skipUnless(shutil.which('ffmpeg'), 'ffmpeg unavailable')
    def test_extraction_does_not_record_the_muxer_discarded_tail_frame(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            media = root / 'sample.mp4'
            subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i',
                            'color=white:s=64x64:r=25:d=2', '-c:v', 'libx264', str(media)],
                           check=True)
            case = dict(id='test', source_id='atogaki-vol01', media_path=str(media),
                        media_sha256=hashlib.sha256(media.read_bytes()).hexdigest(),
                        start_ms=0, end_ms=1000)
            output = root / 'out'
            output.mkdir()
            jobs = extract(dict(cases=[case]), output, .5)
            self.assertEqual(len(jobs), 2)
            self.assertTrue(all(0 <= j['media_time_s'] < 1 for j in jobs))
            self.assertTrue(all(Path(j['image_path']).exists() for j in jobs))

    @unittest.skipUnless(shutil.which('ffmpeg'), 'ffmpeg unavailable')
    def test_nonzero_video_start_uses_absolute_trim_end(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            media = root / 'offset.mp4'
            subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i',
                            'color=white:s=64x64:r=25:d=2', '-f', 'lavfi', '-i',
                            'anullsrc=r=16000:cl=mono', '-vf', 'setpts=PTS+0.12/TB',
                            '-t', '2', '-c:v', 'libx264', '-c:a', 'aac', str(media)], check=True)
            case = dict(id='test', source_id='atogaki-vol02', media_path=str(media),
                        media_sha256=hashlib.sha256(media.read_bytes()).hexdigest(),
                        start_ms=0, end_ms=1050)
            output = root / 'out'
            output.mkdir()
            jobs = extract(dict(cases=[case]), output, .5)
            self.assertEqual(len(jobs), 2)
            self.assertTrue(all(0 <= j['media_time_s'] < 1.05 for j in jobs))

    @unittest.skipUnless(shutil.which('ffmpeg'), 'ffmpeg unavailable')
    def test_fractional_tail_pts_is_not_rounded_away_by_output_t(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            media = root / 'fractional.mp4'
            subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i',
                            'color=white:s=64x64:r=25:d=2', '-f', 'lavfi', '-i',
                            'anullsrc=r=16000:cl=mono', '-vf', 'setpts=PTS+0.0795/TB',
                            '-t', '2', '-c:v', 'libx264', '-video_track_timescale', '16000',
                            '-c:a', 'aac', str(media)], check=True)
            case = dict(id='test', source_id='atogaki-vol03', media_path=str(media),
                        media_sha256=hashlib.sha256(media.read_bytes()).hexdigest(),
                        start_ms=0, end_ms=1120)
            output = root / 'out'
            output.mkdir()
            jobs = extract(dict(cases=[case]), output, .5)
            self.assertTrue(all(0 <= j['media_time_s'] < 1.12 for j in jobs))
            self.assertEqual(len(jobs), len(list((output / 'frames/test').glob('*.png'))))


if __name__ == '__main__':
    unittest.main()
