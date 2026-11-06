import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class SparkleEmbeddingTests(unittest.TestCase):
    def test_nested_code_is_signed_before_its_container(self):
        for identity in ['Developer ID test identity', '-']:
            with self.subTest(identity=identity), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / 'scripts').mkdir()
                script = root / 'scripts/embed-sparkle.sh'
                shutil.copyfile(Path(__file__).with_name('embed-sparkle.sh'), script)
                artifact = root / '.build/artifacts/sparkle/Sparkle'
                framework = artifact / 'Sparkle.xcframework/macos-arm64_x86_64/Sparkle.framework'
                framework.mkdir(parents=True)
                (artifact / 'LICENSE').write_text('Complete upstream license fixture')
                fake = root / 'bin'
                fake.mkdir()
                (fake / 'ditto').write_text('#!/usr/bin/env python3\nimport shutil,sys\nshutil.copytree(sys.argv[1],sys.argv[2])\n')
                (fake / 'codesign').write_text('#!/usr/bin/env python3\nimport json,os,sys\nwith open(os.environ["SIGN_LOG"],"a") as log: log.write(json.dumps(sys.argv[1:])+"\\n")\n')
                for tool in fake.iterdir():
                    tool.chmod(0o755)
                log = root / 'signatures.jsonl'
                app = root / 'Koett.app'
                subprocess.run(['zsh', str(script), str(app), identity], check=True,
                               env=dict(os.environ, PATH=str(fake) + os.pathsep + os.environ['PATH'], SIGN_LOG=str(log)))
                calls = [json.loads(line) for line in log.read_text().splitlines()]
                embedded = app / 'Contents/Frameworks/Sparkle.framework'
                required = ['Versions/B/XPCServices/Installer.xpc', 'Versions/B/XPCServices/Downloader.xpc',
                            'Versions/B/Autoupdate', 'Versions/B/Updater.app']
                self.assertEqual([call[-1] for call in calls[:-1]],
                                 [str(embedded / name) for name in required] + [str(embedded)])
                for call in calls[:-1]:
                    self.assertNotIn('--deep', call)
                    self.assertEqual('--timestamp' in call, identity != '-')
                    self.assertIn('runtime', call)
                self.assertIn('--preserve-metadata=entitlements', calls[1])
                self.assertIn('--verify', calls[-1])
                self.assertEqual((app / 'Contents/Resources/Sparkle-LICENSE.txt').read_bytes(),
                                 (artifact / 'LICENSE').read_bytes())


if __name__ == '__main__':
    unittest.main()
