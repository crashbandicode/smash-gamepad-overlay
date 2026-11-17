//! The textured RetroSpy skins (`switch_pro_alt_builtin`, `gamecube_tron_builtin`): their textures, appended to the
//! archive's `timg/__Combined.bntx` in place, the way `tools/sgpo_installer` (Toolbox-Cli's `append_texture`) appends
//! them on the PC. The panes and materials that use them are added with SGPO's other panes (`lib.rs`).
//!
//! The skins come from a blob built by `tools/skin_blob.py` and embedded in the plugin: SGPO's texture names, pic1
//! panes, mat1 materials, BRTI headers and zlib pixel data, and nothing of Nintendo's.

use miniz_oxide::inflate::core::inflate_flags::{TINFL_FLAG_PARSE_ZLIB_HEADER, TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF};
use miniz_oxide::inflate::core::{decompress, DecompressorOxide};
use miniz_oxide::inflate::TINFLStatus;

use crate::{put_u16, put_u32, u16_at, u32_at, Error, MAX_TEMPLATE, PIC1_MATERIAL};

const MAGIC: &[u8; 8] = b"SGPOSKN1";
const NAME_SLOT: usize = 32;
pub(crate) const BRTI_LEN: usize = 0xA0;
const BRTI_STRIDE: usize = 0x2A8;
const PTR_ARRAY: usize = 0x198;
const STR_HEADER: usize = 0x14;
const BRTD_HEADER: usize = 0x10;
const RLT_LEN: usize = 0x10 + 2 * 24 + 8 * 8;
const DATA_ALIGN: usize = 0x1000;
/// Most textures a merged BNTX may hold: the canonical relocation table counts the texture pointers in a `u8`.
pub const MAX_TEXTURES: usize = 255;
/// Most skin textures a blob may carry.
pub const MAX_SKINS: usize = 64;

fn u64_at(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(d.get(at..at.checked_add(8)?)?.try_into().ok()?))
}

fn put_u64(d: &mut [u8], at: usize, v: u64) {
    if let Some(s) = d.get_mut(at..at + 8) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

/// Bytes one name takes in `_STR`: `u16` length, the bytes, a NUL, padded to an even size.
const fn str_size(len: usize) -> usize {
    2 + (len + 2) / 2 * 2
}

/// The embedded skins, checked once by [`SkinBlob::parse`].
#[derive(Clone, Copy)]
pub struct SkinBlob<'a> {
    pub count: usize,
    pub pic_len: usize,
    pub mat_len: usize,
    pub texmap: usize,
    pub data_len: usize,
    pub align: usize,
    names: &'a [u8],
    pics: &'a [u8],
    mats: &'a [u8],
    brtis: &'a [u8],
    zlib: &'a [u8],
}

impl<'a> SkinBlob<'a> {
    pub fn parse(b: &'a [u8]) -> Option<Self> {
        if b.get(0..8)? != MAGIC {
            return None;
        }
        let count = u16_at(b, 8)? as usize;
        let pic_len = u16_at(b, 10)? as usize;
        let mat_len = u16_at(b, 12)? as usize;
        let texmap = u16_at(b, 14)? as usize;
        let data_len = u32_at(b, 16)? as usize;
        let zlib_len = u32_at(b, 20)? as usize;
        let align = u32_at(b, 24)? as usize;
        let ok = (1..=MAX_SKINS).contains(&count)
            && (PIC1_MATERIAL + 2..=MAX_TEMPLATE).contains(&pic_len)
            && pic_len.is_multiple_of(4)
            && mat_len.is_multiple_of(4)
            && (28..mat_len.saturating_sub(1)).contains(&texmap)
            && align.is_power_of_two()
            && (4..=DATA_ALIGN).contains(&align);
        if !ok {
            return None;
        }
        let mut at = 0x20usize;
        let mut take = |n: usize| {
            let s = b.get(at..at.checked_add(n)?)?;
            at += n;
            Some(s)
        };
        let names = take(count * NAME_SLOT)?;
        let pics = take(count * pic_len)?;
        let mats = take(count * mat_len)?;
        let brtis = take(count * BRTI_LEN)?;
        let zlib = take(zlib_len)?;
        let blob = SkinBlob { count, pic_len, mat_len, texmap, data_len, align, names, pics, mats, brtis, zlib };
        let mut laid = 0usize;
        for k in 0..count {
            let name = blob.name(k);
            let pic = blob.pic(k);
            if name.len() <= 9 || !name.starts_with(b"tex_sgpo_") || name.len() >= NAME_SLOT {
                return None;
            }
            if pic.get(0..4)? != b"pic1" || u32_at(pic, 4)? as usize != pic_len || blob.pane_name(k).is_empty() {
                return None;
            }
            if blob.brti(k).get(0..4)? != b"BRTI" || blob.image_size(k) == 0 {
                return None;
            }
            laid = laid.next_multiple_of(align) + blob.image_size(k);
        }
        (laid == data_len).then_some(blob)
    }

    pub fn name(&self, k: usize) -> &'a [u8] {
        let slot = &self.names[k * NAME_SLOT..(k + 1) * NAME_SLOT];
        &slot[..slot.iter().position(|b| *b == 0).unwrap_or(NAME_SLOT)]
    }

    pub(crate) fn pic(&self, k: usize) -> &'a [u8] {
        &self.pics[k * self.pic_len..(k + 1) * self.pic_len]
    }

    /// The skin pane's name (`sgpo_alt_*`, `sgpo_gct_*`).
    pub fn pane_name(&self, k: usize) -> &'a [u8] {
        let raw = &self.pic(k)[0x0C..0x24];
        &raw[..raw.iter().position(|b| *b == 0).unwrap_or(raw.len())]
    }

    pub(crate) fn mat(&self, k: usize) -> &'a [u8] {
        &self.mats[k * self.mat_len..(k + 1) * self.mat_len]
    }

    fn brti(&self, k: usize) -> &'a [u8] {
        &self.brtis[k * BRTI_LEN..(k + 1) * BRTI_LEN]
    }

    fn image_size(&self, k: usize) -> usize {
        u32_at(self.brti(k), 0x50).unwrap_or(0) as usize
    }

    fn names_len(&self) -> usize {
        (0..self.count).map(|k| self.name(k).len() + 1).sum()
    }

    /// How much the skins can add to an archive: three layouts' panes, materials and names, and the textures.
    pub fn max_growth(&self) -> usize {
        let layouts = 3 * (self.count * (self.pic_len + 4 + self.mat_len + 4) + self.names_len() + 8);
        let bntx = self.count * (8 + 16 + BRTI_STRIDE + NAME_SLOT + 4) + 2 * DATA_ALIGN + self.align + self.data_len;
        layouts + bntx + RLT_LEN
    }
}

/// Where everything in the BNTX is now and goes, worked out before anything moves.
#[derive(Clone, Copy)]
pub(crate) struct BntxPlan {
    pub member: usize,
    pub old_len: usize,
    pub new_len: usize,
    n: usize,
    str_off: usize,
    strings_end: usize,
    str_count: u32,
    brti0: usize,
    data: usize,
    data_len: usize,
    parent: u64,
    new_str_off: usize,
    new_dic: usize,
    new_brti0: usize,
    new_brtd: usize,
    new_first: usize,
    new_data_len: usize,
    new_rlt: usize,
}

/// Read the base BNTX and plan the append. Refuses anything but the canonical Smash layout (Nintendo's, or Toolbox's
/// after an append): one string pool, one dictionary, evenly spaced BRTIs, one BRTD, the relocation table after it.
pub(crate) fn plan_bntx(b: &[u8], member: usize, skins: &SkinBlob) -> Result<BntxPlan, Error> {
    let bad = Error::SkinBntx;
    let version = u32_at(b, 8).ok_or(bad)?;
    if b.get(0..4) != Some(b"BNTX")
        || !(version == 0x0004_0000 || version == 0x0004_0100)
        || u16_at(b, 0x0C) != Some(0xFEFF)
        || b.get(0x0E) != Some(&12)
        || b.get(0x0F) != Some(&64)
        || b.get(0x20..0x24) != Some(b"NX  ")
        || u32_at(b, 0x1C).map(|v| v as usize) != Some(b.len())
    {
        return Err(bad);
    }
    let n = u32_at(b, 0x24).ok_or(bad)? as usize;
    if n == 0 || n + skins.count > MAX_TEXTURES || u64_at(b, 0x28) != Some(PTR_ARRAY as u64) {
        return Err(bad);
    }
    let str_off = u16_at(b, 0x16).ok_or(bad)? as usize;
    if str_off != PTR_ARRAY + 8 * n || b.get(str_off..str_off + 4) != Some(b"_STR") {
        return Err(bad);
    }
    let str_count = u32_at(b, str_off + 0x10).ok_or(bad)?;
    let mut at = str_off + STR_HEADER;
    for _ in 0..=str_count {
        let len = u16_at(b, at).ok_or(bad)? as usize;
        at += str_size(len);
    }
    let strings_end = at;
    let dic = u64_at(b, 0x38).ok_or(bad)? as usize;
    if dic != strings_end.next_multiple_of(8) || b.get(dic..dic + 4) != Some(b"_DIC") || u32_at(b, dic + 4) != Some(n as u32) {
        return Err(bad);
    }
    let brti0 = dic + 8 + 16 * (n + 1);
    let brtd = u64_at(b, 0x30).ok_or(bad)? as usize;
    if b.get(brtd..brtd + 4) != Some(b"BRTD") || brtd < brti0 + n * BRTI_STRIDE {
        return Err(bad);
    }
    let data = brtd + BRTD_HEADER;
    let data_len = (u64_at(b, brtd + 8).ok_or(bad)? as usize).checked_sub(BRTD_HEADER).ok_or(bad)?;
    let rlt = u32_at(b, 0x18).ok_or(bad)? as usize;
    if rlt != data + data_len || b.get(rlt..rlt + 4) != Some(b"_RLT") {
        return Err(bad);
    }
    let fname = u32_at(b, 0x10).ok_or(bad)? as usize;
    if !(str_off + STR_HEADER..strings_end).contains(&fname) {
        return Err(bad);
    }
    for i in 0..n {
        let t = brti0 + i * BRTI_STRIDE;
        let name = u64_at(b, t + 0x60).ok_or(bad)? as usize;
        let pixels = u64_at(b, t + BRTI_STRIDE - 8).ok_or(bad)? as usize;
        if u64_at(b, PTR_ARRAY + 8 * i) != Some(t as u64)
            || b.get(t..t + 4) != Some(b"BRTI")
            || u64_at(b, t + 0x70) != Some((t + BRTI_STRIDE - 8) as u64)
            || !(str_off + STR_HEADER..strings_end).contains(&name)
            || !(data..rlt).contains(&pixels)
        {
            return Err(bad);
        }
        let key = key_at(b, name as u32);
        if (0..skins.count).any(|k| skins.name(k) == key) {
            return Err(Error::SkinBntx);
        }
    }
    let parent = u64_at(b, brti0 + 0x68).ok_or(bad)?;

    let m = skins.count;
    let total = n + m;
    let new_str_off = PTR_ARRAY + 8 * total;
    let new_names: usize = (0..m).map(|k| str_size(skins.name(k).len())).sum();
    let new_strings_end = new_str_off + (strings_end - str_off) + new_names;
    let new_dic = new_strings_end.next_multiple_of(8);
    let new_brti0 = new_dic + 8 + 16 * (total + 1);
    let new_brtd = (new_brti0 + total * BRTI_STRIDE + BRTD_HEADER).next_multiple_of(DATA_ALIGN) - BRTD_HEADER;
    let new_first = data_len.next_multiple_of(skins.align);
    let new_data_len = new_first + skins.data_len;
    let new_rlt = new_brtd + BRTD_HEADER + new_data_len;
    if new_str_off > u16::MAX as usize {
        return Err(bad);
    }
    Ok(BntxPlan {
        member,
        old_len: b.len(),
        new_len: new_rlt + RLT_LEN,
        n,
        str_off,
        strings_end,
        str_count,
        brti0,
        data,
        data_len,
        parent,
        new_str_off,
        new_dic,
        new_brti0,
        new_brtd,
        new_first,
        new_data_len,
        new_rlt,
    })
}

fn key_at(x: &[u8], at: u32) -> &[u8] {
    let at = at as usize;
    let len = u16_at(x, at).unwrap_or(0) as usize;
    x.get(at + 2..at + 2 + len).unwrap_or(&[])
}

/// One `_DIC` node while the trie is built.
#[derive(Clone, Copy, Default)]
pub(crate) struct Node {
    bit: i32,
    child: [u16; 2],
    parent: u16,
    key: u32,
}

/// The work space the skins need, kept off the loading thread's stack (the plugin owns one).
pub struct SkinScratch {
    nodes: [Node; MAX_TEXTURES + 1],
    inflate: DecompressorOxide,
}

impl Default for SkinScratch {
    fn default() -> Self {
        SkinScratch { nodes: [Node::default(); MAX_TEXTURES + 1], inflate: DecompressorOxide::new() }
    }
}

fn bit_at(k: &[u8], idx: u32) -> u8 {
    let idx = idx as usize;
    if idx >= k.len() * 8 {
        return 0;
    }
    (k[k.len() - 1 - idx / 8] >> (idx % 8)) & 1
}

fn mismatch(a: &[u8], b: &[u8]) -> i32 {
    let max = (a.len().max(b.len()) * 8) as u32;
    (0..max).find(|&i| bit_at(a, i) != bit_at(b, i)).unwrap_or(max) as i32
}

fn first_one(k: &[u8]) -> i32 {
    (0..(k.len() * 8) as u32).find(|&i| bit_at(k, i) == 1).unwrap_or(0) as i32
}

fn branch(nodes: &[Node], n: usize, key: &[u8]) -> usize {
    match nodes[n].bit {
        b if b < 0 => 0,
        b => bit_at(key, b as u32) as usize,
    }
}

/// Insert `key` (a `_STR` entry at `key_off`) as node `new`: Toolbox-Cli's `dict_builder::Trie::insert` (itself
/// Switch-Toolbox's `ResDict`), which reproduces Nintendo's `_DIC` when names go in texture order.
fn insert(nodes: &mut [Node], new: usize, x: &[u8], key_off: u32) {
    let key = key_at(x, key_off);
    let mut current = 0usize;
    if nodes[0].child[0] != 0 {
        let mut node = nodes[0].child[0] as usize;
        loop {
            let prev = node;
            node = nodes[node].child[branch(nodes, node, key)] as usize;
            if nodes[node].bit <= nodes[prev].bit {
                current = prev;
                break;
            }
        }
    }
    let bit = mismatch(key_at(x, nodes[current].key), key);
    let mut t = current;
    while bit < nodes[nodes[t].parent as usize].bit {
        t = nodes[t].parent as usize;
    }
    let me = new as u16;
    let target_bit = nodes[t].bit;
    if bit < target_bit {
        let p = nodes[t].parent as usize;
        let mut child = [me; 2];
        child[(bit_at(key, bit as u32) ^ 1) as usize] = t as u16;
        nodes[new] = Node { bit, child, parent: p as u16, key: key_off };
        let pb = branch(nodes, p, key);
        nodes[p].child[pb] = me;
        nodes[t].parent = me;
    } else if bit > target_bit {
        let key_bit = bit_at(key, bit as u32);
        let bt = branch(nodes, t, key);
        let displaced = nodes[t].child[bt] as usize;
        let mut child = [me; 2];
        child[(key_bit ^ 1) as usize] =
            if bit_at(key_at(x, nodes[displaced].key), bit as u32) == key_bit ^ 1 { displaced as u16 } else { 0 };
        nodes[new] = Node { bit, child, parent: t as u16, key: key_off };
        nodes[t].child[bt] = me;
    } else {
        let b = branch(nodes, t, key);
        let displaced = nodes[t].child[b] as usize;
        let split = if displaced == 0 { first_one(key) } else { mismatch(key_at(x, nodes[displaced].key), key) };
        let mut child = [me; 2];
        child[(bit_at(key, split as u32) ^ 1) as usize] = displaced as u16;
        nodes[new] = Node { bit: split, child, parent: t as u16, key: key_off };
        nodes[t].child[b] = me;
    }
}

/// Write the `_DIC` at `dic` for the textures whose BRTIs start at `brti0`, in texture order.
fn write_dic(x: &mut [u8], nodes: &mut [Node], dic: usize, empty: usize, brti0: usize, total: usize) {
    nodes[0] = Node { bit: -1, child: [0, 0], parent: 0, key: empty as u32 };
    for i in 0..total {
        let key = u64_at(x, brti0 + i * BRTI_STRIDE + 0x60).unwrap_or(0) as u32;
        insert(nodes, i + 1, x, key);
    }
    x[dic..dic + 4].copy_from_slice(b"_DIC");
    put_u32(x, dic + 4, total as u32);
    for (i, node) in nodes.iter().enumerate().take(total + 1) {
        let at = dic + 8 + 16 * i;
        put_u32(x, at, node.bit as u32);
        put_u16(x, at + 4, node.child[0]);
        put_u16(x, at + 6, node.child[1]);
        put_u64(x, at + 8, u64::from(node.key));
    }
}

/// Toolbox-Cli's canonical relocation table (`build_canonical_reloc_table`): 2 sections, 8 entries.
fn write_rlt(x: &mut [u8], at: usize, total: usize, dic: usize, brti0: usize, brtd: usize, data_len: usize) {
    x[at..at + RLT_LEN].fill(0);
    x[at..at + 4].copy_from_slice(b"_RLT");
    put_u32(x, at + 4, at as u32);
    put_u32(x, at + 8, 2);
    let sections = [(0usize, brti0 + total * BRTI_STRIDE, 0u32, 6u32), (brtd, BRTD_HEADER + data_len, 6, 2)];
    for (k, (position, size, index, count)) in sections.iter().enumerate() {
        let s = at + 0x10 + 24 * k;
        put_u32(x, s + 8, *position as u32);
        put_u32(x, s + 12, *size as u32);
        put_u32(x, s + 16, *index);
        put_u32(x, s + 20, *count);
    }
    let stride = (BRTI_STRIDE / 8) as u8;
    let total16 = total as u16;
    let entries: [(usize, u16, u8, u8); 8] = [
        (0x28, 1, 1, 0),
        (0x38, 1, 2, 0),
        (PTR_ARRAY, 1, total as u8, 0),
        (dic + 0x10, total16 + 1, 1, 1),
        (brti0 + 0x60, total16, 3, stride - 3),
        (brti0 + 0x80, total16, 2, stride - 2),
        (0x30, 1, 1, 0),
        (brti0 + BRTI_STRIDE - 8, total16, 1, stride - 1),
    ];
    for (k, (position, structs, offsets, padding)) in entries.iter().enumerate() {
        let e = at + 0x10 + 2 * 24 + 8 * k;
        put_u32(x, e, *position as u32);
        put_u16(x, e + 4, *structs);
        x[e + 6] = *offsets;
        x[e + 7] = *padding;
    }
}

/// Fill in a BRTI's size and pointer fields for its slot at `t`.
fn place_brti(x: &mut [u8], t: usize, block: usize, name: usize, pixels: usize) {
    put_u32(x, t + 4, block as u32);
    put_u64(x, t + 8, block as u64);
    put_u64(x, t + 0x60, name as u64);
    put_u64(x, t + 0x70, (t + BRTI_STRIDE - 8) as u64);
    put_u64(x, t + 0x80, (t + BRTI_LEN) as u64);
    put_u64(x, t + 0x88, (t + BRTI_LEN + 0x100) as u64);
    put_u64(x, t + BRTI_STRIDE - 8, pixels as u64);
}

/// Append the skins' textures to the BNTX in `x[..p.old_len]` (`x` is `p.new_len` long). Everything only moves
/// forward, so each block is moved before anything is written over where it was. Fails only if the embedded pixel
/// data does not inflate, after which `x` is partly rewritten.
pub(crate) fn append(x: &mut [u8], p: &BntxPlan, skins: &SkinBlob, scratch: &mut SkinScratch) -> Result<(), Error> {
    let total = p.n + skins.count;
    let new_data = p.new_brtd + BRTD_HEADER;
    let delta_str = p.new_str_off - p.str_off;

    // Pixel data: Nintendo's (or another mod's) moved, then the skins' inflated after it.
    x.copy_within(p.data..p.data + p.data_len, new_data);
    x[new_data + p.data_len..new_data + p.new_first].fill(0);
    let out = &mut x[new_data + p.new_first..new_data + p.new_data_len];
    scratch.inflate.init();
    let flags = TINFL_FLAG_PARSE_ZLIB_HEADER | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
    let (status, _, written) = decompress(&mut scratch.inflate, skins.zlib, out, 0, flags);
    if status != TINFLStatus::Done || written != skins.data_len {
        return Err(Error::SkinData);
    }
    write_rlt(x, p.new_rlt, total, p.new_dic, p.new_brti0, p.new_brtd, p.new_data_len);
    x[p.new_brtd..p.new_brtd + 4].copy_from_slice(b"BRTD");
    put_u32(x, p.new_brtd + 4, 0);
    put_u64(x, p.new_brtd + 8, (BRTD_HEADER + p.new_data_len) as u64);

    // The base's BRTIs, last first.
    let last = total - 1;
    let block = |i: usize| if i == last { p.new_brtd - (p.new_brti0 + i * BRTI_STRIDE) } else { BRTI_STRIDE };
    for i in (0..p.n).rev() {
        let (src, dst) = (p.brti0 + i * BRTI_STRIDE, p.new_brti0 + i * BRTI_STRIDE);
        let name = u64_at(x, src + 0x60).unwrap_or(0) as usize + delta_str;
        let pixels = u64_at(x, src + BRTI_STRIDE - 8).unwrap_or(0) as usize - p.data + new_data;
        x.copy_within(src..src + BRTI_STRIDE, dst);
        place_brti(x, dst, block(i), name, pixels);
    }
    // The base's strings, then the skins' names after them.
    x.copy_within(p.str_off + STR_HEADER..p.strings_end, p.new_str_off + STR_HEADER);
    let mut at = p.strings_end + delta_str;
    let mut pixels = new_data + p.new_first;
    for k in 0..skins.count {
        let name = skins.name(k);
        let t = p.new_brti0 + (p.n + k) * BRTI_STRIDE;
        x[t..t + BRTI_LEN].copy_from_slice(skins.brti(k));
        x[t + BRTI_LEN..t + BRTI_STRIDE].fill(0);
        put_u64(x, t + 0x68, p.parent);
        pixels = pixels.next_multiple_of(skins.align);
        place_brti(x, t, block(p.n + k), at, pixels);
        pixels += skins.image_size(k);
        let size = str_size(name.len());
        x[at..at + size].fill(0);
        put_u16(x, at, name.len() as u16);
        x[at + 2..at + 2 + name.len()].copy_from_slice(name);
        at += size;
    }
    let brti_end = p.new_brti0 + total * BRTI_STRIDE;
    x[brti_end..p.new_brtd].fill(0);
    x[at..p.new_dic].fill(0);
    let s = p.new_str_off;
    x[s..s + 4].copy_from_slice(b"_STR");
    put_u32(x, s + 4, (p.new_brti0 - s) as u32);
    put_u64(x, s + 8, (p.new_brti0 - s) as u64);
    put_u32(x, s + 0x10, p.str_count + skins.count as u32);
    write_dic(x, &mut scratch.nodes, p.new_dic, s + STR_HEADER, p.new_brti0, total);
    for i in 0..total {
        put_u64(x, PTR_ARRAY + 8 * i, (p.new_brti0 + i * BRTI_STRIDE) as u64);
    }
    let fname = u32_at(x, 0x10).unwrap_or(0) as usize + delta_str;
    put_u32(x, 0x10, fname as u32);
    put_u16(x, 0x16, p.new_str_off as u16);
    put_u32(x, 0x18, p.new_rlt as u32);
    put_u32(x, 0x1C, p.new_len as u32);
    put_u32(x, 0x24, total as u32);
    put_u64(x, 0x30, p.new_brtd as u64);
    put_u64(x, 0x38, p.new_dic as u64);
    Ok(())
}

/// Rebuild the `_DIC` of an unchanged BNTX in a copy and report whether it matches the original (a host check that
/// the trie port reproduces Nintendo's).
pub fn dic_matches(b: &[u8], copy: &mut [u8], scratch: &mut SkinScratch) -> bool {
    let (Some(n), Some(dic), Some(str_off)) = (u32_at(b, 0x24), u64_at(b, 0x38), u16_at(b, 0x16)) else { return false };
    let (n, dic) = (n as usize, dic as usize);
    if copy.len() != b.len() || n > MAX_TEXTURES {
        return false;
    }
    copy.copy_from_slice(b);
    let brti0 = dic + 8 + 16 * (n + 1);
    copy[dic..brti0].fill(0);
    write_dic(copy, &mut scratch.nodes, dic, str_off as usize + STR_HEADER, brti0, n);
    copy == b
}
