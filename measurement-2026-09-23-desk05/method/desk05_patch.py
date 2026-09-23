#!/usr/bin/env python3
"""DESK05-TEMP (scratch copy only): serial dumps for the DESK05 run.

1) kernel/src/desk/menu.rs -- after the desktop registered its menu bar, print
   every drop-down box's rectangle and the live `menu_shortcuts[]` array, so the
   board run ties `copy_menu_items` + `install_shortcuts` to the pixels.
2) kernel/src/desk/inf.rs -- inside `inf_conf`, print the rectangle
   `start_dialog` got back from `form_center` and the centred ROOT position, so
   the dialog's geometry can be measured instead of eyeballed.

Applied to the scratch copy AFTER a79_patch_desk10.py, never to the shared tree.
"""
import sys

scr = sys.argv[1]


def rep(path, a, b):
    s = open(path).read()
    assert s.count(a) == 1, (path, a[:70])
    open(path, 'w').write(s.replace(a, b, 1))
    print("patched", path)


rep(scr + '/kernel/src/desk/menu.rs',
    "pub fn install_menu_bar() {\n    let tree = build_menu();\n    install_shortcuts();\n    super::menu_bar(tree as u64, 1);\n}",
    '''pub fn install_menu_bar() {
    let tree = build_menu();
    install_shortcuts();
    super::menu_bar(tree as u64, 1);
    // DESK05-TEMP: dump the registered drop-down geometry + the shortcut array.
    unsafe {
        let o = &*MENU.obj.get();
        let labels = &*MENU.labels.get();
        let mut k = 3 + MENUS.len() + 1; // first box index
        for (ti, def) in MENUS.iter().enumerate() {
            let b = &o[k];
            let e = &o[k + 1];
            let s = &labels[k + 1];
            let n = s.iter().position(|&c| c == 0).unwrap_or(0);
            crate::kprintln!("[desk05] BOX{}#{}=({},{},{},{}) cells={} ENTRY0=({},{},{},{}) text={:?}", ti, k,
                b.ob_x, b.ob_y, b.ob_width, b.ob_height, (b.ob_width + 7) / 8,
                e.ob_x, e.ob_y, e.ob_width, e.ob_height,
                core::str::from_utf8(&s[..n]).unwrap_or("?"));
            k += 1 + def.entries.len();
        }
        let sc = &*MENU.shortcuts.get();
        crate::kprintln!("[desk05] menu_shortcuts={:?}", sc);
    }
}''')

rep(scr + '/kernel/src/desk/inf.rs',
    "    start_dialog(tree_ptr);\n    let mut exitobj;",
    '''    let rect = start_dialog(tree_ptr);
    // DESK05-TEMP: the rectangle `form_center` reported + the centred ROOT.
    crate::kprintln!(
        "[desk05] ADDESKCF form_center rect=({},{},{},{}) ROOT=({},{},{},{})",
        rect.x, rect.y, rect.w, rect.h,
        tree[0].ob_x, tree[0].ob_y, tree[0].ob_width, tree[0].ob_height
    );
    let mut exitobj;''')
