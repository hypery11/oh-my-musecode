import sys,re
# minimal vt100 screen renderer
class Screen:
    def __init__(self,rows=50,cols=180):
        self.rows=rows; self.cols=cols
        self.buf=[[' ']*cols for _ in range(rows)]
        self.cy=0; self.cx=0
    def put(self,ch):
        if self.cy<self.rows and self.cx<self.cols:
            self.buf[self.cy][self.cx]=ch
        self.cx+=1
        if self.cx>=self.cols: self.cx=self.cols-1
    def text(self):
        return '\n'.join(''.join(r).rstrip() for r in self.buf)
def run(data):
    s=Screen()
    i=0; n=len(data)
    while i<n:
        c=data[i]
        if c=='\x1b':
            m=re.match(r'\x1b\[([0-9;?<>! ]*)([A-Za-z@`])',data[i:])
            if m:
                p=m.group(1); f=m.group(2)
                nums=[int(x) for x in re.findall(r'\d+',p)] or []
                if f=='H' or f=='f':
                    s.cy=(nums[0]-1 if nums else 0); s.cx=(nums[1]-1 if len(nums)>1 else 0)
                elif f=='A': s.cy=max(0,s.cy-(nums[0] if nums else 1))
                elif f=='B': s.cy=min(s.rows-1,s.cy+(nums[0] if nums else 1))
                elif f=='C': s.cx=min(s.cols-1,s.cx+(nums[0] if nums else 1))
                elif f=='D': s.cx=max(0,s.cx-(nums[0] if nums else 1))
                elif f=='G': s.cx=(nums[0]-1 if nums else 0)
                elif f=='d': s.cy=(nums[0]-1 if nums else 0)
                elif f=='J':
                    k=nums[0] if nums else 0
                    if k==2 or k==3:
                        s.buf=[[' ']*s.cols for _ in range(s.rows)]
                    elif k==0:
                        for x in range(s.cx,s.cols): s.buf[s.cy][x]=' '
                        for y in range(s.cy+1,s.rows): s.buf[y]=[' ']*s.cols
                    elif k==1:
                        for x in range(0,s.cx+1): s.buf[s.cy][x]=' '
                        for y in range(0,s.cy): s.buf[y]=[' ']*s.cols
                elif f=='K':
                    k=nums[0] if nums else 0
                    if k==0:
                        for x in range(s.cx,s.cols): s.buf[s.cy][x]=' '
                    elif k==1:
                        for x in range(0,s.cx+1): s.buf[s.cy][x]=' '
                    else: s.buf[s.cy]=[' ']*s.cols
                i+=m.end(); continue
            m=re.match(r'\x1b\][^\x07\x1b]*(\x07|\x1b\\)',data[i:])
            if m: i+=m.end(); continue
            m=re.match(r'\x1bP.*?\x1b\\',data[i:],re.S)
            if m: i+=m.end(); continue
            m=re.match(r'\x1b[()][A-B0-9]',data[i:])
            if m: i+=m.end(); continue
            m=re.match(r'\x1b[=>78MD]',data[i:])
            if m:
                if m.group()=='\x1bM': s.cy=max(0,s.cy-1)
                i+=m.end(); continue
            i+=1; continue
        if c=='\n':
            s.cy+=1
            if s.cy>=s.rows:
                s.buf.pop(0); s.buf.append([' ']*s.cols); s.cy=s.rows-1
            i+=1; continue
        if c=='\r': s.cx=0; i+=1; continue
        if c=='\x08': s.cx=max(0,s.cx-1); i+=1; continue
        if c=='\t': s.cx=(s.cx//8+1)*8; i+=1; continue
        if ord(c)<32: i+=1; continue
        s.put(c); i+=1
    return s.text()
data=open(sys.argv[1],'rb').read().decode('utf-8','replace')
print(run(data))
