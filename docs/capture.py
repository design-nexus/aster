#!/usr/bin/env python3
"""Display Aster in Foot and accept QA commands through a private FIFO."""
import os, pty, select, signal, struct, fcntl, termios, subprocess, sys, pathlib, json, tty
root=pathlib.Path(__file__).resolve().parents[1]
original_termios=termios.tcgetattr(0);tty.setraw(0)
master,slave=pty.openpty()
cols,rows=os.get_terminal_size(1)
fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',rows,cols,0,0))
fifo=pathlib.Path('/tmp/aster-capture.fifo')
if fifo.exists(): fifo.unlink()
os.mkfifo(fifo,0o600)
control=os.open(fifo,os.O_RDWR|os.O_NONBLOCK)
process=subprocess.Popen([str(root/'target/release/aster')],stdin=slave,stdout=slave,stderr=slave,env={**os.environ,'STARSHIP_CONFIG':str(root/'docs/preview.toml'),'XDG_DATA_HOME':'/tmp/aster-qa-data','TERM':'xterm-256color'},start_new_session=True,cwd=root)
os.close(slave)
def resized(signum,frame):
    cols,rows=os.get_terminal_size(1);fcntl.ioctl(master,termios.TIOCSWINSZ,struct.pack('HHHH',rows,cols,0,0));os.kill(process.pid,signal.SIGWINCH)
signal.signal(signal.SIGWINCH,resized)
buffer=b''
try:
    while process.poll() is None:
        ready,_,_=select.select([master,control,0],[],[],.1)
        if master in ready:
            try: os.write(1,os.read(master,65536))
            except OSError: break
        if 0 in ready: os.write(master,os.read(0,4096))
        if control in ready:
            buffer+=os.read(control,4096)
            while b'\n' in buffer:
                line,buffer=buffer.split(b'\n',1)
                command=line.decode().split()
                if command[0]=='keys': os.write(master,bytes.fromhex(command[1]))
                if command[0]=='resize':
                    cols,rows=map(int,command[1:]);fcntl.ioctl(master,termios.TIOCSWINSZ,struct.pack('HHHH',rows,cols,0,0));os.kill(process.pid,signal.SIGWINCH)
                if command[0]=='physical': pathlib.Path('/tmp/aster-foot-size.json').write_text(json.dumps(list(os.get_terminal_size(1))))
finally:
    termios.tcsetattr(0,termios.TCSANOW,original_termios)
    if process.poll() is None: process.terminate()
    os.close(master);os.close(control);fifo.unlink(missing_ok=True)
