#!/usr/bin/env python3
"""Sign a notarized release ZIP and appcast; never upload or publish."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import plistlib
import re
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile

FEED = 'https://github.com/oEdyb/koett/releases/latest/download/appcast.xml'
RELEASES = 'https://github.com/oEdyb/koett/releases/download/'
SPARKLE = 'http://www.andymatuschak.org/xml-namespaces/sparkle'
ET.register_namespace('sparkle', SPARKLE)


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def metadata(archive):
    with zipfile.ZipFile(archive) as zipped:
        entries = [name for name in zipped.namelist() if name.endswith('.app/Contents/Info.plist') and name.count('/') == 2]
        if entries != ['Koett.app/Contents/Info.plist']:
            raise ValueError('Expected exactly one root Koett.app')
        info = plistlib.loads(zipped.read(entries[0]))
    if info.get('CFBundleIdentifier') != 'com.olledyberg.Koett' or 'KoettLocalUpdateTesting' in info:
        raise ValueError('Only a production Koett bundle can have a release appcast')
    if info.get('SUFeedURL') != FEED or not info.get('SURequireSignedFeed') or not info.get('SUVerifyUpdateBeforeExtraction') or info.get('SUSignedFeedFailureExpirationInterval') != 0:
        raise ValueError('Expected the approved signed-feed configuration')
    version, build = info.get('CFBundleShortVersionString', ''), info.get('CFBundleVersion', '')
    if not re.fullmatch(r'\d+\.\d+\.\d+', version) or not re.fullmatch(r'\d+(?:\.\d+)*', build):
        raise ValueError('Invalid release version/build')
    if archive.name != f'Koett-{version}-macOS-arm64.zip':
        raise ValueError('Archive must use its immutable versioned asset name')
    if not re.fullmatch(r'[0-9a-f]{40}', info.get('KoettSourceCommit', '')):
        raise ValueError('A full clean source stamp is required')
    production = plistlib.loads((Path(__file__).resolve().parent.parent / 'Packaging/Info.plist').read_bytes())
    if info.get('SUPublicEDKey') != production['SUPublicEDKey']:
        raise ValueError('The archive must use the existing production update key')
    if len(base64.b64decode(info.get('SUPublicEDKey', ''), validate=True)) != 32:
        raise ValueError('Invalid update public key')
    return info


def unsigned_feed(info, archive, signature, length):
    if len(base64.b64decode(signature, validate=True)) != 64 or length != archive.stat().st_size:
        raise ValueError('Signer signature/length is invalid')
    rss = ET.Element('rss', {'version': '2.0'})
    channel = ET.SubElement(rss, 'channel')
    ET.SubElement(channel, 'title').text = 'Koett updates'
    ET.SubElement(channel, 'link').text = 'https://github.com/oEdyb/koett/releases'
    item = ET.SubElement(channel, 'item')
    ET.SubElement(item, 'title').text = 'Koett ' + info['CFBundleShortVersionString']
    ET.SubElement(item, '{' + SPARKLE + '}version').text = info['CFBundleVersion']
    ET.SubElement(item, '{' + SPARKLE + '}shortVersionString').text = info['CFBundleShortVersionString']
    ET.SubElement(item, '{' + SPARKLE + '}minimumSystemVersion').text = info['LSMinimumSystemVersion']
    ET.SubElement(item, 'enclosure', {'url': RELEASES + 'v' + info['CFBundleShortVersionString'] + '/' + archive.name,
        'type': 'application/octet-stream', 'length': str(length), '{' + SPARKLE + '}edSignature': signature})
    return ET.tostring(rss, encoding='utf-8', xml_declaration=True) + b'\n'


def create(archive, output, signer):
    if output.name != 'appcast.xml' or any(p.exists() for p in [output, output.with_suffix('.xml.sha256'), output.with_suffix('.receipt.json')]):
        raise ValueError('Choose a fresh appcast.xml output path')
    info = metadata(archive)
    before = sha256(archive)
    result = subprocess.check_output([str(signer), '--sign', str(archive)], text=True).strip()
    attrs = ET.fromstring('<enclosure xmlns:sparkle="' + SPARKLE + '" ' + result + '/>').attrib
    signature, length = attrs['{' + SPARKLE + '}edSignature'], int(attrs['length'])
    content = unsigned_feed(info, archive, signature, length)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent, prefix='.appcast-') as temp:
        feed = Path(temp) / 'appcast.xml'
        feed.write_bytes(content)
        subprocess.run([str(signer), '--sign-feed', str(feed)], check=True)
        verifier = Path(__file__).with_name('verify-macos-update.swift')
        subprocess.run(['swift', str(verifier), info['SUPublicEDKey'], str(archive), str(feed)], check=True)
        if sha256(archive) != before:
            raise ValueError('Archive changed during signing')
        feed.rename(output)
    receipt = {'sourceCommit': info['KoettSourceCommit'], 'version': info['CFBundleShortVersionString'],
        'build': info['CFBundleVersion'], 'stableFeedURL': FEED,
        'archiveURL': RELEASES + 'v' + info['CFBundleShortVersionString'] + '/' + archive.name,
        'archive': archive.name, 'archiveSHA256': before, 'archiveBytes': length,
        'archiveEdSignature': signature, 'feed': output.name, 'feedSHA256': sha256(output),
        'signaturesVerified': True, 'published': False}
    output.with_suffix('.xml.sha256').write_text(f"{receipt['feedSHA256']}  {output.name}\n")
    output.with_suffix('.receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    return receipt


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--signer', required=True, type=Path)
    args = parser.parse_args()
    print(json.dumps(create(args.archive.resolve(), args.output.absolute(), args.signer.resolve()), indent=2))
