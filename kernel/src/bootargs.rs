//! Boot-argument tokens (SER01).
//!
//! The boot firmware of a Raspberry Pi reads `cmdline.txt` from the boot
//! partition and hands its text to the kernel; NewTOS fetches it with the
//! VideoCore mailbox tag "Get command line"
//! (`drivers::bcm2711::mailbox::with_command_line`, source:
//! `references/circle/lib/koptions.cpp:52-61`). On QEMU `raspi4b` the same
//! tag answers the `-append` string.
//!
//! The text is a list of options separated by blanks
//! (`references/circle/doc/cmdline.txt:3-4`: *"Multiple options must be placed
//! on a single line delimited by a space character"*). The parser here splits
//! on any ASCII whitespace rather than on the space alone, so a `cmdline.txt`
//! saved with a trailing CR/LF or with tabs still yields clean tokens, and it
//! stops at a NUL. A token matches only as a whole: `serial=on` is found in
//! `quiet serial=on`, but not in `serial=onx` or `xserial=on`.
//!
//! Pure byte logic with no kernel dependencies, so the host test suite
//! (`tools/xtask`, which includes this file by path) can run its tests.

/// The token that switches the serial (UART0) output on, `board-rpi4`.
pub const SERIAL_ON: &[u8] = b"serial=on";

/// The token that switches memory protection off (ARCH03 3b) — FreeMiNT's
/// `MEM_PROT=NO` (`references/freemint/sys/bootmenu.c:207-219`). Without it
/// every process has its own rights (`PROT_P/G/S/PR/I`).
pub const MEMPROT_OFF: &[u8] = b"memprot=off";

/// The token that makes the parent-to-child rule strict (ARCH03 E5,
/// FreeMiNT `MPF_STRICT`, `references/freemint/sys/k_exec.c:799-806`): a
/// child no longer keeps access to its parent's private memory.
pub const MEMPROT_STRICT: &[u8] = b"memprot=strict";

/// The token that switches per-process ASIDs on (ARCH03 stage 4,
/// `docs/analysis/arch03-implementation-plan.md` Stufe 4, E12): every root
/// is installed with an ASID of its own and an address-space switch drops no
/// TLB entries. Off without it (the TLBI-on-every-switch path).
pub const ASID_ON: &[u8] = b"asid=on";

/// The rollover test hook of stage 4: ASIDs on, but only four numbers per
/// generation (three usable), so that ASID generations roll over after
/// every few process starts — the path that otherwise needs 65 535 of them.
pub const ASID_TINY: &[u8] = b"asid=tiny";

/// The fallback switch of the SD driver (FS08): `emmc=compat` keeps the
/// microSD bus at 1 bit and moves every sector with its own single-block
/// command (`CMD17`/`CMD24`) — the driver as it was before FS08. Without it
/// the driver switches the card to the 4-bit bus (`ACMD6`) when its SCR
/// allows, and moves multi-sector requests with one `CMD18`/`CMD25`.
pub const EMMC_COMPAT: &[u8] = b"emmc=compat";

/// `true` if `args` contains `token` as a whole whitespace-delimited token.
///
/// `args` ends at its first NUL byte, if any. An empty `token` never
/// matches.
pub fn has_token(args: &[u8], token: &[u8]) -> bool {
    if token.is_empty() {
        return false;
    }
    let end = args.iter().position(|&b| b == 0).unwrap_or(args.len());
    args[..end]
        .split(|b| b.is_ascii_whitespace())
        .any(|t| t == token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memprot_tokens_are_whole_tokens() {
        assert!(has_token(b"serial=on memprot=off", MEMPROT_OFF));
        assert!(!has_token(b"serial=on memprot=offx", MEMPROT_OFF));
        assert!(has_token(b"memprot=strict\n", MEMPROT_STRICT));
        assert!(!has_token(b"memprot=off", MEMPROT_STRICT));
    }

    #[test]
    fn asid_tokens_are_whole_tokens() {
        assert!(has_token(b"serial=on asid=on memprot=off", ASID_ON));
        assert!(!has_token(b"serial=on asid=only", ASID_ON));
        assert!(!has_token(b"asid=tiny", ASID_ON));
        assert!(has_token(b"asid=tiny\r\n", ASID_TINY));
        assert!(!has_token(b"asid=on", ASID_TINY));
    }

    #[test]
    fn emmc_compat_is_a_whole_token() {
        assert!(has_token(b"serial=on emmc=compat memprot=on", EMMC_COMPAT));
        assert!(has_token(b"emmc=compat\r\n", EMMC_COMPAT));
        assert!(!has_token(b"emmc=compatx", EMMC_COMPAT));
        assert!(!has_token(b"emmc=COMPAT", EMMC_COMPAT));
        assert!(!has_token(b"emmc=", EMMC_COMPAT));
        assert!(!has_token(b"serial=on memprot=on", EMMC_COMPAT));
    }

    #[test]
    fn finds_the_token_alone_and_among_others() {
        assert!(has_token(b"serial=on", SERIAL_ON));
        assert!(has_token(b"quiet serial=on", SERIAL_ON));
        assert!(has_token(b"serial=on quiet", SERIAL_ON));
        assert!(has_token(b"a  serial=on  b", SERIAL_ON));
    }

    #[test]
    fn accepts_tabs_and_line_ends_as_separators() {
        assert!(has_token(b"quiet\tserial=on\r\n", SERIAL_ON));
        assert!(has_token(b"serial=on\n", SERIAL_ON));
    }

    #[test]
    fn rejects_partial_and_different_tokens() {
        assert!(!has_token(b"serial=onx", SERIAL_ON));
        assert!(!has_token(b"xserial=on", SERIAL_ON));
        assert!(!has_token(b"serial=off", SERIAL_ON));
        assert!(!has_token(b"serial", SERIAL_ON));
        assert!(!has_token(b"SERIAL=ON", SERIAL_ON));
        assert!(!has_token(b"serial = on", SERIAL_ON));
    }

    #[test]
    fn empty_input_and_empty_token_never_match() {
        assert!(!has_token(b"", SERIAL_ON));
        assert!(!has_token(b"   ", SERIAL_ON));
        assert!(!has_token(b"serial=on", b""));
    }

    #[test]
    fn stops_at_the_first_nul() {
        assert!(has_token(b"serial=on\0garbage", SERIAL_ON));
        assert!(!has_token(b"quiet\0serial=on", SERIAL_ON));
    }
}
