"""Build `sgpo-skins.bin`, the textured RetroSpy skins the plugin's layout merge adds at load, from an `info_melee`
layout.arc that `tools/sgpo_installer` patched with those skins.

Only SGPO's own data is taken: its texture names, the 36 skin `pic1` panes and `mat1` materials (indices made
relative), the textures' BRTI fields (pointers cleared) and their pixel data (zlib). Nothing of Nintendo's is copied.
The output is a build input (`SGPO_SKIN_BLOB`), not a repository file.

    python3 tools/skin_blob.py <patched layout.arc> <out sgpo-skins.bin>

Format (little-endian), read by `crates/sgpo-layout-merge` (`SkinBlob`):
    0x00 b"SGPOSKN1"
    0x08 u16 count, u16 pic_len, u16 mat_len, u16 mat_texmap_offset
    0x10 u32 data_len, u32 zlib_len, u32 data_align, u32 0
    0x20 count * 32-byte NUL-padded texture names (also the txl1 names)
         count * pic_len  pic1 sections, material index = k
         count * mat_len  mat1 entries, first texmap's txl index = k
         count * 0xA0     BRTI headers, size and pointer fields zero
         zlib_len         zlib stream of the pixel data, each texture at the next data_align boundary
"""
import struct
import sys
import zlib

TARGETS = ('blyt/info_melee.bflyt', 'blyt/info_melee_lct_player_00.bflyt', 'blyt/info_melee_lct_player_01.bflyt')
PREFIXES = (b'sgpo_alt_', b'sgpo_gct_')
PIC1_MATERIAL = 0x64
BRTI_LEN = 0xA0


def members(d):
    count, = struct.unpack_from('<H', d, 0x1A)
    data_off, = struct.unpack_from('<I', d, 0x0C)
    names = 0x20 + 16 * count + 8
    out = {}
    for i in range(count):
        _, attr, s, e = struct.unpack_from('<IIII', d, 0x20 + 16 * i)
        at = names + (attr & 0xFFFFFF) * 4
        out[d[at:d.index(b'\0', at)].decode()] = d[data_off + s:data_off + e]
    return out


def sections(b):
    at, = struct.unpack_from('<H', b, 0x06)
    for _ in range(struct.unpack_from('<H', b, 0x10)[0]):
        size, = struct.unpack_from('<I', b, at + 4)
        yield b[at:at + 4], b[at:at + size]
        at += size


def table(sec, base):
    n, = struct.unpack_from('<H', sec, 8)
    return n, struct.unpack_from(f'<{n}I', sec, 12), base


def skin_parts(flyt):
    secs = list(sections(flyt))
    txl = next(s for k, s in secs if k == b'txl1')
    mat = next(s for k, s in secs if k == b'mat1')
    tn, toffs, _ = table(txl, 12)
    txl_names = [txl[12 + o:txl.index(b'\0', 12 + o)] for o in toffs]
    skin_txl = [k for k, n in enumerate(txl_names) if n.startswith(b'tex_sgpo_')]
    mn, moffs, _ = table(mat, 0)
    mats = [mat[moffs[k]:(moffs[k + 1] if k + 1 < mn else len(mat))] for k in range(mn)]
    skin_mat = [k for k, m in enumerate(mats) if m[:9] == b'mat_sgpo_']
    pics = [s for k, s in secs if k == b'pic1' and s[0x0C:0x24].startswith(PREFIXES)]
    count = len(pics)
    assert len(skin_txl) == len(skin_mat) == count, (len(skin_txl), len(skin_mat), count)
    t0, m0 = skin_txl[0], skin_mat[0]
    assert skin_txl == list(range(t0, t0 + count)) and t0 + count == tn, 'skin txl1 names must be appended last'
    assert skin_mat == list(range(m0, m0 + count)) and m0 + count == mn, 'skin materials must be appended last'
    out_pics, out_mats = [], []
    for k, p in enumerate(pics):
        assert struct.unpack_from('<H', p, PIC1_MATERIAL)[0] == m0 + k, f'pic {k} material'
        out_pics.append(p[:PIC1_MATERIAL] + struct.pack('<H', k) + p[PIC1_MATERIAL + 2:])
    texmap = None
    for k, m in enumerate(mats[m0:]):
        hits = {j for j in range(28, len(m) - 1, 2) if struct.unpack_from('<H', m, j)[0] == t0 + k}
        texmap = hits if texmap is None else texmap & hits
    assert texmap and len(texmap) == 1, f'texmap offset not unique: {texmap}'
    off = texmap.pop()
    for k, m in enumerate(mats[m0:]):
        out_mats.append(m[:off] + struct.pack('<H', k) + m[off + 2:])
    return [txl_names[t] for t in skin_txl], out_pics, out_mats, off


def main(src, dst):
    m = members(open(src, 'rb').read())
    parts = [skin_parts(m[t]) for t in TARGETS]
    names, pics, mats, texmap = parts[0]
    for other in parts[1:]:
        assert other == parts[0], 'the three layouts must carry identical skin panes and materials'
    assert len({len(p) for p in pics}) == 1 and len({len(x) for x in mats}) == 1

    b = m['timg/__Combined.bntx']
    q = lambda at: struct.unpack_from('<Q', b, at)[0]  # noqa: E731
    n, = struct.unpack_from('<I', b, 0x24)
    brtis, datas, align = [], bytearray(), None
    by_name = {}
    for k in range(n):
        t = q(0x198 + 8 * k)
        nm = q(t + 0x60)
        ln, = struct.unpack_from('<H', b, nm)
        by_name[b[nm + 2:nm + 2 + ln]] = t
    for name in names:
        t = by_name[name]
        size, a = struct.unpack_from('<II', b, t + 0x50)
        align = a if align is None else align
        assert a == align, 'one alignment for every skin texture'
        hdr = bytearray(b[t:t + BRTI_LEN])
        hdr[4:16] = bytes(12)
        hdr[0x60:0x78] = bytes(24)
        hdr[0x78:0xA0] = bytes(40)
        brtis.append(bytes(hdr))
        datas += bytes(-len(datas) % align)
        datas += b[q(q(t + 0x70)):q(q(t + 0x70)) + size]
    z = zlib.compress(bytes(datas), 9)
    out = bytearray(b'SGPOSKN1')
    out += struct.pack('<HHHH', len(names), len(pics[0]), len(mats[0]), texmap)
    out += struct.pack('<IIII', len(datas), len(z), align, 0)
    for name in names:
        assert len(name) < 32
        out += name + bytes(32 - len(name))
    for chunk in (pics, mats, brtis):
        out += b''.join(chunk)
    out += z
    open(dst, 'wb').write(out)
    print(f'{dst}: {len(names)} skins, pic {len(pics[0])} B, mat {len(mats[0])} B (texmap @ {texmap}), '
          f'pixels {len(datas)} B -> zlib {len(z)} B, align {align}, blob {len(out)} B')


if __name__ == '__main__':
    main(*sys.argv[1:3])
