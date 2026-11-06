#!/usr/bin/env python3
"""Package an already notarized app without changing its signature or ownership."""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import subprocess
import tempfile


TEAM = 'LF8KF3G42Q'


def run(*args):
    result = subprocess.run([str(arg) for arg in args], check=True, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return result.stdout


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def validate_app(app):
    info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
    if info.get('CFBundleIdentifier') != 'com.olledyberg.Koett':
        raise ValueError('Expected com.olledyberg.Koett')
    if 'KoettLocalUpdateTesting' in info:
        raise ValueError('Lab update builds cannot be packaged for distribution')
    run('codesign', '--verify', '--deep', '--strict', app)
    details = run('codesign', '-d', '--verbose=4', app)
    if f'TeamIdentifier={TEAM}' not in details or 'Authority=Developer ID Application:' not in details:
        raise ValueError('The app requires the Koett Developer ID Application signature')
    run('xcrun', 'stapler', 'validate', app)
    assessment = run('spctl', '--assess', '--type', 'execute', '--verbose=2', app)
    if 'Notarized Developer ID' not in assessment:
        raise ValueError('The app must be notarized and stapled before packaging')
    return info


def tree_manifest(app):
    result = {}
    for path in sorted(app.rglob('*')):
        name = str(path.relative_to(app))
        mode = path.lstat().st_mode & 0o7777
        if path.is_symlink():
            result[name] = {'mode': mode, 'link': str(path.readlink())}
        elif path.is_file():
            result[name] = {'mode': mode, 'sha256': sha256(path)}
        else:
            result[name] = {'mode': mode}
    return result


def package(app, output, identity, profile=None):
    app, output = app.resolve(), output.absolute()
    if output.suffix != '.dmg' or output.exists():
        raise ValueError('Choose a new .dmg output path; existing artifacts are never overwritten')
    info = validate_app(app)
    before = tree_manifest(app)
    output.parent.mkdir(parents=True, exist_ok=True)
    receipt = {'sourceCommit': info.get('KoettSourceCommit'),
               'version': info['CFBundleShortVersionString'], 'build': info['CFBundleVersion'],
               'inputApp': str(app), 'notarized': False, 'distributable': False}
    layout = Path(__file__).resolve().parent.parent / 'Packaging/DMG/FinderLayout.dsstore'
    with tempfile.TemporaryDirectory(prefix='koett-dmg-') as directory:
        stage = Path(directory) / 'image'
        stage.mkdir()
        run('ditto', app, stage / 'Koett.app')
        if tree_manifest(stage / 'Koett.app') != before:
            raise ValueError('Staging changed app content, symlinks or permissions')
        (stage / 'Applications').symlink_to('/Applications')
        run('ditto', layout, stage / '.DS_Store')
        run('hdiutil', 'create', '-srcfolder', stage, '-volname', 'Koett', '-fs', 'HFS+',
            '-format', 'UDZO', '-srcowners', 'off', '-anyowners', output)
    run('codesign', '--force', '--timestamp', '--sign', identity,
        '--identifier', 'com.olledyberg.Koett.dmg', output)
    run('codesign', '--verify', '--strict', output)
    details = run('codesign', '-d', '--verbose=4', output)
    if f'TeamIdentifier={TEAM}' not in details or 'Authority=Developer ID Application:' not in details:
        raise ValueError('The disk image requires the Koett Developer ID Application signature')
    run('hdiutil', 'verify', output)
    if profile:
        submission = json.loads(run('xcrun', 'notarytool', 'submit', output,
                                    '--keychain-profile', profile, '--wait', '--output-format', 'json'))
        receipt['notary'] = submission
        output.with_suffix('.notary.json').write_text(json.dumps(submission, indent=2) + '\n')
        if submission.get('status') != 'Accepted':
            raise ValueError('Apple did not accept this DMG; see the notary receipt')
        run('xcrun', 'stapler', 'staple', output)
        run('xcrun', 'stapler', 'validate', output)
        run('codesign', '--verify', '--strict', output)
        receipt['gatekeeper'] = run('spctl', '--assess', '--type', 'open', '--verbose=2',
                                    '--context', 'context:primary-signature', output)
        receipt['notarized'] = True
        receipt['distributable'] = True
    if tree_manifest(app) != before:
        raise ValueError('Input app changed during packaging')
    receipt['sha256'] = sha256(output)
    receipt['executableUUID'] = run('dwarfdump', '--uuid', app / 'Contents/MacOS/koett').strip()
    output.with_suffix('.dmg.sha256').write_text(f"{receipt['sha256']}  {output.name}\n")
    output.with_suffix('.receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--identity', required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--notary-profile', help='Submit only to Apple using an existing Keychain profile')
    mode.add_argument('--prepare-only', action='store_true', help='Signed local candidate; NOT distributable')
    args = parser.parse_args()
    print(json.dumps(package(args.app, args.output, args.identity, args.notary_profile), indent=2))


if __name__ == '__main__':
    main()
