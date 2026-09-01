#!/usr/bin/env python3
"""Decode a NewTOS AES trap trace (`C:\\AESTRACE.BIN`) into readable text.

The trace is produced by `tos_api::aes::trace` (kernel built with
`--features aes-trace`). The target writes fixed-size binary records and does
no formatting at all — every byte of interpretation happens here, which is the
`docs/TOS-REFERENCE-DUMPER.md` §3 rule applied to a live trace.

    tools/aestrace.py AESTRACE.BIN                  # full trace
    tools/aestrace.py AESTRACE.BIN --op 25          # only evnt_multi
    tools/aestrace.py AESTRACE.BIN --tail 200       # last 200 records
    tools/aestrace.py AESTRACE.BIN --summary        # per-opcode call counts
    tools/aestrace.py AESTRACE.BIN --events         # evnt_*/message records only

File layout (little-endian, native AArch64 order):

  * 256-byte ASCII header line, space-padded, `\\n`-terminated
  * N × 112-byte records, oldest first

Record layout — must stay in lockstep with `struct Rec` in
`tos-api/src/aes/trace.rs`:

  off  size  field
    0     4  seq        u32   record number, gapless across a ring wrap
    4     4  passes     u32   predicate evaluations inside a blocking evnt_*
    8     8  t_ticks    u64   CNTPCT_EL0 at hook entry
   16     8  dur_ticks  u64   call entry -> exit
   24     8  addr_in0   u64
   32     8  addr_out0  u64
   40     2  opcode     i16   (MSG_* records: the message type msg[0])
   42     2  pid        i16
   44     2  in_len     i16   control[1]
   46     2  out_len    i16   control[2]
   48     2  ain_len    i16   control[3]
   50     2  reserved
   52     1  kind       u8
   53     1  flags      u8
   54     1  slot       u8    GEMDOS process slot, 0xFF = in-kernel caller
   55     1  nin        u8    int_in words present in w
   56     1  nout       u8    int_out words present in w, after the nin
   57     1  mb_or      u8    OR of the button byte over all evaluations
   58     1  mb_and     u8    AND of the button byte over all evaluations
   59     1  pad
   60    48  w[24]      i16
  108     4  pad
"""

import argparse
import struct
import sys

FORMAT = b"NEWTOS-AESTRACE v2 "
HDR_LEN = 256
REC_LEN = 112
REC_FMT = "<IIQQQQhhhhhhBBBBBBBx" + "24h" + "4x"

KIND_CALL, KIND_MSG_POST, KIND_MSG_RECV, KIND_MARK = 1, 2, 3, 4
KIND_NAME = {1: "CALL", 2: "POST", 3: "RECV", 4: "MARK"}

F_YIELDED, F_NO_INTIN, F_POST_FAILED, F_KERNEL = 0x01, 0x02, 0x04, 0x08

# AES opcode names — `tos-api/src/aes/mod.rs::crysbind`, EmuTOS
# `aes/gemsuper.c:77-421`.
AES_NAME = {
    10: "appl_init", 11: "appl_read", 12: "appl_write", 13: "appl_find",
    14: "appl_tplay", 15: "appl_trecord", 17: "appl_yield", 19: "appl_exit",
    20: "evnt_keybd", 21: "evnt_button", 22: "evnt_mouse", 23: "evnt_mesag",
    24: "evnt_timer", 25: "evnt_multi", 26: "evnt_dclick",
    30: "menu_bar", 31: "menu_icheck", 32: "menu_ienable", 33: "menu_tnormal",
    34: "menu_text", 35: "menu_register", 36: "menu_popup", 37: "menu_attach",
    38: "menu_istart", 39: "menu_settings",
    40: "objc_add", 41: "objc_delete", 42: "objc_draw", 43: "objc_find",
    44: "objc_offset", 45: "objc_order", 46: "objc_edit", 47: "objc_change",
    48: "objc_sysvar",
    50: "form_do", 51: "form_dial", 52: "form_alert", 53: "form_error",
    54: "form_center", 55: "form_keybd", 56: "form_button",
    70: "graf_rubberbox", 71: "graf_dragbox", 72: "graf_movebox",
    73: "graf_growbox", 74: "graf_shrinkbox", 75: "graf_watchbox",
    76: "graf_slidebox", 77: "graf_handle", 78: "graf_mouse",
    79: "graf_mkstate",
    80: "scrp_read", 81: "scrp_write", 82: "scrp_clear",
    90: "fsel_input", 91: "fsel_exinput",
    100: "wind_create", 101: "wind_open", 102: "wind_close",
    103: "wind_delete", 104: "wind_get", 105: "wind_set", 106: "wind_find",
    107: "wind_update", 108: "wind_calc", 109: "wind_new",
    110: "rsrc_load", 111: "rsrc_free", 112: "rsrc_gaddr", 113: "rsrc_saddr",
    114: "rsrc_obfix",
    120: "shel_read", 121: "shel_write", 122: "shel_get", 123: "shel_put",
    124: "shel_find", 125: "shel_envrn", 126: "shel_rdef", 127: "shel_wdef",
}

# GEM message types — EmuTOS `include/aesdefs.h`.
MSG_NAME = {
    10: "MN_SELECTED", 20: "WM_REDRAW", 21: "WM_TOPPED", 22: "WM_CLOSED",
    23: "WM_FULLED", 24: "WM_ARROWED", 25: "WM_HSLID", 26: "WM_VSLID",
    27: "WM_SIZED", 28: "WM_MOVED", 29: "WM_NEWTOP", 30: "WM_UNTOPPED",
    31: "WM_ONTOP", 33: "WM_BOTTOM", 34: "WM_ICONIFY", 35: "WM_UNICONIFY",
    36: "WM_ALLICONIFY", 37: "WM_TOOLBAR",
    40: "AC_OPEN", 41: "AC_CLOSE",
    50: "CT_UPDATE", 51: "CT_MOVE", 52: "CT_NEWTOP", 53: "CT_KEY",
}

# evnt_multi MU_* bits — EmuTOS `include/aesdefs.h`.
MU_BITS = [(0x01, "KEYBD"), (0x02, "BUTTON"), (0x04, "M1"), (0x08, "M2"),
           (0x10, "MESAG"), (0x20, "TIMER")]

# wind_get / wind_set field names — EmuTOS `include/aesdefs.h` WF_*.
WF_NAME = {
    1: "WF_KIND", 2: "WF_NAME", 3: "WF_INFO", 4: "WF_WORKXYWH",
    5: "WF_CURRXYWH", 6: "WF_PREVXYWH", 7: "WF_FULLXYWH", 8: "WF_HSLIDE",
    9: "WF_VSLIDE", 10: "WF_TOP", 11: "WF_FIRSTXYWH", 12: "WF_NEXTXYWH",
    14: "WF_NEWDESK", 15: "WF_HSLSIZE", 16: "WF_VSLSIZE", 17: "WF_SCREEN",
    18: "WF_COLOR", 19: "WF_DCOLOR", 20: "WF_OWNER", 21: "WF_BEVENT",
    22: "WF_BOTTOM", 23: "WF_ICONIFY", 24: "WF_UNICONIFY",
    25: "WF_UNICONIFYXYWH", 26: "WF_TOOLBAR", 27: "WF_FTOOLBAR",
    28: "WF_NTOOLBAR", 29: "WF_MENU",
}

# wind_update / graf_mouse modes.
WU_NAME = {0: "END_UPDATE", 1: "BEG_UPDATE", 2: "END_MCTRL", 3: "BEG_MCTRL"}
GM_NAME = {0: "ARROW", 1: "TEXT_CRSR", 2: "HOURGLASS", 3: "POINT_HAND",
           4: "FLAT_HAND", 5: "THIN_CROSS", 6: "THICK_CROSS",
           7: "OUTLN_CROSS", 255: "USER_DEF", 256: "M_OFF", 257: "M_ON",
           258: "M_SAVE", 259: "M_LAST", 260: "M_RESTORE", 261: "M_PREVIOUS"}


def bits(v, table):
    """Render a bitmask as NAME|NAME, or '-' when zero."""
    v &= 0xFFFF
    on = [n for b, n in table if v & b]
    return "|".join(on) if on else "-"


class Rec:
    __slots__ = ("seq", "passes", "t", "dur", "ain0", "aout0", "op", "pid",
                 "in_len", "out_len", "ain_len", "kind", "flags", "slot",
                 "nin", "nout", "mb_or", "mb_and", "w")

    def __init__(self, raw):
        f = struct.unpack(REC_FMT, raw)
        (self.seq, self.passes, self.t, self.dur, self.ain0, self.aout0,
         self.op, self.pid, self.in_len, self.out_len, self.ain_len, _rsv,
         self.kind, self.flags, self.slot, self.nin, self.nout,
         self.mb_or, self.mb_and) = f[:19]
        self.w = list(f[19:43])

    @property
    def i(self):
        """The captured `int_in` prefix."""
        return self.w[:self.nin]

    @property
    def o(self):
        """The captured `int_out` words (the whole dispatch-local array)."""
        return self.w[self.nin:self.nin + self.nout]

    def iw(self, n, default=0):
        return self.i[n] if n < len(self.i) else default

    def ow(self, n, default=0):
        return self.o[n] if n < len(self.o) else default


def parse_header(blob):
    """Return the header line as a dict of key=value plus the raw text."""
    line = blob[:HDR_LEN].decode("ascii", "replace").rstrip()
    fields = {}
    for tok in line.split():
        if "=" in tok:
            k, v = tok.split("=", 1)
            fields[k] = v
        else:
            fields.setdefault("_tags", []).append(tok)
    return line, fields


def detail(r):
    """The per-opcode field decoding: name the words that matter for this call.

    The capture side is deliberately dumb (a uniform `int_in` prefix + the
    whole `int_out`); all opcode knowledge lives here.
    """
    op = r.op
    if op == 25:  # evnt_multi
        flags = r.iw(0) & 0xFFFF
        fired = r.ow(0) & 0xFFFF
        m1 = (r.iw(4), r.iw(5), r.iw(6), r.iw(7), r.iw(8))
        m2 = (r.iw(9), r.iw(10), r.iw(11), r.iw(12), r.iw(13))
        ms = ((r.iw(15) & 0xFFFF) << 16) | (r.iw(14) & 0xFFFF)
        return (
            f"want={flags:#06x}({bits(flags, MU_BITS)}) "
            f"bclk={r.iw(1)} bmask={r.iw(2):#04x} bstate={r.iw(3):#04x} "
            f"m1={'leave' if m1[0] else 'enter'}{m1[1:]} "
            f"m2={'leave' if m2[0] else 'enter'}{m2[1:]} timer={ms}ms\n"
            f"{'':>44}-> fired={fired:#06x}({bits(fired, MU_BITS)}) "
            f"mx={r.ow(1)} my={r.ow(2)} mb={r.ow(3) & 0xFFFF:#04x} "
            f"ks={r.ow(4) & 0xFFFF:#04x} key={r.ow(5) & 0xFFFF:#06x} "
            f"bret={r.ow(6)}"
        )
    if op == 21:  # evnt_button
        return (f"bclk={r.iw(0)} bmask={r.iw(1):#04x} bstate={r.iw(2):#04x} "
                f"-> clicks={r.ow(0)} mx={r.ow(1)} my={r.ow(2)} "
                f"mb={r.ow(3) & 0xFFFF:#04x} ks={r.ow(4) & 0xFFFF:#04x}")
    if op == 22:  # evnt_mouse
        return (f"{'leave' if r.iw(0) else 'enter'}"
                f"({r.iw(1)},{r.iw(2)},{r.iw(3)},{r.iw(4)}) "
                f"-> ret={r.ow(0)} mx={r.ow(1)} my={r.ow(2)} "
                f"mb={r.ow(3) & 0xFFFF:#04x}")
    if op == 20:  # evnt_keybd
        return f"-> key={r.ow(0) & 0xFFFF:#06x}"
    if op == 24:  # evnt_timer
        ms = ((r.iw(1) & 0xFFFF) << 16) | (r.iw(0) & 0xFFFF)
        return f"{ms}ms -> ret={r.ow(0)}"
    if op == 23:  # evnt_mesag
        return f"buf={r.ain0:#x} -> ret={r.ow(0)}"
    if op == 26:  # evnt_dclick
        return f"rate={r.iw(0)} setit={r.iw(1)} -> index={r.ow(0)}"
    if op == 78:  # graf_mouse
        m = r.iw(0)
        return f"mode={m}({GM_NAME.get(m, '?')}) form={r.ain0:#x} -> {r.ow(0)}"
    if op == 79:  # graf_mkstate
        return (f"-> ret={r.ow(0)} mx={r.ow(1)} my={r.ow(2)} "
                f"mb={r.ow(3) & 0xFFFF:#04x} ks={r.ow(4) & 0xFFFF:#04x}")
    if op == 107:  # wind_update
        m = r.iw(0)
        return f"mode={m}({WU_NAME.get(m, '?')}) -> {r.ow(0)}"
    if op == 104:  # wind_get
        f_ = r.iw(1)
        return (f"handle={r.iw(0)} field={f_}({WF_NAME.get(f_, '?')}) "
                f"-> ret={r.ow(0)} ({r.ow(1)},{r.ow(2)},{r.ow(3)},{r.ow(4)})")
    if op == 105:  # wind_set
        f_ = r.iw(1)
        return (f"handle={r.iw(0)} field={f_}({WF_NAME.get(f_, '?')}) "
                f"w=({r.iw(2)},{r.iw(3)},{r.iw(4)},{r.iw(5)}) "
                f"str={r.ain0:#x} -> ret={r.ow(0)}")
    if op == 100:  # wind_create
        return (f"kind={r.iw(0) & 0xFFFF:#06x} "
                f"full=({r.iw(1)},{r.iw(2)},{r.iw(3)},{r.iw(4)}) "
                f"-> handle={r.ow(0)}")
    if op in (101, 108):  # wind_open / wind_calc
        return (f"in=({r.iw(0)},{r.iw(1)},{r.iw(2)},{r.iw(3)},{r.iw(4)},"
                f"{r.iw(5)}) -> ret={r.ow(0)} "
                f"({r.ow(1)},{r.ow(2)},{r.ow(3)},{r.ow(4)})")
    if op in (102, 103):  # wind_close / wind_delete
        return f"handle={r.iw(0)} -> ret={r.ow(0)}"
    if op == 106:  # wind_find
        return f"({r.iw(0)},{r.iw(1)}) -> handle={r.ow(0)}"
    if op == 42:  # objc_draw
        return (f"tree={r.ain0:#x} start={r.iw(0)} depth={r.iw(1)} "
                f"clip=({r.iw(2)},{r.iw(3)},{r.iw(4)},{r.iw(5)}) "
                f"-> ret={r.ow(0)}")
    if op == 43:  # objc_find
        return (f"tree={r.ain0:#x} start={r.iw(0)} depth={r.iw(1)} "
                f"at=({r.iw(2)},{r.iw(3)}) -> obj={r.ow(0)}")
    if op == 47:  # objc_change
        return (f"tree={r.ain0:#x} obj={r.iw(0)} "
                f"clip=({r.iw(2)},{r.iw(3)},{r.iw(4)},{r.iw(5)}) "
                f"newstate={r.iw(6) & 0xFFFF:#06x} redraw={r.iw(7)} "
                f"-> ret={r.ow(0)}")
    if op == 44:  # objc_offset
        return (f"tree={r.ain0:#x} obj={r.iw(0)} "
                f"-> ret={r.ow(0)} x={r.ow(1)} y={r.ow(2)}")
    if op == 77:  # graf_handle
        return (f"-> vdi={r.ow(0)} wchar={r.ow(1)} hchar={r.ow(2)} "
                f"wbox={r.ow(3)} hbox={r.ow(4)}")
    if op == 30:  # menu_bar
        return f"tree={r.ain0:#x} show={r.iw(0)} -> ret={r.ow(0)}"
    if op == 10:  # appl_init
        return f"-> ap_id={r.ow(0)}"
    if op == 112:  # rsrc_gaddr
        return (f"type={r.iw(0)} index={r.iw(1)} "
                f"-> ret={r.ow(0)} addr={r.aout0:#x}")
    # Generic: the raw words, which is still enough to read a call.
    inp = " ".join(str(v) for v in r.i)
    out = " ".join(str(v) for v in r.o)
    a = f" ain0={r.ain0:#x}" if r.ain0 else ""
    b = f" aout0={r.aout0:#x}" if r.aout0 else ""
    return f"in=[{inp}]{a} -> out=[{out}]{b}"


def render(recs, hz, t0, out=sys.stdout):
    def us(t):
        return (t - t0) * 1_000_000 // hz if hz else 0

    print(f"{'seq':>7} {'t_ms':>11} {'dur_us':>9} {'sl':>3} {'pid':>3} "
          f"{'op':>4} {'name':<14} {'i/o/a':<9} detail", file=out)
    print("-" * 120, file=out)
    for r in recs:
        t_ms = us(r.t) / 1000.0
        dur_us = (r.dur * 1_000_000 // hz) if hz else 0
        sl = "K" if r.slot == 0xFF else str(r.slot)
        if r.kind == KIND_CALL:
            name = AES_NAME.get(r.op, "?")
            counts = f"{r.in_len}/{r.out_len}/{r.ain_len}"
            extra = ""
            if r.flags & F_YIELDED:
                extra += " [YIELDED — no result, svc will re-execute]"
            if r.flags & F_NO_INTIN:
                extra += " [int_in NULL]"
            if r.passes:
                extra += (f" [evals={r.passes} mb_or={r.mb_or:#04x} "
                          f"mb_and={r.mb_and:#04x}]")
            elif r.kind == KIND_CALL and 20 <= r.op <= 25:
                # The wait never evaluated its predicate in a sampled loop
                # (e.g. evnt_button whose button already matched on entry):
                # mb_or/mb_and hold no sample, so do not print one.
                extra += " [evals=0 — fired without a sampled evaluation]"
            print(f"{r.seq:>7} {t_ms:>11.3f} {dur_us:>9} {sl:>3} {r.pid:>3} "
                  f"{r.op:>4} {name:<14} {counts:<9} {detail(r)}{extra}",
                  file=out)
        elif r.kind in (KIND_MSG_POST, KIND_MSG_RECV):
            tag = KIND_NAME[r.kind]
            mt = r.w[0]
            words = " ".join(str(v) for v in r.w[:max(r.nin, 8)])
            fail = " [QUEUE FULL — DROPPED]" if r.flags & F_POST_FAILED else ""
            print(f"{r.seq:>7} {t_ms:>11.3f} {'':>9} {sl:>3} {r.pid:>3} "
                  f"{'':>4} {tag + ' ' + MSG_NAME.get(mt, str(mt)):<14} "
                  f"{'':<9} msg=[{words}] sender={r.w[1]} wnd={r.w[3]}{fail}",
                  file=out)
        else:
            print(f"{r.seq:>7} {t_ms:>11.3f} {'':>9} {sl:>3} {r.pid:>3} "
                  f"{'':>4} {'MARK':<14}", file=out)


MU_BUTTON = 0x02


def button_report(recs, hz, out=sys.stdout):
    """The `evnt_multi`/`evnt_button` button-condition report.

    This is the view that decides whether the AES is reporting a *level*
    ("the button is currently in the state you asked about") instead of an
    *event* ("the button just changed"). EmuTOS gates its level quick-check on
    mouse ownership — `if (rlr == gl_mowner)`, `aes/gemevlib.c:186-203` — so an
    application that has not called `wind_update(BEG_MCTRL)` never gets the
    level test; it blocks in `ev_block` and is woken by real button events.

    The falsifiable question: with the pointer merely moving and no button
    touched, does MU_BUTTON come back on nearly every call?
    """
    calls = [r for r in recs if r.kind == KIND_CALL and r.op in (21, 25)]
    if not calls:
        print("no evnt_button / evnt_multi calls in this trace", file=out)
        return

    mctrl = [r for r in recs
             if r.kind == KIND_CALL and r.op == 107 and r.iw(0) in (2, 3)]
    print("wind_update mouse-ownership calls "
          "(BEG_MCTRL=3 / END_MCTRL=2) — EmuTOS gates its level "
          "quick-check on this:", file=out)
    if mctrl:
        for r in mctrl:
            t_ms = ((r.t - t0_of(recs)) * 1000 // hz) if hz else 0
            print(f"   t={t_ms} ms  mode={r.iw(0)}"
                  f"({WU_NAME.get(r.iw(0), '?')})  pid={r.pid}", file=out)
    else:
        print("   NONE — no application ever took mouse control.", file=out)
    print(file=out)

    asked = [r for r in calls if r.op == 21 or (r.iw(0) & MU_BUTTON)]
    fired = [r for r in asked if (r.ow(0) & MU_BUTTON) or r.op == 21]
    print(f"evnt_button/evnt_multi calls: {len(calls)}", file=out)
    print(f"  ... asking for a button event (MU_BUTTON set / evnt_button): "
          f"{len(asked)}", file=out)
    print(f"  ... that returned a button event:                            "
          f"{len(fired)}", file=out)
    if asked:
        print(f"  ... hit rate: {100.0 * len(fired) / len(asked):.1f} %",
              file=out)

    # Requested (mask, state) histogram — "mask 1, state 0" is the
    # 'tell me when the button is NOT pressed' request.
    from collections import Counter
    req = Counter()
    for r in asked:
        if r.op == 25:
            req[(r.iw(2) & 0xFFFF, r.iw(3) & 0xFFFF, r.iw(1))] += 1
        else:
            req[(r.iw(1) & 0xFFFF, r.iw(2) & 0xFFFF, r.iw(0))] += 1
    print("\n  requested (bmask, bstate, bclicks):", file=out)
    for (m, st, cl), n in req.most_common():
        note = ""
        if m and (st & m) == 0:
            note = "   <-- 'button UP' request: a LEVEL test matches whenever "\
                   "the button is not pressed"
        print(f"    mask={m:#04x} state={st:#04x} clicks={cl}: {n} call(s)"
              f"{note}", file=out)

    # Rate of button-returning calls, and what the sampled button byte was.
    if fired and hz:
        span = (fired[-1].t + fired[-1].dur - fired[0].t) / hz
        if span > 0:
            print(f"\n  button-returning calls over {span:.2f} s = "
                  f"{len(fired) / span:.1f} per second", file=out)
    sampled_up = sum(1 for r in fired if r.passes and r.mb_or == 0)
    print(f"  of those, {sampled_up} sampled the button as UP (0x00) on "
          f"EVERY evaluation of the call", file=out)
    print("  (a button event that fires while the button was never pressed "
          "is a level test, not an event)", file=out)


def t0_of(recs):
    return recs[0].t if recs else 0


def summary(recs, hz, out=sys.stdout):
    from collections import Counter
    calls = Counter()
    dur = Counter()
    for r in recs:
        if r.kind == KIND_CALL:
            calls[r.op] += 1
            dur[r.op] += r.dur
    print(f"{'op':>4} {'name':<16} {'calls':>7} {'total_ms':>10} "
          f"{'avg_us':>10}", file=out)
    print("-" * 52, file=out)
    for op, n in calls.most_common():
        tot_us = (dur[op] * 1_000_000 // hz) if hz else 0
        print(f"{op:>4} {AES_NAME.get(op, '?'):<16} {n:>7} "
              f"{tot_us / 1000.0:>10.1f} {tot_us // n:>10}", file=out)
    posts = sum(1 for r in recs if r.kind == KIND_MSG_POST)
    recvs = sum(1 for r in recs if r.kind == KIND_MSG_RECV)
    print(f"\nmessages: {posts} posted, {recvs} delivered", file=out)


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("file")
    ap.add_argument("--op", type=int, action="append",
                    help="only this AES opcode (repeatable)")
    ap.add_argument("--tail", type=int, help="only the last N records")
    ap.add_argument("--head", type=int, help="only the first N records")
    ap.add_argument("--summary", action="store_true",
                    help="per-opcode call/time summary instead of the trace")
    ap.add_argument("--events", action="store_true",
                    help="only evnt_* calls and message records")
    ap.add_argument("--button", action="store_true",
                    help="button-condition report: what the app asked for, "
                         "what was sampled, what fired, and whether any app "
                         "took mouse control (wind_update BEG_MCTRL)")
    args = ap.parse_args()

    blob = open(args.file, "rb").read()
    if not blob.startswith(b"NEWTOS-AESTRACE"):
        sys.exit(f"{args.file}: not a NewTOS AES trace "
                 f"(header {blob[:16]!r})")
    # Refuse a version this decoder does not lay out identically. Silently
    # mis-parsing a differently-shaped file is exactly the failure mode
    # `docs/TOS-REFERENCE-DUMPER.md` §3 warns about.
    if not blob.startswith(FORMAT):
        sys.exit(f"{args.file}: format {blob[:19].decode('ascii', 'replace')!r}"
                 f", this decoder speaks {FORMAT.decode()!r} — rebuild the "
                 f"kernel or use the matching decoder")
    line, h = parse_header(blob)
    print(line)
    hz = int(h.get("hz", 0))
    t0 = int(h.get("t0", 0))
    rec_len = int(h.get("rec", REC_LEN))
    if rec_len != REC_LEN:
        sys.exit(f"record size {rec_len} != decoder's {REC_LEN} — "
                 f"the target and this decoder are out of sync")
    n = int(h.get("n", 0))
    produced = int(h.get("produced", n))
    if produced > n:
        print(f"*** TRUNCATED: the ring wrapped, {produced - n} older "
              f"records were dropped; this file holds the newest {n}.")
    else:
        print(f"*** COMPLETE: all {n} records produced are in this file.")
    ovh = h.get("ovh", "0/0")
    tot, hooks = (ovh.split("/") + ["0"])[:2]
    if hz and int(hooks):
        tot_us = int(tot) * 1_000_000 // hz
        print(f"*** capture cost: {tot_us} us over {hooks} hooks "
              f"= {tot_us / int(hooks):.2f} us/record mean, "
              f"{int(h.get('ovhmax', 0)) * 1_000_000 / hz:.2f} us worst "
              f"(this is the number that must not perturb timing)")
    flovh = h.get("flovh", "0/0")
    ftot, nfl = (flovh.split("/") + ["0"])[:2]
    if hz and int(nfl):
        ftot_us = int(ftot) * 1_000_000 // hz
        print(f"*** flush cost:   {ftot_us / 1000.0:.1f} ms over {nfl} "
              f"flush(es) — runs only while the user is idle")
    print("*** note: a CALL record's t is its ENTRY time and dur its length, "
          "so a POST/RECV")
    print("***       emitted inside a call has a LOWER seq but a t inside "
          "that call's span.")
    print()

    body = blob[HDR_LEN:]
    avail = len(body) // REC_LEN
    if avail != n:
        print(f"*** note: header says n={n}, file holds {avail} records "
              f"(short write / interrupted flush)")
    recs = [Rec(body[i * REC_LEN:(i + 1) * REC_LEN]) for i in range(avail)]

    if args.op:
        want = set(args.op)
        recs = [r for r in recs if r.kind != KIND_CALL or r.op in want]
    if args.events:
        recs = [r for r in recs
                if (r.kind == KIND_CALL and 20 <= r.op <= 26)
                or r.kind in (KIND_MSG_POST, KIND_MSG_RECV)]
    if args.head:
        recs = recs[:args.head]
    if args.tail:
        recs = recs[-args.tail:]

    if args.button:
        button_report(recs, hz)
    elif args.summary:
        summary(recs, hz)
    else:
        render(recs, hz, t0)


if __name__ == "__main__":
    main()
