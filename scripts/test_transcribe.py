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

    def test_failed_process_and_timeout_are_errors(self):
        with self.assertRaises(RuntimeError):
            cli.run([sys.executable, '-c', 'raise SystemExit(7)'])
        with self.assertRaises(subprocess.TimeoutExpired):
            cli.run([sys.executable, '-c', 'import time; time.sleep(30)'], timeout=0.1)


if __name__ == '__main__':
    unittest.main()
