import os,pty,sys,time,select,fcntl,termios,struct,json,re,tempfile
# TUI harness: drives the Muse binary in a pseudo-terminal and answers its startup probes.
# Configure through the environment (no paths are baked in):
#   OMM_MUSE_BIN   the Muse binary to drive (required)
#   OMM_PTY_HOME   sandbox HOME (default: a fresh temp dir; never the real home)
#   OMM_PTY_WS     workspace cwd (default: a fresh temp dir)
BIN=os.environ.get("OMM_MUSE_BIN") or sys.exit("drive.py: set OMM_MUSE_BIN to the Muse binary")
HOME=os.environ.get("OMM_PTY_HOME") or tempfile.mkdtemp(prefix="omm-pty-home-")
WS=os.environ.get("OMM_PTY_WS") or tempfile.mkdtemp(prefix="omm-pty-ws-")
os.makedirs(HOME,exist_ok=True); os.makedirs(WS,exist_ok=True)
script=json.load(open(sys.argv[1]))
outfile=sys.argv[2]
extra=sys.argv[3:] 
env=dict(os.environ)
env.update({"MUSE_NO_AUTO_UPDATE":"1","HOME":HOME,"XDG_CONFIG_HOME":HOME+"/.config","XDG_DATA_HOME":HOME+"/.local/share","TERM":"xterm-256color","COLORTERM":"truecolor"})
for kv in list(env.keys()):
    pass
# allow env overrides via ENVK=V in argv after outfile prefixed with 'env:'
args=[BIN,"--provider","echo","--yolo"]
for a in extra:
    if a.startswith("env:"):
        k,v=a[4:].split("=",1); env[k]=v
    else:
        args.append(a)
pid,fd=pty.fork()
if pid==0:
    os.chdir(WS)
    os.execve(BIN,args,env)
fcntl.ioctl(fd,termios.TIOCSWINSZ,struct.pack("HHHH",50,180,0,0))
buf=b""
PAL=["000000","cd0000","00cd00","cdcd00","0000ee","cd00cd","00cdcd","e5e5e5",
     "7f7f7f","ff0000","00ff00","ffff00","5c5cff","ff00ff","00ffff","ffffff"]
def respond(d):
    out=b""
    # OSC color queries
    for m in re.finditer(rb'\x1b\]10;\?\x07', d): out+=b'\x1b]10;rgb:d0d0/d0d0/d0d0\x07'
    for m in re.finditer(rb'\x1b\]11;\?\x07', d): out+=b'\x1b]11;rgb:1010/1010/1010\x07'
    for m in re.finditer(rb'\x1b\]4;(\d+);\?\x07', d):
        i=int(m.group(1)); c=PAL[i%16]
        out+=('\x1b]4;%d;rgb:%s%s/%s%s/%s%s\x07'%(i,c[0:2],c[0:2],c[2:4],c[2:4],c[4:6],c[4:6])).encode()
    n=d.count(b'\x1b[6n'); out+=b'\x1b[1;1R'*n
    if b'\x1b[c' in d: out+=b'\x1b[?62;1;2;6;9;15;22c'
    if b'\x1b[?u' in d: out+=b'\x1b[?0u'
    if b'\x1b[>0q' in d: out+=b'\x1bP>|xterm(370)\x1b\\'
    if b'\x1b[5n' in d: out+=b'\x1b[0n'
    return out
def pump(t):
    global buf
    end=time.time()+t
    while time.time()<end:
        r,_,_=select.select([fd],[],[],0.1)
        if r:
            try: d=os.read(fd,65536)
            except OSError: return False
            if not d: return False
            buf+=d
            rsp=respond(d)
            if rsp: os.write(fd,rsp)
    return True
for st in script:
    if st[0]=="wait": pump(st[1])
    elif st[0]=="send": os.write(fd,st[1].encode())
    elif st[0]=="raw": os.write(fd,bytes(st[1]))
pump(2)
open(outfile,"wb").write(buf)
try: os.kill(pid,9)
except Exception: pass
print("captured",len(buf))
