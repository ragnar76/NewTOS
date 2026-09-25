#!/usr/bin/env python3
"""FS04-TEMP: add a redraw benchmark to the driver, and optionally force the
fallback framebuffer geometry so the same benchmark can be taken at a second
resolution.

usage: fs04_redraw_bench.py <scratch-tree> [WIDTHxHEIGHT]
"""
import sys, re, os
ws = sys.argv[1]
geo = sys.argv[2] if len(sys.argv) > 2 else None

p = ws + '/kernel/src/desk/fs04drv.rs'
s = open(p).read()
old = """    super::global::with(|g| g.g_ccopypref = old);
    kprintln!("[fs04] driver done");"""
new = """    super::global::with(|g| g.g_ccopypref = old);

    // G: what one full desktop redraw costs, and how much cache maintenance
    //    it issues.  Taken at two framebuffer geometries so the scaling is
    //    measured rather than assumed.
    {
        let (w, h) = super::global::with(|g| (g.g_desk.w, g.g_desk.h)).unwrap_or((0, 0));
        fp::reset();
        let t0 = fp::now();
        for _ in 0..5 {
            super::redraw::redraw_desktop();
        }
        let t1 = fp::now();
        kprintln!("[fs04] G-redraw-x5 at {}x{}", w, h);
        report("G-redraw-x5", t1.wrapping_sub(t0));
    }
    kprintln!("[fs04] driver done");"""
assert s.count(old) == 1
open(p, 'w').write(s.replace(old, new, 1))
print("redraw bench added")

if geo:
    w, h = geo.split('x')
    q = ws + '/kernel/src/drivers/bcm2711/framebuffer.rs'
    t = open(q).read()
    a = """        let (w, h) = match query_physical_size() {"""
    b = f"""        let (w, h) = (({w}u32, {h}u32), 0).0; // FS04-TEMP: forced geometry
        #[allow(unreachable_code)]
        let _unused = match query_physical_size() {{"""
    assert t.count(a) == 1
    t = t.replace(a, b, 1)
    # the forced arm makes the original match a dead value binding; keep it
    # compiling by discarding its result.
    c = """                (FALLBACK_WIDTH, FALLBACK_HEIGHT)
            }
        };"""
    d = """                (FALLBACK_WIDTH, FALLBACK_HEIGHT)
            }
        };
        let _ = _unused;"""
    assert t.count(c) == 1
    t = t.replace(c, d, 1)
    open(q, 'w').write(t)
    print("fallback geometry forced to", geo)
