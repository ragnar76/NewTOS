//! B2a — every raster in a resource `tools/mkrsc.py` emits must start on a
//! **word** (2-byte) boundary.
//!
//! A classic Atari `.rsc` gets this for free: strings live in `rsh_string`,
//! rasters in a separate, even-aligned `rsh_imdata` section, and every raster
//! is an even number of bytes (`bi_wb * bi_hl`, `((ib_wicon+15)/16)*2*ib_hicon`).
//! Every consumer reads them as 16-bit words — EmuTOS `gr_gblt(WORD *pimage,…)`
//! (`aes/gemgraf.c:631`), our `blit_plane` (`tos-api/src/aes/object.rs`), and
//! TeraDesk's own `xrsrc_load` alike.
//!
//! `mkrsc.py` interns strings **and** rasters into one `data` section, so
//! without explicit padding an odd-length string pushes the next raster onto an
//! odd offset. Measured on the shipped files before the fix: 36 of 76 raster
//! slots in `icons.rsc` (native-RSHDR) and 3 of 3 in `gem_fsel.nrsc` were odd.
//!
//! These tests drive the real tool over a synthetic classic `.rsc` whose string
//! lengths are chosen to force the misalignment, in all three output modes, and
//! check **every** raster slot — so a future change to the section layout is
//! caught, not just today's files.
//!
//! B5 — every TEDINFO string the tool emits must be followed by the **full
//! declared length**, not just by the bytes up to its NUL.
//!
//! `te_txtlen` is a buffer capacity, not a string length: an editable field
//! carries a short (often empty) resource text and a much larger `te_txtlen`,
//! and everything that writes the field bounds itself by `te_txtlen` — EmuTOS
//! `ob_edit` (`aes/gemobed.c:394,407,420,432`), `inf_sset`
//! (`util/optimize.c:111`), our `ins_char` (`tos-api/src/aes/object.rs`). A
//! classic `.rsc` therefore reserves those bytes behind `te_ptext`; measured
//! on `references/emutos/aes/gem.rsc` `ted[0]`: `te_ptext` 0x0c8b,
//! `te_ptmplt` 0x0cb2, **39** bytes apart == `te_txtlen` 39, 38 of them NUL.
//! Checked over all 13 TEDINFOs there, all 104 of TeraDesk's `desktop.rsc`
//! and all 118 of QED's `qed.rsc`: the gap equals `te_txtlen` every time.
//! EmuTOS' own resource compiler keeps the same contract (`tools/erd.c:3007`,
//! `te_ptext_len += te_txtlen`).
//!
//! `intern_str` used to copy only up to the NUL, so the shipped blobs were
//! short: `gem_fsel.nrsc` 11 of 13 TEDINFOs (worst case a 1-byte buffer
//! against `te_txtlen` 39), `desktop.rsc` 6 of 104, `qed.rsc` 0 of 118.
//!
//! Both invariants share one fixture on purpose: the reservation bytes land
//! in the very `data` section the rasters are aligned in, so the alignment
//! tests double as the guard that `RASTER_ALIGN` survives the reservation.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::workspace_root;

// ── classic (big-endian m68k) resource fixture ────────────────────────────

const CLS_OBJ: usize = 24;
const CLS_TED: usize = 28;
const CLS_IB: usize = 34;
const CLS_BB: usize = 14;

const G_BOX: u16 = 20;
const G_IMAGE: u16 = 23;
const G_ICON: u16 = 31;

const LASTOB: u16 = 0x0020;

fn be16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn be32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}

/// Build a minimal, structurally valid classic `.rsc` that exercises both
/// tool postconditions at once.
///
/// **Alignment (B2a):** the string lengths force the raster misalignment —
/// `ted[0]`'s template is 3 bytes with its NUL (odd) and `ib[0].ib_ptext` is
/// 5 bytes (odd), so with the strings and the rasters sharing one output
/// section, the first BITBLK raster and then `ib[1]`'s two rasters land on
/// odd offsets.
///
/// **Reservation (B5):** `ted[1]` is the shape every file-selector field in
/// `references/emutos/aes/gem.rsc` has — an **empty** text with a large
/// `te_txtlen` (there 39, here 9) — and the fixture reserves those bytes on
/// disk exactly as a classic file does. `ted[0]` is the well-formed shape
/// (`te_txtlen` == the placeholder's length, as all 118 of QED's are), which
/// must come out unchanged.
///
/// The two interact, which is why they share one fixture: the reservation
/// bytes land in the same `data` section the rasters are aligned in.
fn classic_fixture() -> Vec<u8> {
    // Section layout (classic files put image data in its own section).
    let off_obj = 36usize;
    let n_obj = 3usize;
    let off_ted = off_obj + n_obj * CLS_OBJ;
    let n_ted = 2usize;
    let off_ib = off_ted + n_ted * CLS_TED;
    let n_ib = 2usize;
    let off_bb = off_ib + n_ib * CLS_IB;
    let n_bb = 1usize;
    let off_frstr = off_bb + n_bb * CLS_BB;
    let n_frstr = 1usize;
    let off_frimg = off_frstr + n_frstr * 4;
    let n_frimg = 1usize;
    let off_tri = off_frimg + n_frimg * 4;
    let n_tree = 1usize;
    let off_str = off_tri + n_tree * 4;

    // Strings, in one run: (offset, bytes-with-NUL). Entry 4 is `ted[1]`'s
    // text: empty, but with the full `te_txtlen` of 9 reserved behind it —
    // what a classic `.rsc` writes for an editable field (`gem.rsc` ted[0]:
    // te_ptext 0x0c8b, te_ptmplt 0x0cb2, 39 bytes apart, 38 of them NUL).
    let strings: [&[u8]; 7] = [
        b"ABC\0",
        b"XY\0",
        b"ICON\0",
        b"B\0",
        b"\0\0\0\0\0\0\0\0\0",
        b"____\0",
        b"X\0",
    ];
    let mut str_off = Vec::new();
    let mut o = off_str;
    for s in strings {
        str_off.push(o);
        o += s.len();
    }
    // The image section starts on an even offset, as a classic RCS writes it.
    let off_img = o + (o & 1);

    // Rasters: one 16×2 BITBLK (bi_wb=2, bi_hl=2 → 4 bytes) and two 16×16
    // ICONBLK mask/data pairs (32 bytes each).
    let bb_len = 2 * 2usize;
    let ib_len = 32usize;
    let bb_data = off_img;
    let ib0_mask = bb_data + bb_len;
    let ib0_data = ib0_mask + ib_len;
    let ib1_mask = ib0_data + ib_len;
    let ib1_data = ib1_mask + ib_len;
    let img_end = ib1_data + ib_len;

    let mut d: Vec<u8> = Vec::new();

    // ── RSHDR (18 × u16, big-endian) ──
    for x in [
        0u16, // rsh_vrsn (old format, no CICON)
        off_obj as u16,
        off_ted as u16,
        off_ib as u16,
        off_bb as u16,
        off_frstr as u16,
        off_str as u16,
        off_img as u16,
        off_frimg as u16,
        off_tri as u16,
        n_obj as u16,
        n_tree as u16,
        n_ted as u16,
        n_ib as u16,
        n_bb as u16,
        n_frstr as u16,
        n_frimg as u16,
        img_end as u16, // rsh_rssize
    ] {
        be16(&mut d, x);
    }
    assert_eq!(d.len(), off_obj);

    // ── OBJECTs: root G_BOX, a G_ICON, a G_IMAGE (LASTOB) ──
    // ob_x/y/w/h are classic char-grid packed words; any value converts.
    let mut obj = |next: i16, head: i16, tail: i16, ty: u16, flags: u16, spec: u32| {
        be16(&mut d, next as u16);
        be16(&mut d, head as u16);
        be16(&mut d, tail as u16);
        be16(&mut d, ty);
        be16(&mut d, flags);
        be16(&mut d, 0); // ob_state
        be32(&mut d, spec);
        for _ in 0..4 {
            be16(&mut d, 2); // ob_x/ob_y/ob_width/ob_height (2 chars)
        }
    };
    obj(-1, 1, 2, G_BOX, 0, 0x0000_1100);
    obj(2, -1, -1, G_ICON, 0, off_ib as u32);
    obj(0, -1, -1, G_IMAGE, LASTOB, off_bb as u32);
    assert_eq!(d.len(), off_ted);

    // ── TEDINFOs ──
    // Field order (classic, big-endian): te_ptext, te_ptmplt, te_pvalid, then
    // te_font, te_fontid, te_just, te_color, te_fontsize, te_thickness,
    // te_txtlen, te_tmplen.
    //
    // ted[0] — the well-formed shape: te_ptext "ABC" (4 B, even) with
    // te_txtlen 4, te_ptmplt "XY" (3 B, ODD — this is what misaligns the
    // rasters) with te_tmplen 3. Nothing to reserve; must convert unchanged.
    be32(&mut d, str_off[0] as u32);
    be32(&mut d, str_off[1] as u32);
    be32(&mut d, str_off[1] as u32); // te_pvalid (interned once, shared)
    for x in [3u16, 0, 0, 1, 0, 0, 4, 3] {
        be16(&mut d, x);
    }
    // ted[1] — the B5 shape: an EMPTY text that declares te_txtlen 9, so the
    // converted blob must hold 9 bytes behind te_ptext even though the string
    // is one byte long. te_ptmplt "____" (te_tmplen 5) and a one-character
    // te_pvalid, which has no length word at all.
    be32(&mut d, str_off[4] as u32);
    be32(&mut d, str_off[5] as u32);
    be32(&mut d, str_off[6] as u32);
    for x in [3u16, 0, 0, 1, 0, 0, 9, 5] {
        be16(&mut d, x);
    }
    assert_eq!(d.len(), off_ib);

    // ── ICONBLKs: ib[0] with a 5-byte (ODD) label, ib[1] with a 2-byte one ──
    let mut iconblk = |mask: usize, data: usize, text: usize| {
        be32(&mut d, mask as u32);
        be32(&mut d, data as u32);
        be32(&mut d, text as u32);
        for x in [
            0x1000u16, // ib_char: fg 1 / bg 0, no overlay character
            0, 0, // ib_xchar, ib_ychar
            0, 0, // ib_xicon, ib_yicon
            16, 16, // ib_wicon, ib_hicon
            0, 16, 72, 8, // ib_xtext, ib_ytext, ib_wtext, ib_htext
        ] {
            be16(&mut d, x);
        }
    };
    iconblk(ib0_mask, ib0_data, str_off[2]); // "ICON\0" = 5 bytes, odd
    iconblk(ib1_mask, ib1_data, str_off[3]); // "B\0"    = 2 bytes
    assert_eq!(d.len(), off_bb);

    // ── BITBLK ──
    be32(&mut d, bb_data as u32);
    for x in [2u16, 2, 0, 0, 1] {
        be16(&mut d, x); // bi_wb, bi_hl, bi_x, bi_y, bi_color
    }
    assert_eq!(d.len(), off_frstr);

    // ── free-string / free-image tables, tree index ──
    be32(&mut d, str_off[0] as u32);
    assert_eq!(d.len(), off_frimg);
    be32(&mut d, off_bb as u32); // a free image IS a BITBLK
    assert_eq!(d.len(), off_tri);
    be32(&mut d, off_obj as u32); // tree 0 root
    assert_eq!(d.len(), off_str);

    // ── string data, then the (even-aligned) image data ──
    for s in strings {
        d.extend_from_slice(s);
    }
    while d.len() < off_img {
        d.push(0);
    }
    // Recognisable raster content; the value is irrelevant to alignment.
    for i in 0..(img_end - off_img) {
        d.push((i as u8) ^ 0xA5);
    }
    assert_eq!(d.len(), img_end);
    d
}

// ── readers for the two native output formats ─────────────────────────────

fn le16(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}

fn le32(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

fn le64(d: &[u8], o: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[o..o + 8]);
    u64::from_le_bytes(b)
}

/// Every raster slot (`bi_pdata`, `ib_pmask`, `ib_pdata`) in a native `.rsc`,
/// as `(name, byte offset from the blob base)`.
fn raster_slots(d: &[u8]) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    if &d[..4] == b"NRSC" {
        // NRSC v5 (LP64) / v6 (ILP32): magic + 16 × u32.
        let f = |i: usize| le32(d, 4 + i * 4) as usize;
        let version = f(0);
        let (off_ib, n_ib, off_bb, n_bb) = (f(12), f(13), f(14), f(15));
        let (nat_ib, nat_bb, ptr) = if version == 5 {
            (48usize, 24usize, 8usize)
        } else {
            (36, 16, 4)
        };
        let read = |o: usize| {
            if ptr == 8 {
                le64(d, o)
            } else {
                le32(d, o) as u64
            }
        };
        for i in 0..n_bb {
            out.push((format!("bb[{i}].bi_pdata"), read(off_bb + i * nat_bb)));
        }
        for i in 0..n_ib {
            let o = off_ib + i * nat_ib;
            out.push((format!("ib[{i}].ib_pmask"), read(o)));
            out.push((format!("ib[{i}].ib_pdata"), read(o + ptr)));
        }
    } else {
        // native-RSHDR: 18 × u16 little-endian, LP64 structs.
        let h = |i: usize| le16(d, i * 2) as usize;
        let (off_ib, off_bb, n_ib, n_bb) = (h(3), h(4), h(13), h(14));
        for i in 0..n_bb {
            out.push((format!("bb[{i}].bi_pdata"), le64(d, off_bb + i * 24)));
        }
        for i in 0..n_ib {
            let o = off_ib + i * 48;
            out.push((format!("ib[{i}].ib_pmask"), le64(d, o)));
            out.push((format!("ib[{i}].ib_pdata"), le64(d, o + 8)));
        }
    }
    out
}

fn assert_all_rasters_word_aligned(path: &Path, d: &[u8]) {
    let slots = raster_slots(d);
    assert!(
        !slots.is_empty(),
        "{}: fixture must produce raster slots",
        path.display()
    );
    let odd: Vec<_> = slots
        .iter()
        .filter(|(_, v)| *v != 0 && v & 1 != 0)
        .collect();
    assert!(
        odd.is_empty(),
        "{}: {} of {} raster slots start on an ODD offset — they are read as \
         16-bit words (blit_plane, EmuTOS gr_gblt, TeraDesk xrsrc_load): {:?}",
        path.display(),
        odd.len(),
        slots.len(),
        odd
    );
}

// ── TEDINFO reservations (B5) ─────────────────────────────────────────────

/// One TEDINFO of a native resource: the three string pointers as byte
/// offsets from the blob base, plus the two length words.
struct Ted {
    ptext: u64,
    ptmplt: u64,
    pvalid: u64,
    txtlen: u16,
    tmplen: u16,
}

/// What a native resource says about its data section: the TEDINFO array, and
/// every byte offset that is *not* a `te_ptext` yet is referenced as the start
/// of a datum (templates, validation strings, labels, free strings, icon
/// labels, rasters). The distance from a pointer to the next referenced offset
/// is the space the blob really reserves behind it.
struct Layout {
    teds: Vec<Ted>,
    other_refs: Vec<u64>,
    total: u64,
}

fn layout(d: &[u8]) -> Layout {
    let mut teds = Vec::new();
    let mut other = Vec::new();

    // Section bases and struct widths differ per output format; the TEDINFO
    // field order does not (three pointers, then eight WORDs).
    let (off_ted, n_ted, off_frstr, n_frstr, off_obj, n_obj, off_ib, n_ib, ptr, total) =
        if &d[..4] == b"NRSC" {
            let f = |i: usize| le32(d, 4 + i * 4) as usize;
            let ptr = if f(0) == 5 { 8usize } else { 4 };
            (f(10), f(11), f(6), f(7), f(4), f(2), f(12), f(13), ptr, f(5) as u64)
        } else {
            let h = |i: usize| le16(d, i * 2) as usize;
            (h(2), h(12), h(5), h(15), h(1), h(10), h(3), h(13), 8usize, h(17) as u64)
        };
    let nat_ted = 3 * ptr + 16;
    let nat_obj = if ptr == 8 { 32usize } else { 24 };
    let nat_ib = if ptr == 8 { 48usize } else { 36 };
    let spec_at = if ptr == 8 { 16usize } else { 12 };
    let read = |o: usize| if ptr == 8 { le64(d, o) } else { le32(d, o) as u64 };

    for i in 0..n_ted {
        let o = off_ted + i * nat_ted;
        let t = Ted {
            ptext: read(o),
            ptmplt: read(o + ptr),
            pvalid: read(o + 2 * ptr),
            txtlen: le16(d, o + 3 * ptr + 12),
            tmplen: le16(d, o + 3 * ptr + 14),
        };
        other.extend([t.ptmplt, t.pvalid]);
        teds.push(t);
    }
    for i in 0..n_frstr {
        other.push(read(off_frstr + i * ptr));
    }
    for i in 0..n_obj {
        let o = off_obj + i * nat_obj;
        // G_BUTTON / G_STRING / G_TITLE carry a string in ob_spec.
        if matches!(le16(d, o + 6) & 0xFF, 26 | 28 | 32) {
            other.push(read(o + spec_at));
        }
    }
    for i in 0..n_ib {
        other.push(read(off_ib + i * nat_ib + 2 * ptr)); // ib_ptext
    }
    other.extend(raster_slots(d).into_iter().map(|(_, v)| v));
    other.retain(|&v| v != 0);
    Layout { teds, other_refs: other, total }
}

/// The B5 postcondition: behind every TEDINFO string pointer the blob reserves
/// at least the length the resource declares for it, and the bytes past the
/// string's NUL are NUL too (so the buffer stays a valid C string when a
/// shorter text is written into it).
///
/// `te_pvalid` is deliberately absent: the format declares **no** length for
/// it (`include/obdefs.h:174-182` has only `te_txtlen`/`te_tmplen`), it is
/// read-only, and EmuTOS expands it into a scratch buffer rather than in place
/// (`aes/gemobed.c:351-355`). There is nothing to reserve.
fn assert_declared_lengths_are_reserved(path: &Path, d: &[u8]) {
    let l = layout(d);
    assert!(!l.teds.is_empty(), "{}: fixture must have TEDINFOs", path.display());
    let mut refs: Vec<u64> = l.other_refs.clone();
    refs.extend(l.teds.iter().map(|t| t.ptext));
    refs.push(l.total);
    refs.retain(|&v| v != 0);
    refs.sort_unstable();

    for (i, t) in l.teds.iter().enumerate() {
        for (what, p, declared) in [
            ("te_ptext (te_txtlen)", t.ptext, t.txtlen),
            ("te_ptmplt (te_tmplen)", t.ptmplt, t.tmplen),
        ] {
            if p == 0 {
                continue;
            }
            let next = *refs.iter().find(|&&q| q > p).unwrap_or(&l.total);
            assert!(
                u64::from(declared) <= next - p,
                "{}: tedinfo[{i}] {what} declares {declared} bytes but the blob                  reserves only {} behind offset {p} — every writer bounds itself                  by the declared length, not by the string",
                path.display(),
                next - p
            );
            let s = p as usize;
            let n = d[s..].iter().position(|&b| b == 0).unwrap_or_else(|| {
                panic!("{}: tedinfo[{i}] {what} is not NUL-terminated", path.display())
            });
            let hi = (s + declared as usize).max(s + n + 1);
            assert!(
                d[s + n + 1..hi].iter().all(|&b| b == 0),
                "{}: tedinfo[{i}] {what}: the reserved bytes behind the string                  are not NUL",
                path.display()
            );
        }
    }
}

/// A `te_ptext` is the only TEDINFO buffer anyone writes, so it must never be
/// the same storage as a read-only datum — even when the source file happens
/// to point both at one classic offset, as the fixture's `frstr[0]` does.
/// Sharing would let typing in one field rewrite another field's label.
fn assert_text_buffers_are_private(path: &Path, d: &[u8]) {
    let l = layout(d);
    for (i, t) in l.teds.iter().enumerate() {
        if t.ptext == 0 {
            continue;
        }
        assert!(
            !l.other_refs.contains(&t.ptext),
            "{}: tedinfo[{i}]'s writable te_ptext buffer at offset {} is shared              with a read-only string",
            path.display(),
            t.ptext
        );
    }
}

// ── driving the real tool ─────────────────────────────────────────────────

fn scratch_dir(name: &str) -> PathBuf {
    let dir = workspace_root()
        .join("target")
        .join("xtask-rsc-align")
        .join(name);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn run_mkrsc(dir: &Path, mode: Option<&str>, src: &Path, out: &Path) {
    let root = workspace_root();
    let script = root.join("tools").join("mkrsc.py");
    let python = std::env::var_os("PYTHON").unwrap_or_else(|| "python3".into());
    let mut c = Command::new(&python);
    c.current_dir(dir).arg(&script).arg("convert");
    if let Some(m) = mode {
        c.arg(m);
    }
    let st = c
        .arg(src)
        .arg(out)
        .status()
        .unwrap_or_else(|e| panic!("cannot run {python:?} {}: {e}", script.display()));
    assert!(st.success(), "mkrsc.py convert {mode:?} failed with {st}");
}

/// Convert the fixture in one output mode and hand the bytes to `check`.
fn converted(tag: &str, mode: Option<&str>, check: impl Fn(&Path, &[u8])) {
    let dir = scratch_dir(tag);
    let src = dir.join("fixture.rsc");
    let out = dir.join("out.rsc");
    std::fs::write(&src, classic_fixture()).expect("write fixture");
    run_mkrsc(&dir, mode, &src, &out);
    let d = std::fs::read(&out).expect("read converted resource");
    check(&out, &d);
}

/// Convert the fixture in one output mode and check every raster slot.
///
/// The fixture now also carries a TEDINFO reservation (`ted[1]`, 9 bytes
/// behind an empty text), so this is at the same time the guard that the
/// reservation does not push a raster off its word boundary.
fn check_mode(tag: &str, mode: Option<&str>) {
    converted(tag, mode, assert_all_rasters_word_aligned);
}

#[test]
fn mkrsc_convert_lp64_keeps_every_raster_word_aligned() {
    check_mode("lp64", None);
}

#[test]
fn mkrsc_convert_ilp32_keeps_every_raster_word_aligned() {
    check_mode("ilp32", Some("--ilp32"));
}

#[test]
fn mkrsc_convert_rshdr_keeps_every_raster_word_aligned() {
    check_mode("rshdr", Some("--rshdr"));
}

/// The committed blob the kernel embeds (`tos-api/src/aes/fsel.rs`
/// `GEM_FSEL_RSC`), regenerated by `cargo xtask gen-fsel-rsc`: it must carry
/// the same invariant, so a stale artifact cannot outlive the tool fix.
#[test]
fn the_committed_gem_fsel_blob_has_word_aligned_rasters() {
    let p = gem_fsel_path();
    let d = std::fs::read(&p).expect("gem_fsel.nrsc must be committed");
    assert_all_rasters_word_aligned(&p, &d);
}

fn gem_fsel_path() -> PathBuf {
    workspace_root()
        .join("tos-api")
        .join("src")
        .join("aes")
        .join("gem_fsel.nrsc")
}

// ── B5: the declared TEDINFO lengths, in all three output modes ───────────

#[test]
fn mkrsc_convert_lp64_reserves_the_declared_tedinfo_lengths() {
    converted("lp64-reserve", None, assert_declared_lengths_are_reserved);
}

#[test]
fn mkrsc_convert_ilp32_reserves_the_declared_tedinfo_lengths() {
    converted("ilp32-reserve", Some("--ilp32"), assert_declared_lengths_are_reserved);
}

#[test]
fn mkrsc_convert_rshdr_reserves_the_declared_tedinfo_lengths() {
    converted("rshdr-reserve", Some("--rshdr"), assert_declared_lengths_are_reserved);
}

/// The fixture's `frstr[0]` deliberately points at the same classic offset as
/// `ted[0].te_ptext`, the aliasing a shared intern cache would collapse.
#[test]
fn mkrsc_convert_lp64_gives_every_text_buffer_its_own_storage() {
    converted("lp64-private", None, assert_text_buffers_are_private);
}

#[test]
fn mkrsc_convert_rshdr_gives_every_text_buffer_its_own_storage() {
    converted("rshdr-private", Some("--rshdr"), assert_text_buffers_are_private);
}

/// The committed file-selector blob, the one the audit measured at 11 of 13
/// TEDINFOs short (`ted[0]`: a 1-byte buffer against `te_txtlen` 39). It is
/// relocated by `relocate_blob` — the embedded path, EmuTOS' `gem_rsc_init`
/// — which does **not** recompute the length words, so the reservation in the
/// file is the only thing standing between `objc_edit` and the next string.
#[test]
fn the_committed_gem_fsel_blob_reserves_the_declared_tedinfo_lengths() {
    let p = gem_fsel_path();
    let d = std::fs::read(&p).expect("gem_fsel.nrsc must be committed");
    assert_declared_lengths_are_reserved(&p, &d);
    assert_text_buffers_are_private(&p, &d);
}
