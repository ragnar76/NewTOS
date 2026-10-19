# rootscan.py <card.img> -- raw FAT16 root-directory dump of partition 1 (MBR): per 32-byte entry status/attr/name, LFN entries decoded
import struct, sys
img = open(sys.argv[1], 'rb')
mbr = img.read(512); lba = struct.unpack_from('<I', mbr, 446 + 8)[0]
img.seek(lba * 512); b = img.read(512)
bps, spc, rsv, nfat, nroot = struct.unpack_from('<HBHBH', b, 11); fatsz = struct.unpack_from('<H', b, 22)[0]
root = lba * 512 + (rsv + nfat * fatsz) * bps
img.seek(root); d = img.read(nroot * 32)
n_lfn = n_del = n_live = 0
for i in range(0, len(d), 32):
    e = d[i:i + 32]
    if e[0] == 0: break
    st, at = e[0], e[11]
    if at == 0x0F:
        chars = e[1:11] + e[14:26] + e[28:32]
        txt = chars.decode('utf-16le', 'replace').split('\x00')[0].split('￿')[0]
        kind = 'LFN-DEL' if st == 0xE5 else 'LFN'
        n_lfn += 1
        print(f'{i//32:3d} {kind:8s} seq={st:02x} chk={e[13]:02x} "{txt}"')
    else:
        nm = e[:8].decode('latin1') + '.' + e[8:11].decode('latin1')
        cl = struct.unpack_from('<H', e, 26)[0]; ln = struct.unpack_from('<I', e, 28)[0]
        if st == 0xE5: n_del += 1
        else: n_live += 1
        print(f'{i//32:3d} {"DEL" if st==0xE5 else "ENTRY":8s} {nm!r} attr={at:02x} cl={cl} len={ln}')
print(f'rootscan: live={n_live} deleted={n_del} lfn-slots={n_lfn}')
