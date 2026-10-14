#!/usr/bin/env python3
"""Exercise the release executable in a real PTY with temporary configurations."""
import fcntl
import os
import pathlib
import pty
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

binary = str(pathlib.Path(sys.argv[1]).resolve())

def drain(master, seconds=.2):
    end = time.monotonic() + seconds
    body = bytearray()
    while time.monotonic() < end:
        if select.select([master], [], [], .02)[0]:
            try:
                chunk=os.read(master, 65536)
                if b"\x1b[6n" in chunk: os.write(master,b"\x1b[1;1R")
                body.extend(chunk)
            except OSError:
                break
    return bytes(body)

with tempfile.TemporaryDirectory() as folder:
    path = pathlib.Path(folder) / 'starship.toml'
    original = '# keep this\nformat = "$directory$character"\nright_format = "$jobs"\n'
    path.write_text(original)
    for mouse in (True, False):
        master, slave = pty.openpty()
        before = termios.tcgetattr(slave)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 120, 0, 0))
        env = {**os.environ, 'STARSHIP_CONFIG': str(path), 'TERM': 'xterm-256color', 'XDG_DATA_HOME': folder}
        process = subprocess.Popen([binary] + ([] if mouse else ['--no-mouse']), stdin=slave, stdout=slave, stderr=slave, env=env, start_new_session=True)
        output = drain(master, .8)
        assert b'\x1b[?1049h' in output, 'alternate screen was not entered'
        for cols, rows in [(100, 30), (80, 24), (60, 20), (1, 1), (120, 40)]:
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
            os.kill(process.pid, signal.SIGWINCH)
            output += drain(master, .08)
        # F2, search, Esc; actions, Esc; each screen and pane; help.
        for keys in [b'\x1bOQ', b'All settings', b'\x1b', b'\x0b', b'\x1b', b'2', b'\x1b[17~', b'3', b'Source', b'\r', b'\x1b[17~', b'?', b'\x1b', b'1']:
            os.write(master, keys)
            output += drain(master, .06)
        # Stage a builder then cancel: no existing draft or file is changed.
        os.write(master, b'n')
        output += drain(master, .15)
        os.write(master, b'x')
        output += drain(master)
        # Edit an advanced expression; q and numbers remain text in the input.
        os.write(master, b'\x0b')
        output += drain(master)
        os.write(master, b'Edit left format\r')
        output += drain(master)
        os.write(master, b'q123')
        output += drain(master)
        assert process.poll() is None, 'input escaped to a quit shortcut'
        os.write(master, b'\r')
        output += drain(master)
        os.write(master, b'\x03')
        output += drain(master)
        assert process.poll() is None, 'Ctrl+C bypassed unsaved guard'
        os.write(master, b'\x1b')
        output += drain(master)
        # SIGTSTP releases raw mode; SIGCONT restores it.
        os.kill(process.pid, signal.SIGTSTP)
        output += drain(master, .25)
        assert termios.tcgetattr(slave) == before, 'suspension did not restore termios'
        os.kill(process.pid, signal.SIGCONT)
        output += drain(master)
        os.write(master, b'q')
        output += drain(master)
        os.write(master, b'd')
        output += drain(master, .3)
        process.wait(timeout=5)
        if process.returncode != 0: pathlib.Path("/tmp/aster-pty-failure.log").write_bytes(output)
        assert process.returncode == 0, f"exit {process.returncode}; see /tmp/aster-pty-failure.log"
        assert termios.tcgetattr(slave) == before, 'termios changed after exit'
        assert b'\x1b[?1049l' in output and b'\x1b[?1000l' in output
        assert path.read_text() == original, 'unsaved fixture was overwritten'
        os.close(master)
        os.close(slave)
    # Closing the terminal must stop the app, even with no further keyboard event.
    master,slave=pty.openpty()
    fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',24,80,0,0))
    process=subprocess.Popen([binary,'--no-mouse'],stdin=slave,stdout=slave,stderr=slave,env=env,start_new_session=True)
    drain(master,.6)
    os.close(master);os.close(slave)
    try: process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill();process.wait();raise AssertionError('terminal hangup left a running process')
print('PTY: navigation, resize, input precedence, staged cancel, guarded quit, suspension, termios, alternate screen mouse release and terminal hangup passed')
