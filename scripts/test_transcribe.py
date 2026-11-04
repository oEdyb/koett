import importlib.machinery
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

loader = importlib.machinery.SourceFileLoader('cli', str(Path(__file__).with_name('koett-transcribe')))
spec = importlib.util.spec_from_loader(loader.name, loader)
cli = importlib.util.module_from_spec(spec)
loader.exec_module(cli)


class TranscribeTests(unittest.TestCase):
    def test_invalid_sources_and_embedded_credentials(self):
        for source in ('ftp://example.com/video', '--exec=touch /tmp/no', 'https://user:pass@example.com/v'):
            with self.assertRaises(ValueError):
                cli.source_identity(source)

    def test_arguments_preserve_url_and_bound_download(self):
        url = 'https://www.instagram.com/p/DWfipnmjYjo/?x=$(touch%20no)'
        with patch.object(cli, 'require', return_value='/bin/yt-dlp'):
            args = cli.download_args(url, Path('/tmp/with spaces'), '/bin/ffmpeg')
        self.assertEqual(args[-2:], ['--', url])
        for flag in ('--ignore-config', '--no-cookies-from-browser', '--no-playlist'):
            self.assertIn(flag, args)
        self.assertEqual(args[args.index('--playlist-end') + 1], '1')

    def test_cache_hit_never_downloads_or_loads_model(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(cli, 'CACHE', Path(temp)):
            url = 'https://example.com/video'
            record = {'source': url, 'model': 'Parakeet v3', 'text': 'Exact raw text.'}
            cli.cache_path(url, 'v3', True).write_text(json.dumps(record))
            with patch.object(cli, 'require', side_effect=AssertionError('cache needs no tools')):
                result = cli.transcribe(url, 'v3')
            self.assertEqual(result['text'], record['text'])
            self.assertTrue(result['cached'])
            self.assertNotEqual(cli.cache_path(url, 'v2', True), cli.cache_path(url, 'v3', True))

    def test_local_change_invalidates_cache(self):
        with tempfile.TemporaryDirectory() as temp:
            media = Path(temp) / 'media.wav'
            media.write_bytes(b'one')
            before = cli.cache_path(str(media), 'v3', False)
            media.write_bytes(b'different')
            self.assertNotEqual(before, cli.cache_path(str(media), 'v3', False))

    def test_engine_cache_identity_is_separate(self):
        with patch.object(cli, 'ENGINE', 'FluidAudio 0.15.6'):
            mac = cli.cache_path('https://example.com/v', 'v3', True)
        with patch.object(cli, 'ENGINE', 'sherpa-onnx 1.13.5'):
            linux = cli.cache_path('https://example.com/v', 'v3', True)
        self.assertNotEqual(mac, linux)

    def test_linux_file_pipeline_and_no_cache(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binary = root / 'CrossPlatform/target/release/koett-transcribe-engine'
            binary.parent.mkdir(parents=True)
            binary.touch()
            media = root / 'input.wav'
            media.touch()
            calls = []

            def fake_run(args):
                calls.append(args)
                if args[0] == binary:
                    Path(args[4]).write_text(json.dumps([{
                        'text': 'Raw words.', 'audioSeconds': 2,
                        'engineMilliseconds': 120,
                    }]))

            probe = subprocess.CompletedProcess([], 0, json.dumps({
                'streams': [{'codec_type': 'audio'}], 'format': {'duration': '2'}
            }))
            with patch.object(cli, 'REPO', root), patch.object(cli, 'LINUX', True), \
                    patch.object(cli, 'CACHE', root / 'cache/results'), \
                    patch.object(cli, 'require', side_effect=lambda name: name), \
                    patch.object(cli, 'run', side_effect=fake_run), \
                    patch.object(cli.subprocess, 'run', return_value=probe):
                result = cli.transcribe(str(media), 'v2', no_cache=True)
            self.assertEqual(result['text'], 'Raw words.')
            self.assertEqual(result['asr_seconds'], 0.12)
            self.assertFalse(result['cached'])
            self.assertEqual(calls[-1][1:3], ['--model-version', 'v2'])
            self.assertEqual(list((root / 'cache').iterdir()), [])

    def test_failed_process_and_timeout_are_errors(self):
        with self.assertRaises(RuntimeError):
            cli.run([sys.executable, '-c', 'raise SystemExit(7)'])
        with self.assertRaises(subprocess.TimeoutExpired):
            cli.run([sys.executable, '-c', 'import time; time.sleep(30)'], timeout=0.1)


if __name__ == '__main__':
    unittest.main()
