//! KEYS01 — `Kbrate` (XBIOS 0x23) and the values the key-repeat handler
//! reads.
//!
//! EmuTOS `kbrate` (`bios/ikbd.c:439-449`) only reads and writes the two
//! globals `kb_initial` / `kb_repeat`, and `do_key_repeat` / `kbd_int` consume
//! exactly those globals (`:527`, `:891`). NewTOS' repeat handler
//! (`kernel::key_repeat`, driven from `kernel::ikbd::poll_repeat`) reads them
//! through `tos_api::xbios::system::kb_rate`, so `Kbrate(-1,-1)` must report
//! what `kb_rate()` hands the handler, and a set must change it.
//! Spec: `docs/api/xbios/23-kbrate.md`, `docs/api/xbios/_state.md`
//! §"Key-repeat handler".

use tos_api::trap::{TrapArgs, TrapClass};

fn kbrate(initial: i16, repeat: i16) -> i64 {
    tos_api::xbios::dispatch(&TrapArgs {
        regs: [initial as u16 as u64, repeat as u16 as u64, 0, 0, 0, 0, 0, 0],
        func: 0x23,
        svc_imm: TrapClass::Xbios as u16,
        space: tos_api::uaccess::Space::Kernel,
    })
}

/// One test, because the two byte fields are process-global state.
#[test]
fn kbrate_get_set_is_what_the_repeat_handler_reads() {
    // Power-up: 15 / 2 (`include/sysconf.h:68-69`), reported and in use.
    assert_eq!(kbrate(-1, -1), 0x0F02);
    assert_eq!(tos_api::xbios::system::kb_rate(), (15, 2));

    // Set both: returns the old value, the handler sees the new one.
    assert_eq!(kbrate(20, 3), 0x0F02);
    assert_eq!(tos_api::xbios::system::kb_rate(), (20, 3));
    assert_eq!(kbrate(-1, -1), 0x1403);

    // Set one, query the other.
    assert_eq!(kbrate(30, -1), 0x1403);
    assert_eq!(tos_api::xbios::system::kb_rate(), (30, 3));
    assert_eq!(kbrate(-1, 0), 0x1E03);
    assert_eq!(tos_api::xbios::system::kb_rate(), (30, 0));

    // Byte masking (`& 0x00ff`, `:444,446`) and the sign-extended WORD return.
    assert_eq!(kbrate(0x1FF, 0x101), 0x1E00);
    assert_eq!(tos_api::xbios::system::kb_rate(), (0xFF, 0x01));
    assert_eq!(kbrate(15, 2), 0xFF01u16 as i16 as i64);

    // Back to the defaults.
    assert_eq!(kbrate(-1, -1), 0x0F02);
    assert_eq!(tos_api::xbios::system::kb_rate(), (15, 2));
}

/// With no process running (boot, EL1 kernel code) the repeat handler
/// sees the boot `conterm` 7 — key repeat on (`bios/ikbd.c:1100`).
#[test]
fn conterm_without_a_process_is_the_boot_value() {
    assert!(tos_api::bdos::process::current().is_null());
    assert_eq!(tos_api::bios::chardev::current_conterm(), 7);
    assert_ne!(
        tos_api::bios::chardev::current_conterm() & tos_api::bios::chardev::CONTERM_KEY_REPEAT,
        0
    );
}
