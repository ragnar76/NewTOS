//! SCRATCH — DESK43/U2 measurement instrument. **Not product code.**
//!
//! Removed again after the measurement; the diff is kept in
//! `measurement-2026-09-26-desk43-u2/method/`.
//!
//! Two hooks in `desk::run`:
//!
//! * [`seed`] — before `app_start`'s `shel_get`: fill the AES shell buffer with
//!   a 128-byte control-panel prefix (`CP_SHELL_INIT`, `aes/geminit.c:97`)
//!   followed by an `.INF` image whose `#W` line names a directory the file on
//!   disk does **not** name. Whatever the desktop then opens says which of the
//!   two sources `app_start` used, and whether it looked at offset 0 or 128.
//! * [`probe`] — after `[desk] desktop up`: run `app_save(FALSE)` (the write
//!   half of the channel) and read the buffer straight back, then
//!   `app_save(TRUE)` for the disk half.

use alloc::vec;

use crate::kprintln;

/// `CP_SHELL_INIT` (`aes/geminit.c:97`) — what `run_accs_and_desktop` puts into
/// the front of the shell buffer once per AES boot (`:685`). NewTOS's AES does
/// not do that yet, so the measurement supplies it: it makes the prefix
/// *visible*, and its leading `#` is what a reader at the wrong offset would
/// mistake for an `.INF` signature.
const CP_SHELL_INIT: &[u8] = b"#a000000\r\n";

/// The `.INF` image parked in the shell buffer behind the prefix. Its one open
/// window is `C:\AUTO\`; the file on disk names `C:\ACC\`.
const SEED_INF: &[u8] = b"\
#R 02\r\n\
#E 1A E0 00 00 60\r\n\
#Q 41 40 43 40 43 40\r\n\
#K 4F 49 00 4E 48 55 57 41 00 44 00 5A 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 52 00 53 00 00\r\n\
#W 00 00 02 04 26 0C 00 C:\\AUTO\\@\r\n\
#W 00 00 00 01 50 1C 00 @\r\n\
#W 00 00 00 01 50 1C 00 @\r\n\
#W 00 00 00 01 50 1C 00 @\r\n\
#W 00 00 00 01 50 1C 00 @\r\n\
#W 00 00 00 01 50 1C 00 @\r\n\
#W 00 00 00 01 50 1C 00 @\r\n\
#M 00 00 00 FF C DISK C@ @\r\n\
#T 00 09 03 FF   TRASH@ @\r\n\
#O 06 09 04 FF   PRINTER@ @\r\n\
#G 06 FF *.NAP@ @\r\n\
#F 06 FF *.NCP@ @\r\n\
#P 06 FF *.NTP@ @\r\n\
#F 06 FF *.TOS@ @\r\n\
#P 06 FF *.TTP@ @\r\n\
#G 06 FF *.PRG@ @\r\n\
#G 06 FF *.APP@ @\r\n\
#Y 06 FF *.GTP@ @\r\n\
#D FF 02 @ *.*@\r\n\
#N FF 07 @ *.*@\r\n\
#F FF 07 @ *.*@\r\n\
";

const CPDATA_LEN: usize = 128;
const SIZE_SHELBUF: usize = 4192;

/// Put `CP_SHELL_INIT` + [`SEED_INF`] into the AES shell buffer through the
/// real `shel_put` trap, exactly as a control panel plus a dying desktop would.
pub(super) fn seed() {
    let mut buf = vec![0u8; SIZE_SHELBUF];
    buf[..CP_SHELL_INIT.len()].copy_from_slice(CP_SHELL_INIT);
    buf[CPDATA_LEN..CPDATA_LEN + SEED_INF.len()].copy_from_slice(SEED_INF);
    buf[CPDATA_LEN + SEED_INF.len()] = 0;
    super::shel_put(&buf[..CPDATA_LEN + SEED_INF.len() + 1]);
    kprintln!(
        "[u2-seed] shel_put: {} prefix bytes + {} INF bytes + NUL, #W path C:\\AUTO\\",
        CP_SHELL_INIT.len(),
        SEED_INF.len()
    );
}

fn dump(tag: &str, b: &[u8]) {
    kprintln!("[u2] {} ({} bytes):", tag, b.len());
    let mut line = alloc::string::String::new();
    for &c in b {
        match c {
            b'\r' => line.push_str("\\r"),
            b'\n' => line.push_str("\\n"),
            0 => line.push_str("\\0"),
            0x20..=0x7E => line.push(c as char),
            _ => line.push('.'),
        }
    }
    kprintln!("[u2]   {}", line);
}

/// Drive both halves of the channel and report what the buffer holds.
pub(super) fn probe() {
    // The write half — `deskmain.c:2155`, the statement the desktop epilog
    // runs. Called here directly so it can be observed without a launch.
    super::inf::app_save(false);

    let mut buf = vec![0u8; SIZE_SHELBUF];
    super::shel_get(&mut buf);
    let nul = CPDATA_LEN
        + buf[CPDATA_LEN..]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(buf.len() - CPDATA_LEN);
    kprintln!("[u2] after app_save(FALSE): first NUL at offset {}", nul);
    kprintln!(
        "[u2] prefix preserved = {}",
        &buf[..CP_SHELL_INIT.len()] == CP_SHELL_INIT
    );
    dump("prefix[0..32]", &buf[..32]);
    dump("buf[120..136] (the CPDATA seam)", &buf[120..136]);
    dump("buf[128..176] (start of the INF text)", &buf[128..176]);
    // The `#W` line with a non-empty path, if any.
    let text = &buf[CPDATA_LEN..nul];
    let mut i = 0;
    while i + 3 < text.len() {
        if &text[i..i + 3] == b"#W " {
            let end = i + text[i..].iter().position(|&c| c == b'\r').unwrap_or(text.len() - i);
            if end >= i + 2 && text[end - 1] == b'@' && text[end - 2] != b' ' {
                dump("#W with a path", &text[i..end]);
            }
            i = end;
        }
        i += 1;
    }

    // The disk half — `app_save(TRUE)`, what *Save desktop* runs.
    let ok = super::inf::app_save(true);
    kprintln!("[u2] app_save(TRUE) -> {}", ok);

    // ── Phase 2: the real launch path ────────────────────────────────────
    // Wipe the shell buffer, then launch a GEM application through the real
    // `run_gem_app` — the path a double-click takes. Its `desktop_relinquish`
    // is the EmuTOS epilog and must refill the buffer (`deskmain.c:2155`).
    let zero = vec![0u8; SIZE_SHELBUF];
    super::shel_put(&zero);
    let mut check = vec![0u8; 200];
    super::shel_get(&mut check);
    kprintln!(
        "[u2] buffer wiped: all zero = {}",
        check.iter().all(|&b| b == 0)
    );
    let mut tail = [0u8; 128];
    tail[2] = 0x0D;
    let ran = super::main::run_gem_app(b"C:\\ARGVTEST.NAP\0", &tail);
    kprintln!("[u2] run_gem_app -> {}", ran);
    let mut after = vec![0u8; SIZE_SHELBUF];
    super::shel_get(&mut after);
    let nul2 = CPDATA_LEN
        + after[CPDATA_LEN..]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(after.len() - CPDATA_LEN);
    kprintln!("[u2] after the launch: first NUL at offset {}", nul2);
    dump("after-launch buf[128..176]", &after[128..176]);
    let text2 = &after[CPDATA_LEN..nul2];
    let mut j = 0;
    while j + 3 < text2.len() {
        if &text2[j..j + 3] == b"#W " {
            let end = j + text2[j..].iter().position(|&c| c == b'\r').unwrap_or(text2.len() - j);
            if end >= j + 2 && text2[end - 1] == b'@' && text2[end - 2] != b' ' {
                dump("after-launch #W with a path", &text2[j..end]);
            }
            j = end;
        }
        j += 1;
    }
}
