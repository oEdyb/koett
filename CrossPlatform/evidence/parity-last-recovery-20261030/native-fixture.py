# ruff: noqa: F821
"""Reuse the native model harness setup and helpers in an isolated Last/recovery run."""
import sys
from pathlib import Path

source = Path(__file__).resolve().parents[2] / 'scripts/test_linux_models.py'
sys.path.insert(0, str(source.parent))
globals()['__file__'] = str(source)
exec(compile(source.read_text().split('process = None\nchecks = []\ntry:')[0], str(source), 'exec'))  # noqa: S102 — trusted repository harness helpers
(output / 'config/mimeapps.list').write_text('[Default Applications]\ninode/directory=org.kde.dolphin.desktop\n')
last = database.parent / 'Last Transcript.txt'
exact = '  Åäö 世界 🦀\n\nPrivate Last fixture with trailing space '
last.write_text(exact)
last.chmod(0o600)
settings = json.loads(config.read_text())
settings['microphone'] = 'alsa:nonexistent-private-fixture-device'
config.write_text(json.dumps(settings))
process = None

def cli(command):
    result = subprocess.run([str(binary), command], capture_output=True, text=True, timeout=20)
    value = json.loads(result.stdout)
    assert (result.returncode == 0) == bool(value["ok"]), (command, result.returncode, value)
    return value

def clipboard():
    return subprocess.check_output(['wl-paste', '--no-newline']).decode()

def start():
    global process, main
    process = subprocess.Popen([str(binary)], stdout=(output / 'app.log').open('a'), stderr=subprocess.STDOUT)
    main = wait(lambda: next((n for n in walk(Atspi.get_desktop(0)) if n.get_name() == 'Koett' and n.get_role_name() == 'frame' and n.get_application().get_process_id() == process.pid), None), 'main')
    wait(lambda: find('Ready', main), 'ready', timeout=180)
    wait(lambda: cli('recovery-status')['last_available'], 'last available')

def quit_app():
    assert cli('quit')['ok']
    process.wait(timeout=30)

def dictate():
    click('Record', main)
    wait(lambda: find('Stop', main), 'recording')
    subprocess.run(['pw-play', '--target', 'agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
    click('Stop', main)
    wait(lambda: find('Ready', main), 'completed', timeout=180)

try:
    start()
    assert cli('copy-last')['ok']
    assert clipboard() == exact
    paste = cli('paste-last')
    assert not paste['ok'] and 'Copied' in paste['error']['message']
    assert clipboard() == exact
    capture('restart-copy-and-paste-guard')
    quit_app()
    start()
    assert cli('copy-last')['ok'] and clipboard() == exact
    # Real CPAL capture from only the private monitor, with missing stored identity.
    click('Record', main)
    wait(lambda: find('Stop', main), 'recording')
    capture('missing-microphone-warning-during-capture')
    assert any('unavailable' in n.get_name().lower() or 'default microphone' in n.get_name().lower() for n in walk(main))
    subprocess.run(['pw-play', '--target', 'agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
    click('Stop', main)
    wait(lambda: find('Ready', main), 'transcribed', timeout=180)
    assert json.loads(config.read_text())['microphone'] != 'alsa:nonexistent-private-fixture-device'
    with sqlite3.connect(database) as connection:
        rows = connection.execute('SELECT text FROM transcripts WHERE id NOT LIKE "visual-%"').fetchall()
    assert len(rows) == 1 and rows[0][0]
    assert last.read_text() == rows[0][0]
    assert last.stat().st_mode & 0o777 == 0o600
    quit_app()
    start()
    assert cli('copy-last')['ok'] and clipboard() == rows[0][0]
    # Make both real storage destinations unavailable, preserving previous private DB.
    database.rename(database.with_suffix('.saved'))
    database.mkdir()
    fallback = database.parent / 'Failed Transcripts'
    if fallback.exists():
        fallback.rename(database.parent / 'Failed Transcripts.saved')
    fallback.write_text('blocker')
    dictate()
    wait(lambda: cli('recovery-status')['unsaved_count'] == 1, 'first unsaved')
    dictate()
    wait(lambda: cli('recovery-status')['unsaved_count'] == 2, 'second unsaved')
    assert cli('copy-unsaved')['ok']
    copied = clipboard()
    click('Copy Unsaved', main)
    wait(lambda: clipboard() == copied, 'native Copy Unsaved exact')
    assert copied and '\n\n' in copied
    capture('save-failure-actions')
    assert find('Copy Unsaved', main) and find('Retry Save', main)
    assert not cli('retry-unsaved')['ok']
    assert not cli('quit')['ok']
    wait(lambda: find('Quit Anyway'), 'quit protection')
    capture('quit-protection-two-unsaved')
    click('Keep open')
    assert process.poll() is None
    click('Menu', main)
    capture('recovery-native-menu')
    keys('key', 1)
    resize('Koett', 500, 680)
    keys('chord', 29, 16)
    wait(lambda: find('Quit Anyway'), 'narrow quit protection')
    capture('quit-protection-narrow-keyboard')
    click('Keep open')
    database.rmdir()
    database.with_suffix('.saved').rename(database)
    fallback.unlink()
    click('Retry Save', main)
    wait(lambda: cli('recovery-status')['unsaved_count'] == 0, 'native Retry Save')
    assert cli('retry-unsaved')['ok']
    assert cli('recovery-status')['unsaved_count'] == 0
    with sqlite3.connect(database) as connection:
        saved = connection.execute('SELECT id,text FROM transcripts WHERE id NOT LIKE "visual-%"').fetchall()
    assert len(saved) == 3 and len({r[0] for r in saved}) == 3
    assert copied == '\n\n'.join(r[1] for r in saved[1:])
    assert not list((output / 'data').rglob('*.wav'))
    capture('retry-saved')
    for command, folder in [('media-transcripts', 'Media Transcripts'), ('failed-transcripts', 'Failed Transcripts'), ('failed-recordings', 'Failed Recordings')]:
        assert cli(command)['ok']
        assert (database.parent / folder).is_dir()
    wait(lambda: find('Media Transcripts', role='frame') or any('Dolphin' in n.get_name() for n in walk(Atspi.get_desktop(0))), 'private folder window')
    capture('private-folders-open')
    kwin('for (const w of workspace.windowList()) if (String(w.resourceClass).includes("dolphin")) w.closeWindow();')
    database.rename(database.with_suffix('.discard-fixture'))
    database.mkdir()
    fallback.rmdir()
    fallback.write_text('explicit private discard fixture')
    dictate()
    wait(lambda: cli('recovery-status')['unsaved_count'] == 1, 'explicit discard record')
    assert not cli('quit')['ok']
    wait(lambda: find('Quit Anyway'), 'explicit discard confirmation')
    click('Quit Anyway')
    process.wait(timeout=30)
    (output / 'checks.json').write_text(json.dumps({'restart_copy_exact': True, 'paste_focus_guard': True, 'two_unsaved_copy_retry_quit': True, 'sqlite_once': True, 'missing_mic_capture_warning': True, 'no_success_wav': True, 'explicit_fixture_quit_anyway': True, 'cli_exit_codes': True}, indent=2))
finally:
    if process is not None and process.poll() is None:
        # Explicit private-fixture discard only, never the user's app.
        process.terminate()
        process.wait(timeout=30)
