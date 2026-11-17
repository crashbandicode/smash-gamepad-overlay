//! SGPO's panes, spliced into the `info_melee` layout archive the game is loading.
//!
//! The installer (`tools/sgpo_installer`) and the plugin share the pane table below. The installer writes a whole
//! patched `layout.arc` on the PC; the plugin instead takes the archive the game is about to load (vanilla, or
//! another mod's through ARCropolis) and adds the same panes to it in place, so the SD card needs no SGPO
//! `layout.arc` and another mod's changes to `info_melee` survive.
//!
//! [`merge_in_place`] never allocates and never panics: every read is bounds-checked, and any surprise returns an
//! [`Error`] so the caller hands the game the archive unchanged.

#![no_std]

mod skins;

pub use skins::{dic_matches, SkinBlob, SkinScratch, MAX_SKINS, MAX_TEXTURES};
use skins::BntxPlan;

pub const LAYOUT_PATH: &str = "ui/layout/info/info_melee/info_melee/layout.arc";
/// The Hash40 ARCropolis keys the file by: CRC-32 of the path, its length in the top byte.
pub const LAYOUT_HASH40: u64 = hash40(LAYOUT_PATH);
/// `ui/layout/info/info_melee/info_melee/layout.arc` in the 13.0.5 `data.arc`.
pub const VANILLA_LEN_13_0_5: usize = 2_166_040;

pub const ROOT_BFLYT: &str = "blyt/info_melee.bflyt";
pub const PLAYER_PARTS_BFLYTS: [&str; 2] = [
    "blyt/info_melee_lct_player_00.bflyt",
    "blyt/info_melee_lct_player_01.bflyt",
];
pub const SGPO_ROOT: &str = "sgpo_root";
pub const SGPO_ROOT_POS: (f32, f32, f32) = (760.0, -330.0, 0.0);
pub const SGPO_ROOT_SIZE: (f32, f32) = (360.0, 300.0);
pub const INITIAL_PANE_ALPHA: u8 = 0;
pub const ROOT_MARKER_TEMPLATE: &str = "set_rep_stock_01";
pub const PLAYER_MARKER_TEMPLATE: &str = "set_rep_01";
pub const PLAYER_MATERIAL_SOURCE_PANE: &str = "set_rep_stock_01";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaneSpec {
    pub name: &'static str,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PaneSpec {
    pub const fn new(name: &'static str, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { name, x, y, width, height }
    }
}

/// The default square skins' panes, children of `sgpo_root` in each target layout.
pub const PANE_SPECS: [PaneSpec; 24] = [
    PaneSpec::new("sgpo_pro_lt", -145.0, 122.0, 54.0, 20.0),
    PaneSpec::new("sgpo_pro_lb", -145.0, 95.0, 54.0, 20.0),
    PaneSpec::new("sgpo_pro_rt", 65.0, 122.0, 54.0, 20.0),
    PaneSpec::new("sgpo_pro_rb", 65.0, 95.0, 54.0, 20.0),
    PaneSpec::new("sgpo_pro_minus", -38.0, 45.0, 20.0, 20.0),
    PaneSpec::new("sgpo_pro_plus", 20.0, 45.0, 20.0, 20.0),
    PaneSpec::new("sgpo_pro_l3", -68.0, -30.0, 18.0, 18.0),
    PaneSpec::new("sgpo_pro_r3", 62.0, -100.0, 18.0, 18.0),
    PaneSpec::new("sgpo_pro_ls_gate", -105.0, -30.0, 58.0, 58.0),
    PaneSpec::new("sgpo_pro_ls_dot", -105.0, -30.0, 14.0, 14.0),
    PaneSpec::new("sgpo_pro_rs_gate", 30.0, -100.0, 58.0, 58.0),
    PaneSpec::new("sgpo_pro_rs_dot", 30.0, -100.0, 14.0, 14.0),
    PaneSpec::new("sgpo_pro_du", -105.0, -78.0, 16.0, 16.0),
    PaneSpec::new("sgpo_pro_dd", -105.0, -128.0, 16.0, 16.0),
    PaneSpec::new("sgpo_pro_dl", -130.0, -103.0, 16.0, 16.0),
    PaneSpec::new("sgpo_pro_dr", -80.0, -103.0, 16.0, 16.0),
    PaneSpec::new("sgpo_pro_dul", -130.0, -78.0, 13.0, 13.0),
    PaneSpec::new("sgpo_pro_dur", -80.0, -78.0, 13.0, 13.0),
    PaneSpec::new("sgpo_pro_ddl", -130.0, -128.0, 13.0, 13.0),
    PaneSpec::new("sgpo_pro_ddr", -80.0, -128.0, 13.0, 13.0),
    PaneSpec::new("sgpo_pro_btn_y", 15.0, -20.0, 24.0, 24.0),
    PaneSpec::new("sgpo_pro_btn_x", 45.0, 10.0, 24.0, 24.0),
    PaneSpec::new("sgpo_pro_btn_b", 45.0, -50.0, 24.0, 24.0),
    PaneSpec::new("sgpo_pro_a_marker", 75.0, -20.0, 28.0, 28.0),
];

/// One layout that receives the panes: its member, the pane the markers are cloned from, and (player parts only)
/// the pane whose material the markers take.
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub member: &'static str,
    pub marker_template: &'static str,
    pub material_source: Option<&'static str>,
}

pub const TARGETS: [Target; 3] = [
    Target { member: ROOT_BFLYT, marker_template: ROOT_MARKER_TEMPLATE, material_source: None },
    Target {
        member: PLAYER_PARTS_BFLYTS[0],
        marker_template: PLAYER_MARKER_TEMPLATE,
        material_source: Some(PLAYER_MATERIAL_SOURCE_PANE),
    },
    Target {
        member: PLAYER_PARTS_BFLYTS[1],
        marker_template: PLAYER_MARKER_TEMPLATE,
        material_source: Some(PLAYER_MATERIAL_SOURCE_PANE),
    },
];

/// The largest pane section the splice copies (a `pic1` with up to 8 UV sets is 0x68 + 8 * 32 bytes).
const MAX_TEMPLATE: usize = 0x200;
/// Members are moved forward keeping their original alignment, capped here (BNTX and BNSH need 0x1000).
const MAX_ALIGN: usize = 0x2000;
/// Up to this many members (13.0.5's `info_melee` has 344). With it the merge needs about 16 KiB of stack.
const MAX_MEMBERS: usize = 768;
const PAN1_LEN: usize = 0x54;
const PANE_NAME: core::ops::Range<usize> = 0x0C..0x24;
const PIC1_MATERIAL: usize = 0x64;
const CHILD_SECTIONS: u16 = PANE_SPECS.len() as u16 + 3;

/// What the caller must reserve beyond the base archive: three layouts' panes, and alignment padding for every
/// member after them. A base archive within `buffer.len() - MAX_GROWTH` always fits.
pub const MAX_GROWTH: usize = 3 * (PAN1_LEN + 16 + PANE_SPECS.len() * MAX_TEMPLATE) + 4 * MAX_ALIGN + 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NotSarc,
    SarcBounds,
    TooManyMembers,
    MemberMissing(&'static str),
    NotFlyt(&'static str),
    FlytBounds(&'static str),
    PartialPatch(&'static str),
    NoRootPane(&'static str),
    TemplateMissing(&'static str),
    TemplateUnsupported(&'static str),
    MaterialMissing(&'static str),
    BufferTooSmall { needed: usize, available: usize },
    SkinTable(&'static str),
    SkinBntx,
    SkinData,
}

impl Error {
    /// A short stable code for logs and the published outcome.
    pub const fn code(self) -> u8 {
        match self {
            Error::NotSarc => 1,
            Error::SarcBounds => 2,
            Error::TooManyMembers => 3,
            Error::MemberMissing(_) => 4,
            Error::NotFlyt(_) => 5,
            Error::FlytBounds(_) => 6,
            Error::PartialPatch(_) => 7,
            Error::NoRootPane(_) => 8,
            Error::TemplateMissing(_) => 9,
            Error::TemplateUnsupported(_) => 10,
            Error::MaterialMissing(_) => 11,
            Error::BufferTooSmall { .. } => 12,
            Error::SkinTable(_) => 13,
            Error::SkinBntx => 14,
            Error::SkinData => 15,
        }
    }

    pub const fn describe(code: u8) -> &'static str {
        match code {
            1 => "not a SARC archive",
            2 => "SARC header or member out of bounds",
            3 => "too many SARC members",
            4 => "a target layout is missing",
            5 => "a target member is not a FLYT layout",
            6 => "a FLYT section is out of bounds",
            7 => "a target layout has only some of SGPO's panes",
            8 => "a target layout has no RootPane child block",
            9 => "a template pane is missing",
            10 => "a template pane is not the expected kind or size",
            11 => "the material source pane is missing",
            12 => "the merged archive does not fit the buffer",
            13 => "a target layout's txl1/mat1 cannot take the skins",
            14 => "timg/__Combined.bntx is not a BNTX the skins can be added to",
            15 => "the embedded skin textures did not inflate",
            _ => "unknown",
        }
    }
}

/// What [`merge_in_place`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Merged {
    /// The archive's new length.
    pub len: usize,
    /// Layouts that received the panes.
    pub patched: u8,
    /// Layouts that already had all of them (an older patched `layout.arc` still installed as a mod).
    pub already: u8,
    /// Layouts that also received the textured skins' panes and materials.
    pub skins: u8,
    /// Skin textures added to `timg/__Combined.bntx`.
    pub textures: u16,
    /// Why the skins were left out ([`Error::code`]), 0 if they went in or none were offered.
    pub skin_error: u8,
}

/// The merge's work space with the skins: the member table and the skins' trie and inflater, about 30 KiB. The
/// plugin keeps one off the loading thread's stack.
pub struct Scratch {
    members: [Member; MAX_MEMBERS],
    order: [u16; MAX_MEMBERS],
    skins: SkinScratch,
}

impl Default for Scratch {
    fn default() -> Self {
        Scratch { members: [Member::default(); MAX_MEMBERS], order: [0; MAX_MEMBERS], skins: SkinScratch::default() }
    }
}

const BNTX_MEMBER: &str = "timg/__Combined.bntx";

pub const fn hash40(path: &str) -> u64 {
    let bytes = path.as_bytes();
    let mut crc = 0xFFFF_FFFFu32;
    let mut i = 0;
    while i < bytes.len() {
        crc ^= bytes[i] as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            bit += 1;
        }
        i += 1;
    }
    ((bytes.len() as u64) << 32) | (!crc) as u64
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

fn put_u16(d: &mut [u8], at: usize, v: u16) {
    if let Some(s) = d.get_mut(at..at + 2) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

fn put_u32(d: &mut [u8], at: usize, v: u32) {
    if let Some(s) = d.get_mut(at..at + 4) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

fn put_f32(d: &mut [u8], at: usize, v: f32) {
    put_u32(d, at, v.to_bits());
}

/// A SARC member: where its NUL-terminated name is and where its bytes are, now and after the merge (absolute
/// offsets). Kept to 16 bytes: the whole merge state lives on the stack of whatever thread loads the file.
#[derive(Clone, Copy, Default)]
struct Member {
    name_at: u32,
    start: u32,
    end: u32,
    new_start: u32,
}

impl Member {
    fn start(&self) -> usize {
        self.start as usize
    }
    fn end(&self) -> usize {
        self.end as usize
    }
    fn len(&self) -> usize {
        (self.end - self.start) as usize
    }
    fn new_start(&self) -> usize {
        self.new_start as usize
    }
    fn name<'a>(&self, d: &'a [u8], data_off: usize) -> Option<&'a [u8]> {
        let tail = d.get(self.name_at as usize..data_off)?;
        Some(&tail[..tail.iter().position(|b| *b == 0)?])
    }
}

struct Sarc {
    data_off: usize,
    count: usize,
}

fn read_sarc(d: &[u8], members: &mut [Member; MAX_MEMBERS]) -> Result<Sarc, Error> {
    if d.get(0..4) != Some(b"SARC") || u16_at(d, 6) != Some(0xFEFF) || d.get(0x14..0x18) != Some(b"SFAT") {
        return Err(Error::NotSarc);
    }
    if d.len() > u32::MAX as usize / 2 {
        return Err(Error::SarcBounds);
    }
    let data_off = u32_at(d, 0x0C).ok_or(Error::SarcBounds)? as usize;
    let count = u16_at(d, 0x1A).ok_or(Error::SarcBounds)? as usize;
    if count > MAX_MEMBERS {
        return Err(Error::TooManyMembers);
    }
    let sfnt = 0x20 + 16 * count;
    if d.get(sfnt..sfnt + 4) != Some(b"SFNT") {
        return Err(Error::SarcBounds);
    }
    let names = sfnt + 8;
    if data_off < names || data_off > d.len() {
        return Err(Error::SarcBounds);
    }
    for (i, member) in members.iter_mut().enumerate().take(count) {
        let node = 0x20 + 16 * i;
        let attr = u32_at(d, node + 4).ok_or(Error::SarcBounds)?;
        let start = data_off + u32_at(d, node + 8).ok_or(Error::SarcBounds)? as usize;
        let end = data_off + u32_at(d, node + 12).ok_or(Error::SarcBounds)? as usize;
        if attr & 0x0100_0000 == 0 || start > end || end > d.len() {
            return Err(Error::SarcBounds);
        }
        let name_at = names + (attr & 0xFF_FFFF) as usize * 4;
        *member = Member { name_at: name_at as u32, start: start as u32, end: end as u32, new_start: start as u32 };
        member.name(d, data_off).ok_or(Error::SarcBounds)?;
    }
    Ok(Sarc { data_off, count })
}

/// What one target layout needs, read before anything moves.
#[derive(Clone, Copy)]
struct Plan {
    member: usize,
    /// Offset (within the member) of RootPane's closing `pae1`: the new block goes there, as its last child.
    insert_at: usize,
    root_pane: usize,
    marker: usize,
    marker_len: usize,
    material: Option<u16>,
    skin: Option<SkinPlan>,
}

/// Where a layout's `txl1` and `mat1` are, for the skins' texture names and materials (offsets within the member).
#[derive(Clone, Copy)]
struct SkinPlan {
    txl_at: usize,
    txl_len: usize,
    txl_count: usize,
    /// End of the last texture name, from the section start: the skins' names go there.
    names_end: usize,
    txl_new: usize,
    mat_at: usize,
    mat_len: usize,
    mat_count: usize,
    pics: usize,
    mats: usize,
}

impl Plan {
    fn panes(&self) -> usize {
        PAN1_LEN + 8 + PANE_SPECS.len() * self.marker_len + self.skin.map_or(0, |s| s.pics) + 8
    }

    fn extra(&self) -> usize {
        self.panes() + self.skin.map_or(0, |s| s.mats + (s.txl_new - s.txl_len))
    }
}

fn skin_plan(
    flyt: &[u8],
    who: &'static str,
    txl: Option<(usize, usize)>,
    mat: Option<(usize, usize)>,
    insert_at: usize,
    s: &SkinBlob,
) -> Result<SkinPlan, Error> {
    let e = Error::SkinTable(who);
    let ((txl_at, txl_len), (mat_at, mat_len)) = (txl.ok_or(e)?, mat.ok_or(e)?);
    if txl_at >= mat_at || mat_at + mat_len > insert_at {
        return Err(e);
    }
    let txl_count = u16_at(flyt, txl_at + 8).ok_or(e)? as usize;
    let mat_count = u16_at(flyt, mat_at + 8).ok_or(e)? as usize;
    if txl_count + s.count > 0x7FFF || mat_count + s.count > 0x7FFF || 12 + 4 * txl_count > txl_len || 12 + 4 * mat_count > mat_len {
        return Err(e);
    }
    let txl = &flyt[txl_at..txl_at + txl_len];
    let mut names_end = 12 + 4 * txl_count;
    for i in 0..txl_count {
        let start = 12 + u32_at(txl, 12 + 4 * i).ok_or(e)? as usize;
        let name = txl.get(start..).ok_or(e)?;
        let name = &name[..name.iter().position(|b| *b == 0).ok_or(e)?];
        if (0..s.count).any(|k| s.name(k) == name) {
            return Err(e);
        }
        names_end = names_end.max(start + name.len() + 1);
    }
    for i in 0..mat_count {
        if u32_at(flyt, mat_at + 12 + 4 * i).ok_or(e)? as usize >= mat_len {
            return Err(e);
        }
    }
    let names: usize = (0..s.count).map(|k| s.name(k).len() + 1).sum();
    let txl_new = (names_end + 4 * s.count + names).next_multiple_of(4).max(txl_len + 4 * s.count);
    Ok(SkinPlan {
        txl_at,
        txl_len,
        txl_count,
        names_end,
        txl_new,
        mat_at,
        mat_len,
        mat_count,
        pics: s.count * s.pic_len,
        mats: s.count * (4 + s.mat_len),
    })
}

const PANE_MAGICS: [&[u8; 4]; 8] = [b"pan1", b"pic1", b"txt1", b"wnd1", b"prt1", b"bnd1", b"scr1", b"ali1"];

fn pane_name(section: &[u8]) -> Option<&[u8]> {
    let magic = section.get(0..4)?;
    if !PANE_MAGICS.iter().any(|m| &m[..] == magic) {
        return None;
    }
    let raw = section.get(PANE_NAME)?;
    Some(&raw[..raw.iter().position(|b| *b == 0).unwrap_or(raw.len())])
}

/// Walk a FLYT's sections: `f(offset, section)` for each. Fails on a bad header or a section out of bounds.
fn sections(flyt: &[u8], who: &'static str, mut f: impl FnMut(usize, &[u8])) -> Result<(), Error> {
    if flyt.get(0..4) != Some(b"FLYT") || u16_at(flyt, 4) != Some(0xFEFF) {
        return Err(Error::NotFlyt(who));
    }
    let mut off = u16_at(flyt, 6).ok_or(Error::FlytBounds(who))? as usize;
    let count = u16_at(flyt, 0x10).ok_or(Error::FlytBounds(who))?;
    for _ in 0..count {
        let size = u32_at(flyt, off + 4).ok_or(Error::FlytBounds(who))? as usize;
        let section = flyt.get(off..off.checked_add(size).ok_or(Error::FlytBounds(who))?).ok_or(Error::FlytBounds(who))?;
        if size < 8 || !size.is_multiple_of(4) {
            return Err(Error::FlytBounds(who));
        }
        f(off, section);
        off += size;
    }
    Ok(())
}

fn plan(flyt: &[u8], member: usize, target: &Target, skins: Option<&SkinBlob>) -> Result<Option<Plan>, Error> {
    let who = target.member;
    let wanted = |name: &[u8]| name == SGPO_ROOT.as_bytes() || PANE_SPECS.iter().any(|s| s.name.as_bytes() == name);
    let skin_pane = |name: &[u8]| skins.is_some_and(|s| (0..s.count).any(|k| s.pane_name(k) == name));
    let (mut present, mut skin_present, mut depth) = (0usize, 0usize, 0i32);
    let (mut root_pane, mut root_depth, mut after_root, mut insert_at) = (None, 0i32, false, None);
    let (mut marker, mut source, mut txl, mut mat) = (None, None, None, None);
    sections(flyt, who, |off, s| {
        let magic = &s[0..4];
        if magic == b"txl1" && txl.is_none() {
            txl = Some((off, s.len()));
        }
        if magic == b"mat1" && mat.is_none() {
            mat = Some((off, s.len()));
        }
        if magic == b"pae1" {
            depth -= 1;
            if root_pane.is_some() && insert_at.is_none() && depth == root_depth {
                insert_at = Some(off);
            }
        }
        if let Some(name) = pane_name(s) {
            if wanted(name) {
                present += 1;
            }
            if skin_pane(name) {
                skin_present += 1;
            }
            if name == b"RootPane" && magic == b"pan1" && root_pane.is_none() {
                root_pane = Some((off, s.len()));
                root_depth = depth;
                after_root = true;
                return;
            }
            if name == target.marker_template.as_bytes() && marker.is_none() {
                marker = Some((off, s.len(), magic == b"pic1"));
            }
            if target.material_source.is_some_and(|m| m.as_bytes() == name) && source.is_none() {
                source = Some((magic == b"pic1").then(|| u16_at(s, PIC1_MATERIAL)).flatten());
            }
        }
        if magic == b"pas1" {
            if after_root {
                root_depth = depth;
            }
            depth += 1;
        }
        after_root = false;
    })?;
    if present == PANE_SPECS.len() + 1 {
        return Ok(None);
    }
    if present != 0 || skin_present != 0 {
        return Err(Error::PartialPatch(who));
    }
    let (root_pane, root_len) = root_pane.ok_or(Error::NoRootPane(who))?;
    let insert_at = insert_at.ok_or(Error::NoRootPane(who))?;
    let (marker, marker_len, is_pic) = marker.ok_or(Error::TemplateMissing(who))?;
    if root_len != PAN1_LEN || !is_pic || !(PIC1_MATERIAL + 2..=MAX_TEMPLATE).contains(&marker_len) {
        return Err(Error::TemplateUnsupported(who));
    }
    let material = match target.material_source {
        None => None,
        Some(_) => Some(source.flatten().ok_or(Error::MaterialMissing(who))?),
    };
    let skin = skins.map(|s| skin_plan(flyt, who, txl, mat, insert_at, s)).transpose()?;
    Ok(Some(Plan { member, insert_at, root_pane, marker, marker_len, material, skin }))
}

fn set_pane(section: &mut [u8], name: &str, pos: (f32, f32, f32), size: (f32, f32)) {
    if let Some(slot) = section.get_mut(PANE_NAME) {
        slot.fill(0);
        slot[..name.len()].copy_from_slice(name.as_bytes());
    }
    if let Some(flags) = section.get_mut(0x08) {
        *flags |= 1;
    }
    if let Some(alpha) = section.get_mut(0x0A) {
        *alpha = INITIAL_PANE_ALPHA;
    }
    put_f32(section, 0x2C, pos.0);
    put_f32(section, 0x30, pos.1);
    put_f32(section, 0x34, pos.2);
    put_f32(section, 0x4C, size.0);
    put_f32(section, 0x50, size.1);
}

/// Splice the block into the member that now starts at `at` (`old_len` bytes, already moved there). The templates
/// were copied out before anything moved.
fn splice(
    buf: &mut [u8],
    at: usize,
    old_len: usize,
    p: &Plan,
    root: &[u8; PAN1_LEN],
    marker: &[u8; MAX_TEMPLATE],
    skins: Option<&SkinBlob>,
) {
    let extra = p.extra();
    let flyt = &mut buf[at..at + old_len + extra];
    let mut len = old_len + p.panes();
    flyt.copy_within(p.insert_at..old_len, p.insert_at + p.panes());
    let mut w = p.insert_at;
    flyt[w..w + PAN1_LEN].copy_from_slice(root);
    set_pane(&mut flyt[w..w + PAN1_LEN], SGPO_ROOT, SGPO_ROOT_POS, SGPO_ROOT_SIZE);
    w += PAN1_LEN;
    flyt[w..w + 4].copy_from_slice(b"pas1");
    put_u32(flyt, w + 4, 8);
    w += 8;
    for spec in &PANE_SPECS {
        let s = &mut flyt[w..w + p.marker_len];
        s.copy_from_slice(&marker[..p.marker_len]);
        set_pane(s, spec.name, (spec.x, spec.y, 0.0), (spec.width, spec.height));
        if let Some(material) = p.material {
            put_u16(s, PIC1_MATERIAL, material);
        }
        w += p.marker_len;
    }
    let skin = p.skin.zip(skins);
    if let Some((_, s)) = skin {
        for k in 0..s.count {
            flyt[w..w + s.pic_len].copy_from_slice(s.pic(k));
            put_u16(flyt, w + PIC1_MATERIAL, (p.skin.map_or(0, |sp| sp.mat_count) + k) as u16);
            w += s.pic_len;
        }
    }
    flyt[w..w + 4].copy_from_slice(b"pae1");
    put_u32(flyt, w + 4, 8);
    let mut added = CHILD_SECTIONS;
    if let Some((sp, s)) = skin {
        added += s.count as u16;
        let grow = 4 * s.count;
        // mat1 (before the panes): the offset table grows, the materials go at the section's end.
        let table = sp.mat_at + 12 + 4 * sp.mat_count;
        flyt.copy_within(table..len, table + grow);
        len += grow;
        for i in 0..sp.mat_count {
            let at = sp.mat_at + 12 + 4 * i;
            put_u32(flyt, at, u32_at(flyt, at).unwrap_or(0) + grow as u32);
        }
        let end = sp.mat_at + sp.mat_len + grow;
        let mats = s.count * s.mat_len;
        flyt.copy_within(end..len, end + mats);
        len += mats;
        for k in 0..s.count {
            put_u32(flyt, table + 4 * k, (sp.mat_len + grow + k * s.mat_len) as u32);
            let at = end + k * s.mat_len;
            flyt[at..at + s.mat_len].copy_from_slice(s.mat(k));
            put_u16(flyt, at + s.texmap, (sp.txl_count + k) as u16);
        }
        put_u32(flyt, sp.mat_at + 4, (sp.mat_len + grow + mats) as u32);
        put_u16(flyt, sp.mat_at + 8, (sp.mat_count + s.count) as u16);
        // txl1 (before mat1): the offset table grows, the names go after the last name.
        let table = sp.txl_at + 12 + 4 * sp.txl_count;
        flyt.copy_within(table..len, table + grow);
        len += grow;
        for i in 0..sp.txl_count {
            let at = sp.txl_at + 12 + 4 * i;
            put_u32(flyt, at, u32_at(flyt, at).unwrap_or(0) + grow as u32);
        }
        let end = sp.txl_at + sp.txl_len + grow;
        let more = sp.txl_new - sp.txl_len - grow;
        flyt.copy_within(end..len, end + more);
        len += more;
        let mut at = sp.txl_at + sp.names_end + grow;
        flyt[at..sp.txl_at + sp.txl_new].fill(0);
        for k in 0..s.count {
            put_u32(flyt, table + 4 * k, (at - sp.txl_at - 12) as u32);
            let name = s.name(k);
            flyt[at..at + name.len()].copy_from_slice(name);
            at += name.len() + 1;
        }
        put_u32(flyt, sp.txl_at + 4, sp.txl_new as u32);
        put_u16(flyt, sp.txl_at + 8, (sp.txl_count + s.count) as u16);
    }
    debug_assert_eq!(len, old_len + extra);
    let sections = u16_at(flyt, 0x10).unwrap_or(0).saturating_add(added);
    put_u16(flyt, 0x10, sections);
    put_u32(flyt, 0x0C, (old_len + extra) as u32);
}

fn align_of(offset: usize) -> usize {
    if offset == 0 { MAX_ALIGN } else { (1usize << offset.trailing_zeros()).min(MAX_ALIGN) }
}

/// Add SGPO's panes to the archive in `buf[..len]`, in place; `buf` must have room for the growth (see
/// [`MAX_GROWTH`]). On an error the buffer may be partly rewritten: hand the game the original archive instead.
pub fn merge_in_place(buf: &mut [u8], len: usize) -> Result<Merged, Error> {
    let mut members = [Member::default(); MAX_MEMBERS];
    let mut order = [0u16; MAX_MEMBERS];
    merge(buf, len, None, &mut members, &mut order, None)
}

/// [`merge_in_place`] plus the textured skins: their panes and materials in the three layouts and their textures in
/// `timg/__Combined.bntx`. If the base cannot take the skins (another mod reshaped those files), the default panes
/// go in alone and [`Merged::skin_error`] says why. `buf` must also have room for [`SkinBlob::max_growth`].
pub fn merge_with(buf: &mut [u8], len: usize, skins: &SkinBlob, scratch: &mut Scratch) -> Result<Merged, Error> {
    let Scratch { members, order, skins: work } = scratch;
    merge(buf, len, Some(skins), members, order, Some(work))
}

type Plans = ([Option<Plan>; 3], u8, Option<BntxPlan>);

fn plan_all(d: &[u8], sarc: &Sarc, members: &[Member], skins: Option<&SkinBlob>) -> Result<Plans, Error> {
    let find = |name: &'static str| {
        members.iter().position(|m| m.name(d, sarc.data_off) == Some(name.as_bytes())).ok_or(Error::MemberMissing(name))
    };
    let mut plans: [Option<Plan>; 3] = [None; 3];
    let mut already = 0u8;
    for (slot, target) in plans.iter_mut().zip(TARGETS.iter()) {
        let index = find(target.member)?;
        let m = members[index];
        *slot = plan(&d[m.start()..m.end()], index, target, skins)?;
        already += u8::from(slot.is_none());
    }
    let bntx = match skins {
        Some(s) if plans.iter().any(Option::is_some) => {
            let index = find(BNTX_MEMBER).map_err(|_| Error::SkinBntx)?;
            let m = members[index];
            Some(skins::plan_bntx(&d[m.start()..m.end()], index, s)?)
        }
        _ => None,
    };
    Ok((plans, already, bntx))
}

fn merge(
    buf: &mut [u8],
    len: usize,
    skins: Option<&SkinBlob>,
    members: &mut [Member; MAX_MEMBERS],
    order: &mut [u16; MAX_MEMBERS],
    mut work: Option<&mut SkinScratch>,
) -> Result<Merged, Error> {
    let available = buf.len();
    let d = buf.get(..len).ok_or(Error::SarcBounds)?;
    let sarc = read_sarc(d, members)?;
    let members = &mut members[..sarc.count];

    let skins = skins.filter(|_| work.is_some());
    let (mut skin_error, mut offered) = (0u8, skins);
    let (plans, already, bntx) = match skins.map(|s| plan_all(d, &sarc, members, Some(s))) {
        Some(Ok(plans)) => plans,
        Some(Err(e)) => {
            skin_error = e.code();
            offered = None;
            plan_all(d, &sarc, members, None)?
        }
        None => plan_all(d, &sarc, members, None)?,
    };
    if plans.iter().all(Option::is_none) {
        return Ok(Merged { len, patched: 0, already, skins: 0, textures: 0, skin_error });
    }

    // Data order, and each member's new start: never earlier than before (so moving back to front is safe), its
    // original alignment kept.
    let order = &mut order[..sarc.count];
    for (i, o) in order.iter_mut().enumerate() {
        *o = i as u16;
    }
    order.sort_unstable_by_key(|&i| members[i as usize].start);
    let grow = |i: usize| match plans.iter().flatten().find(|p| p.member == i) {
        Some(p) => p.extra(),
        None => bntx.filter(|b| b.member == i).map_or(0, |b| b.new_len - b.old_len),
    };
    let mut cursor = sarc.data_off;
    for &i in order.iter() {
        let m = &mut members[i as usize];
        let start = m.start().max(cursor.next_multiple_of(align_of(m.start())));
        cursor = start + m.len() + grow(i as usize);
        if cursor > available {
            return Err(Error::BufferTooSmall { needed: cursor, available });
        }
        m.new_start = start as u32;
    }
    let new_len = cursor.max(len);

    // Templates out first: every member keeps its original bytes until it is moved itself.
    let mut roots = [[0u8; PAN1_LEN]; 3];
    let mut markers = [[0u8; MAX_TEMPLATE]; 3];
    for (k, p) in plans.iter().enumerate() {
        if let Some(p) = p {
            let base = members[p.member].start();
            roots[k].copy_from_slice(&buf[base + p.root_pane..base + p.root_pane + PAN1_LEN]);
            markers[k][..p.marker_len].copy_from_slice(&buf[base + p.marker..base + p.marker + p.marker_len]);
        }
    }
    for &i in order.iter().rev() {
        let (i, m) = (i as usize, members[i as usize]);
        if m.new_start != m.start {
            buf.copy_within(m.start()..m.end(), m.new_start());
        }
        if let Some((k, p)) = plans.iter().enumerate().find_map(|(k, p)| p.filter(|p| p.member == i).map(|p| (k, p))) {
            splice(buf, m.new_start(), m.len(), &p, &roots[k], &markers[k], offered);
        }
        if let (Some(b), Some(s), Some(work)) = (bntx.filter(|b| b.member == i), offered, work.as_deref_mut()) {
            skins::append(&mut buf[m.new_start()..m.new_start() + b.new_len], &b, s, work)?;
        }
    }
    // Zero the padding between members, then rewrite the member table and the archive size.
    let mut prev_end = sarc.data_off;
    for &i in order.iter() {
        let m = members[i as usize];
        buf[prev_end..m.new_start()].fill(0);
        prev_end = m.new_start() + m.len() + grow(i as usize);
    }
    buf[prev_end..new_len].fill(0);
    for (i, m) in members.iter().enumerate() {
        let node = 0x20 + 16 * i;
        let start = m.new_start() - sarc.data_off;
        put_u32(buf, node + 8, start as u32);
        put_u32(buf, node + 12, (start + m.len() + grow(i)) as u32);
    }
    put_u32(buf, 0x08, new_len as u32);
    let patched = plans.iter().flatten().count() as u8;
    let skinned = plans.iter().flatten().filter(|p| p.skin.is_some()).count() as u8;
    let textures = if bntx.is_some() { offered.map_or(0, |s| s.count as u16) } else { 0 };
    Ok(Merged { len: new_len, patched, already, skins: skinned, textures, skin_error })
}
