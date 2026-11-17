//! `merge_in_place` on synthetic archives built here (no game data): placement, alignment, idempotence, refusals,
//! and no panic on any truncated input.

use sgpo_layout_merge::*;

fn section(magic: &[u8; 4], body_len: usize) -> Vec<u8> {
    let mut s = vec![0u8; 8 + body_len];
    s[0..4].copy_from_slice(magic);
    s[4..8].copy_from_slice(&((8 + body_len) as u32).to_le_bytes());
    s
}

fn pane(magic: &[u8; 4], name: &str, len: usize, material: u16) -> Vec<u8> {
    let mut s = section(magic, len - 8);
    s[0x0C..0x0C + name.len()].copy_from_slice(name.as_bytes());
    s[0x0A] = 230;
    s[0x4C..0x50].copy_from_slice(&256f32.to_le_bytes());
    if magic == b"pic1" {
        s[0x64..0x66].copy_from_slice(&material.to_le_bytes());
        s[0x54..0x58].copy_from_slice(&0xdead_beefu32.to_le_bytes());
    }
    s
}

fn flyt(sections: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = sections.concat();
    let mut f = vec![0u8; 0x14];
    f[0..4].copy_from_slice(b"FLYT");
    f[4..6].copy_from_slice(&0xFEFFu16.to_le_bytes());
    f[6..8].copy_from_slice(&0x14u16.to_le_bytes());
    f[8..12].copy_from_slice(&0x0900_0000u32.to_le_bytes());
    f[0x0C..0x10].copy_from_slice(&((0x14 + body.len()) as u32).to_le_bytes());
    f[0x10..0x12].copy_from_slice(&(sections.len() as u16).to_le_bytes());
    f.extend(body);
    f
}

/// A layout shaped like 13.0.5's: RootPane with a picture child, a nested pane with its own children, then a group.
fn layout(extra_root_children: &[Vec<u8>]) -> Vec<u8> {
    let mut s = vec![
        section(b"lyt1", 0x20),
        section(b"txl1", 0x10),
        section(b"mat1", 0x40),
        pane(b"pan1", "RootPane", 0x54, 0),
        section(b"pas1", 0),
        pane(b"pic1", "set_rep_01", 0x88, 1),
        pane(b"pan1", "nested", 0x54, 0),
        section(b"pas1", 0),
        pane(b"pic1", "set_rep_stock_01", 0x88, 13),
        section(b"pae1", 0),
    ];
    s.extend(extra_root_children.iter().cloned());
    s.push(section(b"pae1", 0));
    s.push(section(b"grp1", 0x20));
    flyt(&s)
}

fn sarc(members: &[(&str, Vec<u8>, usize)]) -> Vec<u8> {
    let count = members.len();
    let mut names = Vec::new();
    let mut name_off = Vec::new();
    for (name, _, _) in members {
        name_off.push(names.len() / 4);
        names.extend(name.as_bytes());
        names.push(0);
        while names.len() % 4 != 0 {
            names.push(0);
        }
    }
    let sfnt = 0x20 + 16 * count;
    let data_off = (sfnt + 8 + names.len()).next_multiple_of(0x2000);
    let mut data = Vec::new();
    let mut ranges = Vec::new();
    for (_, bytes, align) in members {
        while !(data_off + data.len()).is_multiple_of(*align) {
            data.push(0);
        }
        let start = data.len();
        data.extend(bytes);
        ranges.push((start, data.len()));
    }
    let mut out = vec![0u8; data_off];
    out[0..4].copy_from_slice(b"SARC");
    out[4..6].copy_from_slice(&0x14u16.to_le_bytes());
    out[6..8].copy_from_slice(&0xFEFFu16.to_le_bytes());
    out[0x0C..0x10].copy_from_slice(&(data_off as u32).to_le_bytes());
    out[0x10..0x12].copy_from_slice(&0x0100u16.to_le_bytes());
    out[0x14..0x18].copy_from_slice(b"SFAT");
    out[0x18..0x1A].copy_from_slice(&0x0Cu16.to_le_bytes());
    out[0x1A..0x1C].copy_from_slice(&(count as u16).to_le_bytes());
    out[0x1C..0x20].copy_from_slice(&0x65u32.to_le_bytes());
    for i in 0..count {
        let n = 0x20 + 16 * i;
        out[n..n + 4].copy_from_slice(&(i as u32 * 7919).to_le_bytes());
        out[n + 4..n + 8].copy_from_slice(&(0x0100_0000 | name_off[i] as u32).to_le_bytes());
        out[n + 8..n + 12].copy_from_slice(&(ranges[i].0 as u32).to_le_bytes());
        out[n + 12..n + 16].copy_from_slice(&(ranges[i].1 as u32).to_le_bytes());
    }
    out[sfnt..sfnt + 4].copy_from_slice(b"SFNT");
    out[sfnt + 4..sfnt + 6].copy_from_slice(&8u16.to_le_bytes());
    out[sfnt + 8..sfnt + 8 + names.len()].copy_from_slice(&names);
    out.extend(data);
    let len = out.len() as u32;
    out[8..12].copy_from_slice(&len.to_le_bytes());
    out
}

fn archive() -> Vec<u8> {
    let filler: Vec<u8> = (0..5000u32).map(|i| (i * 31 % 251) as u8).collect();
    let texture: Vec<u8> = (0..9000u32).map(|i| (i * 17 % 253) as u8).collect();
    sarc(&[
        ("blyt/other_a.bflyt", filler.clone(), 16),
        (PLAYER_PARTS_BFLYTS[0], layout(&[]), 64),
        ("bgsh/__ArchiveShader.bnsh", filler.clone(), 0x1000),
        (PLAYER_PARTS_BFLYTS[1], layout(&[]), 16),
        ("timg/__Combined.bntx", texture, 0x1000),
        (ROOT_BFLYT, layout(&[]), 16),
        ("anim/x.bflan", filler, 8),
    ])
}

struct Entry {
    name: String,
    start: usize,
    end: usize,
}

fn entries(d: &[u8]) -> Vec<Entry> {
    let data_off = u32::from_le_bytes(d[0x0C..0x10].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(d[0x1A..0x1C].try_into().unwrap()) as usize;
    let names = 0x20 + 16 * count + 8;
    (0..count)
        .map(|i| {
            let n = 0x20 + 16 * i;
            let word = |o: usize| u32::from_le_bytes(d[n + o..n + o + 4].try_into().unwrap()) as usize;
            let at = names + (word(4) & 0xFF_FFFF) * 4;
            let len = d[at..].iter().position(|b| *b == 0).unwrap();
            Entry {
                name: String::from_utf8(d[at..at + len].to_vec()).unwrap(),
                start: data_off + word(8),
                end: data_off + word(12),
            }
        })
        .collect()
}

fn member<'a>(d: &'a [u8], name: &str) -> &'a [u8] {
    let e = entries(d).into_iter().find(|e| e.name == name).unwrap();
    &d[e.start..e.end]
}

/// (magic, name, depth, offset) for every section.
fn walk(f: &[u8]) -> Vec<(String, String, i32, usize)> {
    let count = u16::from_le_bytes(f[0x10..0x12].try_into().unwrap());
    let mut off = 0x14;
    let mut depth = 0;
    let mut out = Vec::new();
    for _ in 0..count {
        let magic = String::from_utf8(f[off..off + 4].to_vec()).unwrap();
        let size = u32::from_le_bytes(f[off + 4..off + 8].try_into().unwrap()) as usize;
        if magic == "pae1" {
            depth -= 1;
        }
        let name = if ["pan1", "pic1"].contains(&magic.as_str()) {
            let raw = &f[off + 0x0C..off + 0x24];
            String::from_utf8(raw[..raw.iter().position(|b| *b == 0).unwrap_or(24)].to_vec()).unwrap()
        } else {
            String::new()
        };
        out.push((magic.clone(), name, depth, off));
        if magic == "pas1" {
            depth += 1;
        }
        off += size;
    }
    assert_eq!(off, f.len(), "sections cover the file");
    out
}

fn f32_at(s: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(s[at..at + 4].try_into().unwrap())
}

fn merged(input: &[u8]) -> (Vec<u8>, Merged) {
    let mut buf = input.to_vec();
    buf.resize(input.len() + MAX_GROWTH, 0xAA);
    let m = merge_in_place(&mut buf, input.len()).expect("merge");
    buf.truncate(m.len);
    (buf, m)
}

#[test]
fn hash40_of_the_layout_path() {
    // python3: (len(p) << 32) | zlib.crc32(p)
    assert_eq!(LAYOUT_HASH40, 0x2f_26ff_8cc2);
}

#[test]
fn panes_become_the_last_children_of_root_pane() {
    let input = archive();
    let (out, m) = merged(&input);
    assert_eq!((m.patched, m.already), (3, 0));
    for target in TARGETS {
        let f = member(&out, target.member);
        assert_eq!(u32::from_le_bytes(f[0x0C..0x10].try_into().unwrap()) as usize, f.len());
        let w = walk(f);
        let root = w.iter().position(|s| s.1 == SGPO_ROOT).unwrap();
        assert_eq!(w[root].2, 1, "sgpo_root is a child of RootPane");
        assert_eq!(w[root + 1].0, "pas1");
        let kids: Vec<_> = w[root + 2..root + 2 + PANE_SPECS.len()].iter().collect();
        assert_eq!(w[root + 2 + PANE_SPECS.len()].0, "pae1");
        assert_eq!(w[root + 3 + PANE_SPECS.len()], ("pae1".into(), String::new(), 0, w[root + 3 + PANE_SPECS.len()].3));
        let s = &f[w[root].3..];
        assert_eq!((f32_at(s, 0x2C), f32_at(s, 0x30), f32_at(s, 0x4C), f32_at(s, 0x50)), (760.0, -330.0, 360.0, 300.0));
        assert_eq!((s[0x08] & 1, s[0x0A]), (1, 0));
        for (spec, kid) in PANE_SPECS.iter().zip(kids) {
            assert_eq!((kid.0.as_str(), kid.1.as_str(), kid.2), ("pic1", spec.name, 2));
            let s = &f[kid.3..kid.3 + 0x88];
            assert_eq!((f32_at(s, 0x2C), f32_at(s, 0x30), f32_at(s, 0x4C), f32_at(s, 0x50)), (spec.x, spec.y, spec.width, spec.height));
            assert_eq!((s[0x08] & 1, s[0x0A]), (1, 0));
            assert_eq!(u32::from_le_bytes(s[0x54..0x58].try_into().unwrap()), 0xdead_beef, "template body kept");
            // Root: cloned from set_rep_stock_01 (13) as is. Player parts: cloned from set_rep_01 (1), bound to
            // set_rep_stock_01's material (13).
            assert_eq!(u16::from_le_bytes(s[0x64..0x66].try_into().unwrap()), 13);
        }
    }
}

#[test]
fn other_members_are_unchanged_and_keep_their_alignment() {
    let input = archive();
    let (out, _) = merged(&input);
    assert_eq!(u32::from_le_bytes(out[8..12].try_into().unwrap()) as usize, out.len());
    let before = entries(&input);
    let after = entries(&out);
    for (b, a) in before.iter().zip(&after) {
        assert_eq!(b.name, a.name);
        assert!(a.start >= b.start);
        let align = (1usize << b.start.trailing_zeros()).min(0x2000);
        assert_eq!(a.start % align, 0, "{} keeps its alignment", a.name);
        if !TARGETS.iter().any(|t| t.member == b.name) {
            assert_eq!(&input[b.start..b.end], &out[a.start..a.end], "{} unchanged", a.name);
        }
    }
    let mut spans: Vec<_> = after.iter().map(|e| (e.start, e.end)).collect();
    spans.sort();
    assert!(spans.windows(2).all(|w| w[0].1 <= w[1].0), "members do not overlap");
}

#[test]
fn a_merged_archive_is_left_alone() {
    let (once, _) = merged(&archive());
    let (twice, m) = merged(&once);
    assert_eq!((m.patched, m.already, m.len), (0, 3, once.len()));
    assert_eq!(once, twice);
}

#[test]
fn another_mods_extra_panes_survive() {
    let mut extra = pane(b"pic1", "other_mod_badge", 0x88, 2);
    extra[0x2C..0x30].copy_from_slice(&123f32.to_le_bytes());
    let input = sarc(&[
        (PLAYER_PARTS_BFLYTS[0], layout(&[extra.clone()]), 16),
        (PLAYER_PARTS_BFLYTS[1], layout(&[]), 16),
        (ROOT_BFLYT, layout(&[]), 16),
    ]);
    let (out, m) = merged(&input);
    assert_eq!(m.patched, 3);
    let f = member(&out, PLAYER_PARTS_BFLYTS[0]);
    let w = walk(f);
    let badge = w.iter().position(|s| s.1 == "other_mod_badge").unwrap();
    let root = w.iter().position(|s| s.1 == SGPO_ROOT).unwrap();
    assert!(badge < root && w[badge].2 == 1);
    assert_eq!(&f[w[badge].3..w[badge].3 + 0x88], &extra[..]);
}

#[test]
fn refusals_leave_a_reason() {
    let partial = sarc(&[
        (PLAYER_PARTS_BFLYTS[0], layout(&[pane(b"pan1", SGPO_ROOT, 0x54, 0)]), 16),
        (PLAYER_PARTS_BFLYTS[1], layout(&[]), 16),
        (ROOT_BFLYT, layout(&[]), 16),
    ]);
    let missing = sarc(&[(PLAYER_PARTS_BFLYTS[0], layout(&[]), 16), (ROOT_BFLYT, layout(&[]), 16)]);
    let no_template = sarc(&[
        (PLAYER_PARTS_BFLYTS[0], flyt(&[pane(b"pan1", "RootPane", 0x54, 0), section(b"pas1", 0), section(b"pae1", 0)]), 16),
        (PLAYER_PARTS_BFLYTS[1], layout(&[]), 16),
        (ROOT_BFLYT, layout(&[]), 16),
    ]);
    for (input, want) in [
        (partial, Error::PartialPatch(PLAYER_PARTS_BFLYTS[0])),
        (missing, Error::MemberMissing(PLAYER_PARTS_BFLYTS[1])),
        (no_template, Error::TemplateMissing(PLAYER_PARTS_BFLYTS[0])),
        (b"not an archive at all".to_vec(), Error::NotSarc),
    ] {
        let mut buf = input.clone();
        buf.resize(input.len() + MAX_GROWTH, 0);
        assert_eq!(merge_in_place(&mut buf, input.len()), Err(want));
    }
    let input = archive();
    let mut small = input.clone();
    small.resize(input.len() + 100, 0);
    assert!(matches!(merge_in_place(&mut small, input.len()), Err(Error::BufferTooSmall { .. })));
}

#[test]
fn truncated_or_corrupted_input_never_panics() {
    let input = archive();
    let mut buf = vec![0u8; input.len() + MAX_GROWTH];
    for cut in (0..input.len()).step_by(13) {
        buf[..cut].copy_from_slice(&input[..cut]);
        let _ = merge_in_place(&mut buf, cut);
    }
    for at in (0..input.len()).step_by(29) {
        let mut bad = input.clone();
        bad[at] ^= 0xFF;
        buf[..bad.len()].copy_from_slice(&bad);
        let _ = merge_in_place(&mut buf, bad.len());
    }
}
