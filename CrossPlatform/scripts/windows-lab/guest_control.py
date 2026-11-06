#!/usr/bin/env python3
"""Start/stop the private Windows guest without controlling the host desktop."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import time

CACHE = Path.home() / '.cache/koett-windows'


def qmp(command):
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(5)
        connection.connect(str(CACHE / 'qmp.sock'))
        stream = connection.makefile('rwb', buffering=0)
        stream.readline()
        stream.write(b'{"execute":"qmp_capabilities"}\n')
        stream.readline()
        stream.write((json.dumps({'execute': command})+'\n').encode())
        while True:
            result = json.loads(stream.readline())
            if 'return' in result or 'error' in result:
                return result


def running():
    try:
        pid = int((CACHE / 'qemu.pid').read_text())
        name = Path(f'/proc/{pid}/comm').read_text().strip()
        return (name.startswith('qemu-system') or name == 'koett-winlab') and Path(f'/proc/{pid}/exe').resolve().name in ('qemu-system-x86_64', 'koett-vm')
    except (OSError, ValueError):
        return False


def qemu_binary():
    source = Path(shutil.which('qemu-system-x86_64'))
    target = CACHE / 'koett-vm'
    def digest(path):
        with path.open('rb') as stream:
            return hashlib.file_digest(stream, 'sha256').digest()
    if not target.exists() or digest(target) != digest(source):
        shutil.copy2(source, target)
    return target


def host_idle_fraction():
    def sample():
        values = [int(v) for v in Path('/proc/stat').read_text().splitlines()[0].split()[1:9]]
        return sum(values), values[3]
    total, idle = sample()
    time.sleep(0.5)
    end_total, end_idle = sample()
    return (end_idle - idle) / max(1, end_total - total)


def start(install, dual_display=False, virtio_display=False):
    if running():
        raise SystemExit('Guest is already running; use its private VNC socket.')
    required = ['qemu-system-x86_64', 'qemu-img', 'swtpm']
    if any(not shutil.which(tool) for tool in required):
        raise SystemExit('Install: sudo pacman -S --needed qemu-desktop edk2-ovmf swtpm')
    available = int(next(line.split()[1] for line in Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemAvailable:')))
    if available < 12 * 1024 * 1024:
        raise SystemExit('Need 12 GiB available: 8 GiB guest plus 4 GiB headroom.')
    if host_idle_fraction() < 0.70:
        raise SystemExit('Host is busy; leave the guest off and retry when at least 70% CPU is idle.')
    if not os.access('/dev/kvm', os.R_OK | os.W_OK):
        raise SystemExit('/dev/kvm is not accessible.')
    CACHE.mkdir(parents=True, exist_ok=True)
    CACHE.chmod(0o700)
    disk = CACHE / 'windows11.qcow2'
    if install and disk.exists():
        raise SystemExit('Refusing disk-wiping installer on an existing guest disk. Start normally.')
    if not disk.exists():
        if not install:
            raise SystemExit('Guest disk missing. Prepare official ISO and seed, then --install.')
        subprocess.run(['qemu-img', 'create', '-f', 'qcow2', str(disk), '96G'], check=True)
    nvram = CACHE / 'OVMF_VARS.4m.fd'
    if not nvram.exists():
        shutil.copyfile('/usr/share/edk2/x64/OVMF_VARS.4m.fd', nvram)
    (CACHE / 'tpm').mkdir(exist_ok=True)
    subprocess.run(['swtpm', 'socket', '--tpm2', '--tpmstate', f'dir={CACHE}/tpm', '--ctrl', f'type=unixio,path={CACHE}/tpm.sock', '--log', f'file={CACHE}/tpm.log', '--pid', f'file={CACHE}/tpm.pid', '--daemon'], check=True)
    args = ['nice', '-n', '19', str(qemu_binary()), '-L', '/usr/share/qemu', '-name', 'Koett-Windows-Lab,process=koett-winlab', '-enable-kvm', '-machine', 'q35,smm=on', '-cpu', 'host', '-smp', '4', '-m', '8192', '-global', 'driver=cfi.pflash01,property=secure,value=on', '-drive', 'if=pflash,format=raw,unit=0,readonly=on,file=/usr/share/edk2/x64/OVMF_CODE.secboot.4m.fd', '-drive', f'if=pflash,format=raw,unit=1,file={nvram}', '-drive', f'file={disk},format=qcow2,if=ide', '-device', 'qemu-xhci', '-device', 'usb-tablet', '-chardev', f'socket,id=chrtpm,path={CACHE}/tpm.sock', '-tpmdev', 'emulator,id=tpm0,chardev=chrtpm', '-device', 'tpm-tis,tpmdev=tpm0', '-netdev', 'user,id=n0', '-device', 'e1000e,netdev=n0', '-display', 'none', '-vga', 'none', '-device', 'VGA,vgamem_mb=64,xres=2560,yres=1600', '-vnc', f'unix:{CACHE}/vnc.sock', '-chardev', f'socket,id=serial,path={CACHE}/serial.sock,server=on,wait=off', '-serial', 'chardev:serial', '-qmp', f'unix:{CACHE}/qmp.sock,server=on,wait=off', '-boot', 'order=c', '-daemonize', '-pidfile', str(CACHE / 'qemu.pid'), '-D', str(CACHE / 'qemu.log'), '-audiodev', f'wav,id=audio,path={CACHE}/guest-audio.wav', '-device', 'intel-hda', '-device', 'hda-duplex,audiodev=audio']
    if virtio_display:
        args[args.index('VGA,vgamem_mb=64,xres=2560,yres=1600')] = 'virtio-vga,xres=2560,yres=1600,id=m4-primary,max_outputs=1'
    if dual_display:
        if virtio_display:
            args += ['-device', 'pcie-root-port,id=m4-display-port,bus=pcie.0,slot=8,chassis=8', '-device', 'virtio-gpu-pci,bus=m4-display-port,id=m4-secondary,max_outputs=1,xres=1280,yres=1024']
        else:
            args += ['-device', 'secondary-vga,vgamem_mb=64,xres=1280,yres=1024,id=m4-display']
    if install:
        args += ['-drive', f'file={CACHE}/windows11-enterprise-eval.iso,media=cdrom,readonly=on', '-drive', f'file=fat:{CACHE}/seed,format=raw,readonly=on,if=none,id=seed', '-device', 'usb-storage,drive=seed,removable=on', '-boot', 'order=c,once=d,menu=on']
    try:
        subprocess.run(args, check=True, env={**os.environ, 'QEMU_MODULE_DIR': '/usr/lib/qemu'})
    except subprocess.CalledProcessError:
        tpm_pid = int((CACHE / 'tpm.pid').read_text())
        if Path(f'/proc/{tpm_pid}/comm').read_text().strip() == 'swtpm':
            os.kill(tpm_pid, 15)
        raise
    pid = int((CACHE / 'qemu.pid').read_text())
    for task in Path(f'/proc/{pid}/task').iterdir():
        try:
            os.setpriority(os.PRIO_PROCESS, int(task.name), 19)
        except ProcessLookupError:
            pass
    print(f'Guest started: 4 vCPU, 8 GiB. Private VNC: {CACHE}/vnc.sock')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['start', 'stop', 'status'])
    parser.add_argument('--install', action='store_true', help='Attach installer and disk-wiping answer file; only for initial guest installation.')
    parser.add_argument('--dual-display', action='store_true', help='Add a second private emulated display for mixed-DPI and hotplug checks.')
    parser.add_argument('--virtio-display', action='store_true', help='Use the signed VirtIO display driver already installed in the guest.')
    args = parser.parse_args()
    if args.action == 'status':
        print(json.dumps({'running': running(), 'cache': str(CACHE)}))
    elif args.action == 'start':
        start(args.install, args.dual_display, args.virtio_display)
    elif running():
        print(qmp('system_powerdown'))
        for _ in range(50):
            if not running():
                print('Guest powered off.')
                return
            time.sleep(1)
        raise SystemExit('Guest has not powered off. Inspect guest console; no forced termination performed.')


if __name__ == '__main__':
    main()
