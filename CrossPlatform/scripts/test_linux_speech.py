#!/usr/bin/env python3
"""Private TLS Cartesia protocol and real PCM playback fixture; no provider calls."""
import asyncio
import atexit
import base64
import json
import math
import os
import ssl
import struct
import subprocess
import tempfile
import time
import uuid
from pathlib import Path

import websockets

ROOT = Path(__file__).resolve().parents[1]
NATIVE = bool(os.environ.get('KOETT_SPEECH_NATIVE'))
ERROR = bool(os.environ.get('KOETT_SPEECH_ERROR'))
TIMEOUT = os.environ.get('KOETT_SPEECH_TIMEOUT_MODE')
assert os.environ['XDG_RUNTIME_DIR'].endswith(('/agent-lab/koett-speech', '/agent-lab/koett-nemotron'))
assert os.environ.get('AGENT_LAB_GENERATION')
os.environ['PULSE_SINK'] = 'agent_lab_audio'
os.environ['PULSE_SOURCE'] = 'agent_lab_audio.monitor'


async def main():
    with tempfile.TemporaryDirectory(prefix='koett-speech-') as directory:
        temporary = Path(directory)
        key, cert = temporary / 'key.pem', temporary / 'cert.pem'
        def openssl(*args):
            subprocess.run(['openssl', *args], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        ca, ca_key = temporary / 'ca.pem', temporary / 'ca-key.pem'
        openssl('req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', str(ca_key), '-out', str(ca), '-days', '1', '-subj', '/CN=Koett Private Fixture CA')
        csr = temporary / 'leaf.csr'
        openssl('req', '-newkey', 'rsa:2048', '-nodes', '-keyout', str(key), '-out', str(csr), '-subj', '/CN=localhost')
        extensions = temporary / 'extensions'
        extensions.write_text('subjectAltName=DNS:localhost\nbasicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n')
        openssl('x509', '-req', '-in', str(csr), '-CA', str(ca), '-CAkey', str(ca_key), '-CAcreateserial', '-out', str(cert), '-days', '1', '-extfile', str(extensions))
        for kind in ['CONFIG', 'DATA', 'CACHE', 'STATE']:
            os.environ[f'XDG_{kind}_HOME'] = str(temporary / kind.lower())
        control = temporary / 'keyring-control'
        control.mkdir(mode=0o700)
        keyring = await asyncio.to_thread(subprocess.Popen, ['/usr/bin/gnome-keyring-daemon', '--foreground', '--unlock', '--components=secrets', '--control-directory=' + str(control)], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        keyring.stdin.write(os.urandom(32).hex().encode())
        keyring.stdin.close()
        atexit.register(lambda: keyring.terminate() if keyring.poll() is None else None)
        await asyncio.sleep(0.5)
        tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        tls.load_cert_chain(cert, key)
        received = []
        transcripts = []
        connected = []
        disconnected = []

        async def handle_request(socket):
            connected.append(time.monotonic())
            assert socket.request.headers['X-API-Key'] == 'private-fixture-only'
            assert socket.request.headers['Cartesia-Version'] == '2026-03-01'
            context = None
            async for raw in socket:
                event = json.loads(raw)
                received.append(time.monotonic())
                transcripts.append(event['transcript'])
                assert '```' not in event['transcript'] and 'https://' not in event['transcript']
                assert 'let answer' not in event['transcript']
                assert event['model_id'] == 'sonic-3.5-2026-05-04'
                assert event['voice'] == {'mode': 'id', 'id': 'f786b574-daa5-4673-aa0c-cbe3e8534c02'}
                assert event['output_format'] == {'container': 'raw', 'encoding': 'pcm_f32le', 'sample_rate': 44100}
                assert context is None or context == event['context_id']
                context = event['context_id']
                assert str(uuid.UUID(context)) == context
                if TIMEOUT == 'first_audio':
                    continue
                if TIMEOUT == 'stalled_read' and not event['transcript']:
                    continue
                if ERROR:
                    await socket.send(json.dumps({'type': 'error', 'context_id': context, 'message': 'Private provider failure'}))
                    return
                if event['transcript']:
                    pcm = b''.join(struct.pack('<f', 0.15 * math.sin(2 * math.pi * 440 * n / 44100)) for n in range(4410))
                    for _ in range(1 if TIMEOUT == "stalled_read" else 10):
                        await socket.send(json.dumps({'type': 'chunk', 'context_id': context, 'data': base64.b64encode(pcm).decode()}))
                if not event['continue']:
                    await socket.send(json.dumps({'type': 'done', 'context_id': context}))

        async def handle(socket):
            try:
                await handle_request(socket)
            finally:
                disconnected.append(time.monotonic())

        alsa = temporary / 'alsa.conf'
        alsa.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "agent_lab_audio" }\nctl.!default { type pulse }\n')
        async def stalled_handshake(reader, writer):
            connected.append(time.monotonic())
            try:
                while await reader.read(4096):
                    pass
            finally:
                writer.close()
                await writer.wait_closed()
                disconnected.append(time.monotonic())
        server_context = await asyncio.start_server(stalled_handshake, '127.0.0.1', 0) if TIMEOUT == 'handshake' else websockets.serve(handle, '127.0.0.1', 0, ssl=tls)
        async with server_context as server:
            port = server.sockets[0].getsockname()[1]
            env = dict(os.environ, RUSTUP_TOOLCHAIN="stable", SSL_CERT_FILE=str(ca), ALSA_CONFIG_PATH=str(alsa),
                       KOETT_SPEECH_FIXTURE_URL=f'wss://localhost:{port}',
                       SHERPA_ONNX_LIB_DIR='/tmp/koett-native-host-abi0/lib', KOETT_SPEECH_MARKERS=str(temporary))
            capture = await asyncio.to_thread(subprocess.Popen, ['parec', '--device=agent_lab_audio.monitor', '--format=float32le', '--rate=44100', '--channels=1'], stdout=(temporary / 'monitor.pcm').open('wb'))
            try:
                process = await asyncio.create_subprocess_exec('/home/olle/.cargo/bin/cargo', 'test', '--locked', 'native_spoken_answer' if NATIVE else 'private_wss_playback', '--', '--ignored', '--nocapture',
                    cwd=ROOT, env=env, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.STDOUT)
                async def captures():
                    if not NATIVE:
                        return
                    evidence = Path(os.environ.get('KOETT_SPEECH_EVIDENCE', str(ROOT / 'evidence/parity-speech-20261030')))
                    evidence.mkdir(parents=True, exist_ok=True)
                    for state in ['error' if ERROR else 'playing', 'cancelled']:
                        for _ in range(1200):
                            if (temporary / state).exists():
                                break
                            await asyncio.sleep(0.1)
                        else:
                            raise AssertionError('Native state not reached: ' + state)
                        await asyncio.sleep(0.15)
                        shot = await asyncio.create_subprocess_exec('python3', str(Path.home() / '.local/share/agent-lab/screenshot.py'), str(evidence / ('ask-' + state + ('-error-run' if ERROR else '') + '.png')))
                        assert await shot.wait() == 0
                        if state != "cancelled":
                            key_event = await asyncio.create_subprocess_exec(str(Path.home() / ".local/share/agent-lab/input"), "key", "15")
                            assert await key_event.wait() == 0
                capture_task = asyncio.create_task(captures())
                output, _ = await process.communicate()
                if process.returncode:
                    capture_task.cancel()
                else:
                    await capture_task
                print(output.decode(), flush=True)
                assert process.returncode == 0, output.decode()
                await asyncio.sleep(2)
            finally:
                capture.terminate()
                capture.wait(timeout=5)
            assert len(connected) == (2 if NATIVE else 1)
            assert len(disconnected) == len(connected)
            assert (1 <= len(received) <= 3) if ERROR else len(received) == (0 if TIMEOUT == "handshake" else 3 if NATIVE else 2)
            if received:
                assert received[0] - connected[0] > 0.2, 'prewarm must precede text'
            raw = (temporary / 'monitor.pcm').read_bytes()
            samples = [v[0] for v in struct.iter_unpack('<f', raw)]
            assert samples and (max(map(abs, samples)) < 0.001 if ERROR or TIMEOUT in {"handshake", "first_audio"} else max(map(abs, samples)) > 0.05), ('no actual private PCM signal', len(samples), max(map(abs, samples), default=0))
            assert max(map(abs, samples[-4410:])) < 0.001, 'output failed to become silent'
            print(json.dumps({'mock_wss': True, 'prewarm_before_text': True if received else None, 'pcm_peak': max(map(abs, samples)), 'silent_tail': True, 'live_cartesia': False, 'native_panel_copy': NATIVE, 'provider_error': ERROR, 'timeout_mode': TIMEOUT, 'connections_closed': len(disconnected), 'cancelled_wait': bool(os.environ.get('KOETT_SPEECH_CANCEL'))}))


asyncio.run(main())
