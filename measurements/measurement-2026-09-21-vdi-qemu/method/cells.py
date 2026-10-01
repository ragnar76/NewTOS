#!/usr/bin/env python3
"""Print selected cells of VDI records from several dumps side by side (record format 2)."""
import struct, sys
HDR, REC = 32, 1592
OFF = {'d0': 4, 'contrl': 8, 'intin': 32, 'ptsin': 288, 'intout': 800, 'ptsout': 1056, 'contrl_before': 1568}
def load(path):
    b = open(path, 'rb').read()
    recs = {}
    for i in range((len(b)-HDR)//REC):
        r = b[HDR+i*REC:HDR+(i+1)*REC]
        recs[struct.unpack('>H', r[0:2])[0]] = r
    return recs
def cell(r, field, idx):
    if field == 'd0':
        return struct.unpack('>i', r[4:8])[0]
    return struct.unpack('>h', r[OFF[field]+2*idx:OFF[field]+2*idx+2])[0]
names = sys.argv[1::2][:3]; paths = sys.argv[2::2][:3]
dumps = [load(p) for p in paths]
wanted = [
 (33,'intout',0),(33,'intout',1),(33,'intout',13),(33,'intout',39),
 (34,'intout',4),(34,'intout',5),(34,'ptsout',0),(34,'ptsout',1),(34,'ptsout',2),(34,'ptsout',3),(34,'intout',19),
 (250,'intout',4),(250,'intout',5),(250,'ptsout',0),(250,'ptsout',1),(250,'ptsout',2),(250,'ptsout',3),(250,'intout',19),
 (252,'intout',4),(252,'intout',5),(252,'ptsout',0),(252,'ptsout',1),(252,'ptsout',2),(252,'ptsout',3),(252,'intout',19),
 (42,'intout',0),(113,'intout',0),(72,'intout',0),(73,'intout',0),
 (54,'intout',0),(54,'intout',1),(160,'intout',0),(160,'intout',1),(161,'intout',0),(161,'intout',1),
 (270,'intout',0),(270,'intout',1),(272,'intout',0),(272,'intout',1),
 (173,'intout',0),(173,'intout',1),
 (274,'intout',0),(274,'intout',1),(274,'intout',13),(274,'intout',39),(274,'contrl',6),(274,'contrl_before',6),
]
print(f"{'case':>5} {'field[idx]':<18}" + "".join(f"{n:>16}" for n in names))
for (c, f, i) in wanted:
    vals = []
    for d in dumps:
        v = cell(d[c], f, i)
        vals.append(f"{v} (0x{v & 0xFFFF:04X})")
    print(f"{c:>5} {f+'['+str(i)+']':<18}" + "".join(f"{v:>16}" for v in vals))
