#!/usr/bin/env python3
"""Native GTK root GetChildAtIndex diagnostic; private diagnosis lab only."""
import ast
import os
import select
import subprocess
import sys
import time
from contextlib import ExitStack

LAB = 'koett-gtk-a11y-diagnosis'
assert os.environ.get('AGENT_LAB_RUNTIME') == f'/run/user/{os.getuid()}/agent-lab/{LAB}'
assert os.environ.get('XDG_RUNTIME_DIR') == os.environ['AGENT_LAB_RUNTIME']
os.environ['GTK_A11Y'] = 'atspi'


def call(address, dest, path, method, *args):
    return subprocess.run(['gdbus', 'call', '--address', address, '--dest', dest,
                           '--object-path', path, '--method', method, *args],
                          check=False, capture_output=True, text=True, timeout=4)


def child():
    import gi
    gi.require_version('Gtk', '4.0')
    from gi.repository import GLib, Gtk
    Gtk.init()
    hidden = Gtk.Window(title='Never presented main')
    visible = Gtk.Window(title='Disposable settings')
    visible.present()
    loop = GLib.MainLoop()

    def command(_source, _condition):
        line = sys.stdin.readline().strip()
        if line == 'destroy':
            visible.destroy()
            print('destroyed', flush=True)
        elif line == 'quit':
            loop.quit()
        return True

    GLib.io_add_watch(sys.stdin, GLib.IO_IN, command)
    GLib.idle_add(lambda: (print('ready', flush=True), False)[1])
    loop.run()
    hidden.destroy()


def main():
    address = ast.literal_eval(subprocess.check_output([
        'gdbus', 'call', '--session', '--dest', 'org.a11y.Bus', '--object-path',
        '/org/a11y/bus', '--method', 'org.a11y.Bus.GetAddress'], text=True))[0]
    os.environ['AT_SPI_BUS_ADDRESS'] = address
    with ExitStack() as files:
        registry_log = files.enter_context(open('registry.log', 'w'))
        registry = subprocess.Popen(['/usr/lib/at-spi2-registryd'], stdout=registry_log,
                                    stderr=subprocess.STDOUT)
        process = None
        try:
            owner = call(address, 'org.freedesktop.DBus', '/org/freedesktop/DBus',
                         'org.freedesktop.DBus.GetNameOwner', 'org.a11y.atspi.Registry')
            deadline = time.monotonic() + 5
            while owner.returncode and time.monotonic() < deadline:
                owner = call(address, 'org.freedesktop.DBus', '/org/freedesktop/DBus',
                             'org.freedesktop.DBus.GetNameOwner', 'org.a11y.atspi.Registry')
            assert owner.returncode == 0, owner.stderr
            owner_name = ast.literal_eval(owner.stdout)[0]
            owner_pid = call(address, 'org.freedesktop.DBus', '/org/freedesktop/DBus',
                             'org.freedesktop.DBus.GetConnectionUnixProcessID', owner_name)
            assert str(registry.pid) in owner_pid.stdout, owner_pid.stdout
            print('owned registry:', registry.pid, owner_name, flush=True)
            log = files.enter_context(open('native-stderr.log', 'w'))
            process = subprocess.Popen([sys.executable, __file__, '--child'],
                                       stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                       stderr=log, text=True)
            assert select.select([process.stdout], [], [], 10)[0], 'child readiness'
            assert process.stdout.readline().strip() == 'ready'
            deadline = time.monotonic() + 10
            name = None
            while time.monotonic() < deadline and name is None:
                names = ast.literal_eval(call(address, 'org.freedesktop.DBus',
                    '/org/freedesktop/DBus', 'org.freedesktop.DBus.ListNames').stdout)[0]
                for candidate in names:
                    if not candidate.startswith(':'):
                        continue
                    result = call(address, 'org.freedesktop.DBus', '/org/freedesktop/DBus',
                                  'org.freedesktop.DBus.GetConnectionUnixProcessID', candidate)
                    if str(process.pid) in result.stdout:
                        name = candidate
                        break
            assert name, 'own GTK accessibility connection'
            path = '/org/a11y/atspi/accessible/root'
            method = 'org.a11y.atspi.Accessible.GetChildAtIndex'
            control = call(address, name, path, method, '0')
            print('visible control:', control.returncode, control.stdout.strip(), control.stderr.strip(), flush=True)
            assert control.returncode == 0
            if '--keep-visible' not in sys.argv:
                process.stdin.write('destroy\n')
                process.stdin.flush()
                assert process.stdout.readline().strip() == 'destroyed'
            result = call(address, name, path, method, '0')
            print('after retirement:', result.returncode, result.stdout.strip(), result.stderr.strip(), flush=True)
            try:
                code = process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                code = None
            print('native PID:', process.pid, 'exit:', code, flush=True)
            print('registry still alive:', registry.poll() is None, flush=True)
        finally:
            for owned in [process, registry]:
                if owned is not None and owned.poll() is None:
                    owned.terminate()
                    owned.wait(timeout=5)



if __name__ == '__main__':
    if '--child' in sys.argv:
        child()
    else:
        main()
