"""Native Copy assertions for the private Wayland UI harness."""
import csv
import hashlib
import io
import os
import subprocess
import time
from pathlib import Path

from PIL import Image


def stop_clipboard_owners():
    runtime = os.environ['AGENT_LAB_RUNTIME'].encode()
    assert runtime == os.environ['XDG_RUNTIME_DIR'].encode()
    assert runtime.startswith(f'/run/user/{os.getuid()}/agent-lab/'.encode())
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            if ((entry / 'comm').read_text().strip() == 'wl-copy'
                    and b'AGENT_LAB_RUNTIME=' + runtime + b'\0' in (entry / 'environ').read_bytes()):
                os.kill(int(entry.name), 15)
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            pass


def native_copy(ns, expected, name='Copy', method='action'):
    main, wait, find, atspi = (ns[k] for k in ['main', 'wait', 'find', 'Atspi'])
    node = wait(lambda: find(name, main), name)
    sentinel = f'private native Copy sentinel: {method} {time.monotonic_ns()}'.encode()
    subprocess.run(['wl-copy'], input=sentinel, check=True)
    assert subprocess.check_output(['wl-paste', '--no-newline'], timeout=3) == sentinel
    # Restore app focus and obtain a real input serial after replacing clipboard ownership.
    ns['control']('show')
    ns['resize']('Koett', 960, 680)
    def active():
        main.clear_cache()
        return main.get_state_set().contains(atspi.StateType.ACTIVE)
    wait(active, 'private History active')
    ns['keys']('chord', 29, 33)
    wait(lambda: find('Search transcripts', main, 'text').get_state_set().contains(atspi.StateType.FOCUSED),
         'History consumed private focus key')
    if method == 'action':
        assert node.get_action_iface().do_action(0)
    elif method == 'keyboard':
        # Forward Tab traverses list rows and changes the selected transcript.
        for _ in range(40):
            ns['keys']('chord', 42, 15)
            time.sleep(0.12)
            node.clear_cache()
            if node.get_state_set().contains(atspi.StateType.FOCUSED):
                break
        assert node.get_state_set().contains(atspi.StateType.FOCUSED)
        assert ns['transcript']() == expected
        ns['keys']('key', 57)
    elif method == 'pointer':
        assert name == 'Copy'
        capture_name = 'pointer-target-' + str(time.monotonic_ns())
        ns['capture'](capture_name)
        screenshot = ns['output'] / (capture_name + '.png')
        rgb = screenshot.with_suffix('.rgb.png')
        with Image.open(screenshot) as frame:
            frame.convert('RGB').save(rgb)
        targets = {}
        with Image.open(rgb) as frame:
            for top in range(0, frame.height, 180):
                for left in range(0, frame.width, 320):
                    tile = rgb.with_suffix('.tile.png')
                    frame.crop((left, top, min(left + 640, frame.width),
                                min(top + 360, frame.height))).save(tile)
                    tsv = subprocess.check_output(
                        ['tesseract', str(tile), 'stdout', '--psm', '11', 'tsv'],
                        text=True, stderr=subprocess.DEVNULL)
                    for row in csv.DictReader(io.StringIO(tsv), delimiter='\t'):
                        if row['text'] == 'Copy' and float(row['conf']) >= 70:
                            x = left + int(row['left']) + int(row['width']) // 2
                            y = top + int(row['top']) + int(row['height']) // 2
                            targets[(round(x / 8), round(y / 8))] = (x, y)
                    tile.unlink()
        assert len(targets) == 1, targets
        target = next(iter(targets.values()))
        # A stationary compositor cursor can retain pre-resize surface coordinates.
        ns['keys']('move', 0, 0)
        ns['keys']('move', *target)
        ns['keys']('click')
    else:
        raise ValueError(method)
    if name in ['Copy', 'Copy transcript']:
        wait(lambda: find('Copied', main), 'native button acknowledged ' + method)
    expected = expected.encode()
    try:
        wait(lambda: subprocess.check_output(['wl-paste', '--no-newline'], timeout=3) == expected,
             f'exact native {name} via {method}')
    except AssertionError:
        ns['capture']('native-copy-failure-' + str(time.monotonic_ns()))
        raise
    assert expected != sentinel
    print(f'Native {name} {method}: {len(expected)} bytes sha256={hashlib.sha256(expected).hexdigest()}', flush=True)
    if name == 'Copy':
        wait(lambda: find('Copy', main), 'Copy label reset')
