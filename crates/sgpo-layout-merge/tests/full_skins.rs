//! The textured skins on real files, which stay outside the repository. Skipped unless these are set:
//! `SGPO_STOCK_INFO_MELEE` (an extracted 13.0.5 `layout.arc`), `SGPO_SKIN_BLOB` (`tools/skin_blob.py`'s output) and
//! `SGPO_FULL_INFO_MELEE` (the installer's layout patched with both RetroSpy skins, the blob's source).

use sgpo_layout_merge::{dic_matches, merge_with, Merged, Scratch, SkinBlob, SkinScratch, MAX_GROWTH};

fn file(var: &str) -> Option<Vec<u8>> {
    match std::env::var(var) {
        Ok(path) => Some(std::fs::read(path).expect(var)),
        Err(_) => {
            eprintln!("skipped: {var} is not set");
            None
        }
    }
}

fn members(d: &[u8]) -> Vec<(String, std::ops::Range<usize>)> {
    let count = u16::from_le_bytes([d[0x1A], d[0x1B]]) as usize;
    let data = u32::from_le_bytes(d[0x0C..0x10].try_into().unwrap()) as usize;
    let names = 0x20 + 16 * count + 8;
    let word = |at: usize| u32::from_le_bytes(d[at..at + 4].try_into().unwrap()) as usize;
    (0..count)
        .map(|i| {
            let node = 0x20 + 16 * i;
            let at = names + (word(node + 4) & 0xFF_FFFF) * 4;
            let end = at + d[at..].iter().position(|b| *b == 0).unwrap();
            (String::from_utf8(d[at..end].to_vec()).unwrap(), data + word(node + 8)..data + word(node + 12))
        })
        .collect()
}

fn merge(base: &[u8], blob: &SkinBlob) -> (Vec<u8>, Merged) {
    let mut buf = base.to_vec();
    buf.resize(base.len() + MAX_GROWTH + blob.max_growth(), 0);
    let mut scratch = Box::<Scratch>::default();
    let merged = merge_with(&mut buf, base.len(), blob, &mut scratch).expect("merge");
    buf.truncate(merged.len);
    (buf, merged)
}

#[test]
fn the_trie_port_rebuilds_the_stock_dictionary() {
    let Some(stock) = file("SGPO_STOCK_INFO_MELEE") else { return };
    let (_, range) = members(&stock).into_iter().find(|(n, _)| n == "timg/__Combined.bntx").unwrap();
    let bntx = &stock[range];
    let mut copy = vec![0; bntx.len()];
    assert!(dic_matches(bntx, &mut copy, &mut Box::<SkinScratch>::default()));
}

/// The card's layout came from an older installer that also reset the default panes' scale to 1.0 in the player
/// parts; today's installer and the merge keep the template's (`src/visual.rs` sets every SGPO pane's scale each
/// frame, so it is never drawn). Set those fields to 1.0 so everything else can be compared byte for byte.
fn reset_default_scale(arc: &mut [u8]) {
    for (name, range) in members(arc) {
        if !name.starts_with("blyt/info_melee_lct_player_") {
            continue;
        }
        let flyt = &mut arc[range];
        let mut at = u16::from_le_bytes([flyt[6], flyt[7]]) as usize;
        for _ in 0..u16::from_le_bytes([flyt[0x10], flyt[0x11]]) {
            let size = u32::from_le_bytes(flyt[at + 4..at + 8].try_into().unwrap()) as usize;
            if &flyt[at..at + 4] == b"pic1" && flyt[at + 0x0C..at + 0x24].starts_with(b"sgpo_pro_") {
                flyt[at + 0x44..at + 0x48].copy_from_slice(&1.0f32.to_le_bytes());
                flyt[at + 0x48..at + 0x4C].copy_from_slice(&1.0f32.to_le_bytes());
            }
            at += size;
        }
    }
}

#[test]
fn stock_plus_the_skins_reproduces_the_installer_layout() {
    let (Some(stock), Some(blob), Some(full)) =
        (file("SGPO_STOCK_INFO_MELEE"), file("SGPO_SKIN_BLOB"), file("SGPO_FULL_INFO_MELEE"))
    else {
        return;
    };
    let blob = SkinBlob::parse(&blob).expect("blob");
    let (mut merged, outcome) = merge(&stock, &blob);
    reset_default_scale(&mut merged);
    assert_eq!((outcome.patched, outcome.already, outcome.skins, outcome.textures, outcome.skin_error), (3, 0, 3, 36, 0));
    let (want, got) = (members(&full), members(&merged));
    assert_eq!(want.len(), got.len());
    let differ: Vec<_> = want
        .iter()
        .zip(&got)
        .filter(|(w, g)| w.0 != g.0 || full[w.1.clone()] != merged[g.1.clone()])
        .map(|(w, g)| format!("{} ({} vs {} B)", w.0, w.1.len(), g.1.len()))
        .collect();
    if !differ.is_empty() {
        std::fs::write(std::env::temp_dir().join("sgpo-full-merged.arc"), &merged).unwrap();
    }
    assert!(differ.is_empty(), "members differ: {differ:?}");
    // Member placement differs: the merge keeps each member's original alignment, Toolbox packs them.
    eprintln!("all {} members identical; archive {} B (installer {} B)", got.len(), merged.len(), full.len());
}

#[test]
fn an_installed_skin_layout_is_left_as_it_is() {
    let (Some(blob), Some(full)) = (file("SGPO_SKIN_BLOB"), file("SGPO_FULL_INFO_MELEE")) else { return };
    let blob = SkinBlob::parse(&blob).expect("blob");
    let (merged, outcome) = merge(&full, &blob);
    assert_eq!((outcome.patched, outcome.already, outcome.textures), (0, 3, 0));
    assert!(merged == full);
}
