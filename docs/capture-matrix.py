#!/usr/bin/env python3
"""Capture the existing aster-qa Foot window; use capture.py as its command."""
import json, subprocess, time, pathlib
root=pathlib.Path(__file__).resolve().parent
c=next(c for c in json.loads(subprocess.check_output(['hyprctl','clients','-j'])) if c['class']=='aster-qa')
address=c['address'];window='assert(hl.get_window('+json.dumps('address:'+address)+'))'
subprocess.run(['hyprctl','eval','hl.dispatch(hl.dsp.window.float({action="set",window='+window+'}))'],check=True,capture_output=True)
def send(hex):
    with open('/tmp/aster-capture.fifo','w') as f:f.write('keys '+hex+'\n')
    time.sleep(.14)
def resize(cols,rows):
    width,height=cols*8+16,rows*18+16
    for attempt in range(5):
        subprocess.run(['hyprctl','eval','hl.dispatch(hl.dsp.window.resize({window='+window+',x='+str(width)+',y='+str(height)+'}))'],check=True,capture_output=True)
        subprocess.run(['hyprctl','eval','hl.dispatch(hl.dsp.window.center({window='+window+'}))'],check=True,capture_output=True)
        time.sleep(.45)
        with open('/tmp/aster-capture.fifo','w') as f:f.write('physical\n')
        time.sleep(.15)
        actual=json.loads(pathlib.Path('/tmp/aster-foot-size.json').read_text())
        if actual==[cols,rows]:return
        width+=(cols-actual[0])*8;height+=(rows-actual[1])*18
    raise RuntimeError(f'Could not get {cols}x{rows}: {actual}')
def capture(name):
    c=next(c for c in json.loads(subprocess.check_output(['hyprctl','clients','-j'])) if c['address']==address)
    x,y=c['at'];w,h=c['size'];subprocess.run(['grim','-g',f'{x},{y} {w}x{h}',str(root/'qa'/name)],check=True)
for cols,rows in [(120,40),(100,30),(80,24),(60,20)]:
    resize(cols,rows)
    for screen,panes in [(1,3),(2,3),(3,3),(4,2),(5,3)]:
        send(bytes(str(screen),'ascii').hex())
        for pane in range(panes):
            capture(f'{cols}x{rows}-screen{screen}-pane{pane}.png')
            send('1b5b31377e')
    send('31')
    send('0b');capture(f'{cols}x{rows}-actions.png');send('1b')
    send('1b4f51');capture(f'{cols}x{rows}-navigation.png');send('1b')
    send('3f');capture(f'{cols}x{rows}-help.png');send('1b')
    send('6e');capture(f'{cols}x{rows}-builder.png');send('78')
resize(120,40)
send('31');send('767676');time.sleep(.5);capture('120x40-scenario-right.png')
(root/'screenshot.png').write_bytes((root/'qa/120x40-scenario-right.png').read_bytes())
resize(60,20);send('1b5b31377e');send('1b5b31377e');capture('60x20-focused-preview.png')
(root/'compact.png').write_bytes((root/'qa/60x20-focused-preview.png').read_bytes())
print('Captured every screen and pane plus Actions, Navigation, Help, Builder at all four sizes in Foot')

with open("/tmp/aster-capture.fifo","w") as f:f.write("keys 71\n")
