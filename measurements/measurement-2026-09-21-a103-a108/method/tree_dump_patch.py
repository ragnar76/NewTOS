#!/usr/bin/env python3
"""A108-TEMP (scratch copy only): after the desktop registered its menu bar,
print the geometry `build_menu` really built — THEBAR's and THEACTIVE's
rectangles, every title's (x, w), every box's (x, w, h) and the first File
entry's rectangle — to the serial line, so the board run ties the kernel
tree (which has no host tests) to the host replica and to adjust_menu's
numbers."""
import sys
p = sys.argv[1] + '/kernel/src/desk/menu.rs'; s = open(p).read()
a = "pub fn install_menu_bar() {\n    let tree = build_menu();\n    super::menu_bar(tree as u64, 1);\n}"
assert s.count(a) == 1
b = '''pub fn install_menu_bar() {
    let tree = build_menu();
    super::menu_bar(tree as u64, 1);
    // A108-TEMP: dump the registered geometry.
    unsafe {
        let o = &*MENU.obj.get();
        let bar = &o[1];
        let a = &o[2];
        crate::kprintln!("[a108] THEBAR=({},{},{},{}) THEACTIVE=({},{},{},{}) titles=({},{}),({},{}),({},{}),({},{})",
            bar.ob_x, bar.ob_y, bar.ob_width, bar.ob_height, a.ob_x, a.ob_y, a.ob_width, a.ob_height,
            o[3].ob_x, o[3].ob_width, o[4].ob_x, o[4].ob_width, o[5].ob_x, o[5].ob_width, o[6].ob_x, o[6].ob_width);
        let mut k = 3 + MENUS.len() + 1; // first box index
        for (ti, def) in MENUS.iter().enumerate() {
            let b = &o[k];
            let e = &o[k + 1];
            crate::kprintln!("[a108] BOX{}#{}=({},{},{},{}) spec={:#x} ENTRY0=({},{},{},{})", ti, k,
                b.ob_x, b.ob_y, b.ob_width, b.ob_height, b.ob_spec, e.ob_x, e.ob_y, e.ob_width, e.ob_height);
            k += 1 + def.entries.len();
        }
    }
}'''
s = s.replace(a, b)
open(p, 'w').write(s)
print("patched", p)
