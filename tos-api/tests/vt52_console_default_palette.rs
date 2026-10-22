//! CON04 decision D1, the case without a workstation: before any `v_opnwk`
//! (the `\AUTO\` programs) `ESC b`/`ESC c` registers resolve through EmuTOS'
//! `falcon_default_palette` (`bios/conout.c:31-34,169-170`).

mod vt52_common;

use vt52_common::*;

#[test]
fn registers_use_the_falcon_default_palette_without_a_workstation() {
    let (_g, s) = setup(false);
    const FALCON: [u16; 16] = [
        0xffdf, 0xf800, 0x07c0, 0xffc0, 0x001f, 0xf81f, 0x07df, 0xbdd7, //
        0x8c51, 0xa800, 0x0540, 0xad40, 0x0015, 0xa815, 0x0555, 0x0000,
    ];
    for r in 0..16u8 {
        out(s, &[0x1b, b'c', b'0' + r, b' ']);
    }
    for (r, &word) in FALCON.iter().enumerate() {
        assert!(is_blank(s, r as i16, 0, word), "register {r}");
    }
    // The default colours: register 15 on register 0.
    out(s, b"\x1bc0\r\nX");
    assert!(is_char(s, 0, 1, b'X', FALCON[15], FALCON[0]));
}
