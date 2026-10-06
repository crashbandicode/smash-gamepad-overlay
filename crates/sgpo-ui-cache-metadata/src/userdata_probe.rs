//! Heap-free predicate and line formatter for the ext-user-data diagnostic.
//!
//! `ext_userdata_is_suspect` is a heuristic for values that cannot be a mapped,
//! aligned Horizon user pointer. It is not proof that a value is invalid, and
//! a value that fails the predicate is not proof that the pointer is live.

/// Lowest address treated as the mapped user range. `0x0800_0004` is inside
/// that range and 4-byte aligned, so the predicate leaves it alone.
pub const MAPPED_USER_BASE: u64 = 0x0800_0000;

/// Value loaded from `pane+0xa8` on the post-results Count fault.
pub const KNOWN_UNMAPPED_EXT_USER_DATA: u64 = 0x0200_0000;

/// Name bytes copied from `pane+0xb0`. Longer input is truncated.
pub const NAME_OCTETS: usize = 25;

/// Stack buffer large enough for one startup line or one getter record.
pub const PROBE_LINE_CAP: usize = 384;

const _: () = assert!(PROBE_LINE_CAP >= 192);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeRecordKind {
    Sample,
    Suspect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeRecord<'a> {
    pub kind: ProbeRecordKind,
    pub pane: u64,
    pub lr_text: u64,
    pub vtable_text: u64,
    pub parent: u64,
    pub flags: u8,
    pub flag_ex: u8,
    pub flag_word: u32,
    pub ext_user_data: u64,
    pub name: &'a [u8],
    pub thread: u64,
}

pub fn ext_userdata_is_suspect(value: u64) -> bool {
    value != 0 && (value < MAPPED_USER_BASE || value & 3 != 0)
}

pub fn format_probe_startup(
    buf: &mut [u8],
    build_id: &str,
    display_version: &str,
    main_base: u64,
) -> usize {
    let mut out = BufWriter::new(buf);
    out.push_str("probe startup build=");
    out.push_text(build_id.as_bytes());
    out.push_str(" version=");
    out.push_text(display_version.as_bytes());
    out.push_str(" main=");
    out.push_hex_u64(main_base);
    out.push_str("\n");
    out.pos
}

pub fn format_probe_record(buf: &mut [u8], record: &ProbeRecord<'_>) -> usize {
    let mut out = BufWriter::new(buf);
    out.push_str("probe ");
    out.push_str(match record.kind {
        ProbeRecordKind::Sample => "sample",
        ProbeRecordKind::Suspect => "suspect",
    });
    out.push_str(" pane=");
    out.push_hex_u64(record.pane);
    out.push_str(" lr=");
    out.push_hex_u64(record.lr_text);
    out.push_str(" vtable=");
    out.push_hex_u64(record.vtable_text);
    out.push_str(" parent=");
    out.push_hex_u64(record.parent);
    out.push_str(" flags=");
    out.push_hex_u8(record.flags);
    out.push_str(" flagex=");
    out.push_hex_u8(record.flag_ex);
    out.push_str(" word=");
    out.push_hex_u32(record.flag_word);
    out.push_str(" ext=");
    out.push_hex_u64(record.ext_user_data);
    out.push_str(" tag=");
    out.push_str(tag(record));
    out.push_str(" name=");
    let name = &record.name[..record.name.len().min(NAME_OCTETS)];
    out.push_hex_bytes(name);
    out.push_str(" thread=");
    out.push_hex_u64(record.thread);
    out.push_str("\n");
    out.pos
}

fn tag(record: &ProbeRecord<'_>) -> &'static str {
    match record.kind {
        ProbeRecordKind::Sample => "sample",
        ProbeRecordKind::Suspect if record.ext_user_data == KNOWN_UNMAPPED_EXT_USER_DATA => {
            "suspect,exact_0x02000000"
        }
        ProbeRecordKind::Suspect => "suspect",
    }
}

struct BufWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> BufWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn push_str(&mut self, text: &str) {
        self.push_bytes(text.as_bytes());
    }

    fn push_text(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            let rendered = if byte.is_ascii_graphic() || byte == b' ' {
                byte
            } else {
                b'?'
            };
            self.push_bytes(&[rendered]);
        }
    }

    fn push_hex_u64(&mut self, value: u64) {
        self.push_str("0x");
        self.push_hex_width(&value.to_be_bytes());
    }

    fn push_hex_u32(&mut self, value: u32) {
        self.push_str("0x");
        self.push_hex_width(&value.to_be_bytes());
    }

    fn push_hex_u8(&mut self, value: u8) {
        self.push_str("0x");
        self.push_hex_width(&[value]);
    }

    fn push_hex_bytes(&mut self, bytes: &[u8]) {
        self.push_hex_width(bytes);
    }

    fn push_hex_width(&mut self, bytes: &[u8]) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for &byte in bytes {
            self.push_bytes(&[HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0xf)]]);
        }
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        let room = self.buf.len().saturating_sub(self.pos);
        let count = bytes.len().min(room);
        self.buf[self.pos..self.pos + count].copy_from_slice(&bytes[..count]);
        self.pos += count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample<'a>(kind: ProbeRecordKind, ext: u64, name: &'a [u8]) -> ProbeRecord<'a> {
        ProbeRecord {
            kind,
            pane: 0x1000,
            lr_text: 0x5f540,
            vtable_text: 0x4f3f810,
            parent: 0x2000,
            flags: 0x11,
            flag_ex: 0x04,
            flag_word: 0,
            ext_user_data: ext,
            name,
            thread: 0x42,
        }
    }

    #[test]
    fn predicate_rejects_null_and_aligned_mapped_values() {
        assert!(!ext_userdata_is_suspect(0));
        assert!(!ext_userdata_is_suspect(MAPPED_USER_BASE));
        assert!(!ext_userdata_is_suspect(0x0800_0004));
        assert!(!ext_userdata_is_suspect(0x0045_c46f_6450));
    }

    #[test]
    fn predicate_flags_known_unmapped_and_unaligned_values() {
        assert!(ext_userdata_is_suspect(KNOWN_UNMAPPED_EXT_USER_DATA));
        assert!(ext_userdata_is_suspect(0x0800_0001));
        assert!(ext_userdata_is_suspect(1));
    }

    #[test]
    fn formatter_keeps_invalid_name_bytes_as_hex() {
        let name = [0xff, 0x00, 0x7f, 0x20, 0x41];
        let mut buf = [0u8; PROBE_LINE_CAP];
        let len = format_probe_record(&mut buf, &sample(ProbeRecordKind::Sample, 0x0800_0004, &name));
        let line = std::str::from_utf8(&buf[..len]).expect("formatter emits ascii");
        assert!(line.ends_with('\n'));
        assert!(line.contains("tag=sample"));
        assert!(line.contains("name=ff007f2041"));
        assert!(!line.contains("suspect"));
        assert!(line.contains("ext=0x0000000008000004"));
    }

    #[test]
    fn formatter_tags_the_known_fault_value_without_decoding_name() {
        let mut name = [0u8; NAME_OCTETS];
        name[0] = 0xff;
        name[24] = 0x80;
        let mut buf = [0u8; PROBE_LINE_CAP];
        let len = format_probe_record(
            &mut buf,
            &sample(ProbeRecordKind::Suspect, KNOWN_UNMAPPED_EXT_USER_DATA, &name),
        );
        let line = std::str::from_utf8(&buf[..len]).expect("formatter emits ascii");
        assert!(line.contains("tag=suspect,exact_0x02000000"));
        assert!(line.contains("ext=0x0000000002000000"));
        assert!(line.contains("ff"));
        assert!(line.contains("80"));
        assert!(line.len() < PROBE_LINE_CAP);
    }

    #[test]
    fn formatter_truncates_to_the_caller_buffer_and_the_name_limit() {
        let mut tiny = [0u8; 16];
        let len = format_probe_record(
            &mut tiny,
            &sample(ProbeRecordKind::Sample, KNOWN_UNMAPPED_EXT_USER_DATA, b"abc"),
        );
        assert_eq!(len, tiny.len());

        let mut long_name = [b'A'; 40];
        long_name[25] = 0xff;
        let mut buf = [0u8; PROBE_LINE_CAP];
        let len = format_probe_record(&mut buf, &sample(ProbeRecordKind::Sample, 0x0800_0000, &long_name));
        let line = std::str::from_utf8(&buf[..len]).expect("formatter emits ascii");
        let name = line.split("name=").nth(1).unwrap().split(' ').next().unwrap();
        assert_eq!(name.len(), NAME_OCTETS * 2);
        assert!(name.chars().all(|ch| ch == '4' || ch == '1'));
        assert!(!line.contains("ff"));
    }

    #[test]
    fn startup_line_fits_and_replaces_non_graphic_build_bytes() {
        let mut buf = [0u8; PROBE_LINE_CAP];
        let len = format_probe_startup(&mut buf, "abc\n\u{0}def", "13.0.5", 0x8000_0000);
        let line = std::str::from_utf8(&buf[..len]).expect("formatter emits ascii");
        assert_eq!(line.matches('\n').count(), 1);
        assert!(line.ends_with('\n'));
        assert!(line.contains("build=abc??def"));
        assert!(line.contains("version=13.0.5"));
        assert!(line.contains("main=0x0000000080000000"));

        let mut tiny = [0u8; 8];
        assert_eq!(format_probe_startup(&mut tiny, "build-id", "13.0.5", 1), 8);
    }
}
