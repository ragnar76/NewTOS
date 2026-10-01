#!/usr/bin/env python3
"""DESK06-TEMP (scratch copy only): what the DESK06 board run needs on top of
the standard injector scaffolding.

1) kernel/src/host_env.rs -- step the H02 injector inside
   `console_read_key_blocking`'s spin as well. The injector normally only runs
   from `pump_input`, which is the AES event loop's idle pass; while EmuCON
   blocks in `Cnecin` the AES is not running at all, so without this hook no
   scripted keystroke could ever reach the console. Nothing about the product
   path changes -- this is the QEMU stand-in for a keyboard.

2) kernel/src/desk/menu.rs -- after the menu bar is registered, print the File
   drop-down's entry rectangles + texts, so the click point for
   `Execute EmuCON` is measured rather than guessed.

3) kernel/src/shell_loop.rs -- one serial marker on each side of the nested
   console pass, so the QMP driver has something deterministic to wait for
   (the console's own output goes to the same serial line, but its first byte
   is not a unique token).

Applied to the scratch copy AFTER a79_patch_desk10.py / apply_injector.py /
a78_patch2.py / key_step_patch.py, never to the shared tree.
"""
import sys

scr = sys.argv[1]


def rep(path, a, b):
    s = open(path).read()
    assert s.count(a) == 1, (path, a[:70])
    open(path, 'w').write(s.replace(a, b, 1))
    print("patched", path)


# 1) injector step inside the blocking console read
rep(scr + '/kernel/src/host_env.rs',
    "            crate::drivers::usb::poll();\n\n            if let Some(w) = crate::console::rx_pop_key() {",
    "            crate::drivers::usb::poll();\n"
    "            h02_inject::step(); // DESK06-TEMP: keys for the nested EmuCON\n\n"
    "            if let Some(w) = crate::console::rx_pop_key() {")

# 2) File drop-down entry geometry
rep(scr + '/kernel/src/desk/menu.rs',
    "pub fn install_menu_bar() {\n    let tree = build_menu();\n    install_shortcuts();\n    super::menu_bar(tree as u64, 1);\n}",
    '''pub fn install_menu_bar() {
    let tree = build_menu();
    install_shortcuts();
    super::menu_bar(tree as u64, 1);
    // DESK06-TEMP: dump the File drop-down's entry rectangles + texts.
    unsafe {
        let o = &*MENU.obj.get();
        let labels = &*MENU.labels.get();
        let mut k = 3 + MENUS.len() + 1; // first box index
        for def in MENUS.iter() {
            if def.title as usize == Title::File as usize {
                let b = &o[k];
                crate::kprintln!("[desk06] FILEBOX#{}=({},{},{},{})", k,
                    b.ob_x, b.ob_y, b.ob_width, b.ob_height);
                for e in 0..def.entries.len() {
                    let ob = &o[k + 1 + e];
                    let s = &labels[k + 1 + e];
                    let n = s.iter().position(|&c| c == 0).unwrap_or(0);
                    crate::kprintln!("[desk06] ENTRY{}#{}=({},{},{},{}) flags={:#x} state={:#x} text={:?}",
                        e, k + 1 + e, ob.ob_x, ob.ob_y, ob.ob_width, ob.ob_height,
                        ob.ob_flags, ob.ob_state,
                        core::str::from_utf8(&s[..n]).unwrap_or("?"));
                }
            }
            k += 1 + def.entries.len();
        }
    }
}''')

# 3) markers around the nested console pass
rep(scr + '/kernel/src/shell_loop.rs',
    "    shell::sh_chdef(); // :712 — the directory `shel_wdef` named\n    sh_chgrf(); // :713 — into alpha mode",
    "    shell::sh_chdef(); // :712 — the directory `shel_wdef` named\n"
    "    sh_chgrf(); // :713 — into alpha mode\n"
    "    crate::kprintln!(\"[desk06] console: alpha mode entered\"); // DESK06-TEMP")
rep(scr + '/kernel/src/shell_loop.rs',
    "    shell::sh_chdef(); // :712\n    sh_chgrf(); // :713 — back into graphics mode\n    true",
    "    shell::sh_chdef(); // :712\n"
    "    sh_chgrf(); // :713 — back into graphics mode\n"
    "    crate::kprintln!(\"[desk06] console: graphics mode restored\"); // DESK06-TEMP\n"
    "    true")
