#!/usr/bin/env python3
# Usage: scripts/ios-icon.py ios/DiBillSion/Assets.xcassets/AppIcon.appiconset/icon-1024.png
# Renders the launcher icon (teal square, amber + white overlapping circles; same drawing as
# android/app/src/main/res/drawable/ic_launcher_*.xml) as an opaque 1024x1024 PNG.
import zlib, struct, sys, math
N=1024; s=N/108
bg=(0x1F,0x6F,0x5C); amber=(0xF2,0xC1,0x4E); white=(255,255,255)
circles=[((46*s,54*s),16*s,amber),((62*s,54*s),16*s,white)]  # white drawn last
def color(x,y):
    c=bg
    for (cx,cy),r,col in circles:
        if (x-cx)**2+(y-cy)**2<=r*r: c=col
    return c
rows=[]
for y in range(N):
    row=bytearray([0])
    for x in range(N):
        near=any(abs(math.hypot(x+.5-cx,y+.5-cy)-r)<2 for (cx,cy),r,_ in circles)
        if not near:
            row+=bytes(color(x+.5,y+.5))
        else:
            acc=[0,0,0]
            for i in range(4):
                for j in range(4):
                    c=color(x+(i+.5)/4,y+(j+.5)/4)
                    for k in range(3): acc[k]+=c[k]
            row+=bytes(round(a/16) for a in acc)
    rows.append(bytes(row))
def chunk(t,d): return struct.pack(">I",len(d))+t+d+struct.pack(">I",zlib.crc32(t+d)&0xffffffff)
png=b"\x89PNG\r\n\x1a\n"+chunk(b"IHDR",struct.pack(">IIBBBBB",N,N,8,2,0,0,0))+chunk(b"IDAT",zlib.compress(b"".join(rows),9))+chunk(b"IEND",b"")
open(sys.argv[1],"wb").write(png)
