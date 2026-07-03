# Step 2 — code-vs-docs validation backlog

> **What this is.** Step 1 (the Pi/BCM2711 hardware binding, see
> `docs/HARDWARE-PROFILE.md`) re-points every spec's hardware reference at the
> real BCM2711 and pins the parametric values. While doing so, each binding
> agent recorded places where the **code does not (yet) match the contract**.
> This file is the resulting **Step-2 backlog**: the concrete
> code-vs-docs/contract fixes to make, subsystem by subsystem. No code is
> changed here — this is the to-do list Step 2 works through.
>
> Severity: **BUG** (wrong observable behaviour) · **GAP** (contract step
> unimplemented) · **TEXT** (spec text still cites EmuTOS figures) · **NOTE**
> (harmless divergence / bring-up limit).

---

## Screen / framebuffer

| # | Sev | Item | Where |
|---|-----|------|-------|
| S1 | BUG | XBIOS `Setpalette`/`Setcolor` write a separate ST-format shadow (`xbios/screen.rs`) that is **not** wired to the VDI CLUT (`vdi_backend::PALETTE`), so they change no displayed colour; the "apply at next VBL" timing is unmodelled. | `tos-api/src/xbios/screen.rs` |
| S2 | GAP | `Scrdmp` (XBIOS 0x14) is not wired in the XBIOS dispatcher (no 0x14 entry); the "reset `dumpflg`" contract step falls through. (No-op is correct for the printer-less Pi, but the call should exist.) | `tos-api/src/xbios/mod.rs` |
| S3 | NOTE | `Getrez` returns `2` (`ST_HIGH`, 640×400 mono) for a 640×480×16 mode; `MAX_REZ=2` blocks `Setscreen` from naming the real mode. Decision: keep `2` until the HDMI-mode-query (1080p) phase. | `tos-api/src/xbios/screen.rs:25,57` |
| S4 | NOTE | `Setscreen` has no rez-change side effects (no screen-clear / cursor-home / VT52-reset) and does not re-point the VideoCore scan-out (bounded no-op). Harmless while the mode is fixed. | `tos-api/src/xbios/screen.rs:72` |
| S5 | NOTE | `Vsync` is a synthesized 60 Hz from the generic timer (`CNTPCT*60/CNTFRQ`), not a real VideoCore vblank — terminates but not tear-free. | `kernel/src/host_env.rs:191` |
| S6 | TEXT | `v_opnvwk`/`vq_extnd` specs still cite EmuTOS `NUM_VDI_HANDLES` 64/128 and `MAX_VERTICES` 512/1024; NewTOS is **16 / 256**. Update the spec text. | `docs/api/vdi/workstation/*` |
| S7 | NOTE | Pixel size `DEV_TAB[3,4]` / `INQ_TAB[20..]` left at ROM `372` / 0 — not patched from the HDMI physical-display-size mailbox query. Revisit with 1080p. | `tos-api/src/vdi/workstation.rs:101` |
| S8 | NOTE | `Cursconf` internal flag bits differ from EmuTOS (`M_CFLASH=0x02`, `M_CVIS=0x10`) — write-only, not ABI-observable. | `tos-api/src/xbios/screen.rs` |
| S9 | NOTE | Off-screen colour MFDB↔MFDB copies are bring-up-limited (screen-form cases covered; full 16-op across arbitrary buffers is later work). | `tos-api/src/vdi/` backend |

## Keyboard / Mouse / Console

| # | Sev | Item | Where |
|---|-----|------|-------|
| K1 | BUG | **Console policy violation:** the console RX ring is fed **both** by the USB-HID keyboard (`push_rx`) **and** by the PL011 UART0 (`pl011_rx_drain`/`pl011_rx_poll`), and the blocking/`pump_input` reads actively poll UART0 on real Pi-4. Per `HARDWARE-PROFILE.md` §3.2 the TOS console (CON, handle 2) must **not** read the UART — it should read only the USB-HID keyboard. `Cconin`/`Cnecin`/`Bconin(2)`/`v_string`/`v_locator` all inherit this. | `kernel/src/console.rs:91-107`; `kernel/src/host_env.rs:37-77,105-136` |
| K2 | BUG | **`Bconin(2)` drops the scancode high word.** Contract: CON returns a packed LONG with the IKBD scancode in bits 16-23, ASCII in bits 0-7 (non-ASCII keys → ASCII 0 + scancode). NewTOS returns `console_read_byte_blocking() as Long` = ASCII only; non-ASCII keys (arrows/function) are invisible to `Bconin(2)`/`Cconin`, and ASCII keys report scancode 0. The scancode **is** in the ring word (`scancode<<8\|ascii`) and exposed by `rx_pop_key`, but `Bconin`/`Cconin` use the ASCII-only `rx_pop`. | `tos-api/src/bios/chardev.rs:49-55`; `kernel/src/console.rs:120-133` |
| K3 | GAP | **XBIOS keyboard group not wired.** `Initmous (0x00)`, `Iorec (0x0E)`, `Keytbl (0x10)`, `Bioskeys (0x18)`, `Ikbdws (0x19)`, `Kbrate (0x23)` have **no entry** in the XBIOS dispatcher → return `EINVFN`. Only `Kbdvbase (0x22)` is present. The keyboard/mouse work via the direct `ikbd::feed` path, but the XBIOS API surface is absent. | `tos-api/src/xbios/mod.rs:33-55` |
| K4 | GAP | **`Kbdvbase` table never populated / never dispatched through.** `KBDVECS` is all-zero and no boot code installs default handlers; the IKBD decoder calls `vdi::feed_relative` directly instead of dispatching through `kbdvecs.mousevec`. So `Kbdvbase()->mousevec == 0` and hooking a slot has no effect. Stable-pointer + 9-slot-layout contract holds; the dispatch-through-vectors contract does not. | `tos-api/src/xbios/system.rs:42-78` |
| K5 | GAP | **VDI `vex_*` mouse vectors never invoked.** `vex_butv/motv/curv/wheelv` (125-127,134) and `vex_timv` (118) install/return their slots correctly, but the mouse packet path (`feed_relative`) does not call `user_but`/`user_mot`/`user_cur` in the button→motion→cursor order, and no timer tick invokes `tim_addr`. A hooked vector never fires; the AES event path uses the direct call instead. | `tos-api/src/vdi/mouse.rs:105-134,359-388` |
| K6 | GAP | **`cur_ms_stat` + mouse-button locator terminators missing.** `feed_relative`/`set_state` update `GCURX`/`GCURY`/`MOUSE_BT` but not `cur_ms_stat` (motion bit 5, button-change bits 6/7). So `v_locator` does not produce the `0x20`/`0x21` mouse-button terminators — only keyboard terminators. | `tos-api/src/vdi/mouse.rs:105-134`; `tos-api/src/vdi/input.rs:64-73` |
| K7 | GAP | **No software key-repeat.** The decoder emits one key word per make event; there is no `kb_ticks`-style repeat timer keyed off `kb_initial`/`kb_repeat`, so even if `Kbrate` were wired its stored values would have no observable effect. | `kernel/src/ikbd.rs:134-140` |
| K8 | GAP | **VT52 escape sequences unimplemented.** `fbcon.rs` handles printable glyphs + CR/LF/BS/TAB + wrap + upward scroll only; the handle-2 `Bconout`/VT52 contract (cursor addressing, clear-to-EOL, colour, etc.) is not parsed. (Overlaps the Screen section.) | `kernel/src/fbcon.rs:62-127` |
| K9 | NOTE | **AUX (handle 1) is a BIOS dummy.** The PL011 exists but `is_console_in`/`is_console_out` cover only CON(2)/IKBD(4)/raw-CON(5), so `Bconstat(1)`/`Bconin(1)`/`Bconout(1)`/`Bcostat(1)` return 0 (absent) instead of backing AUX with the UART. The UART is instead written as the CON debug mirror (output) and leaked into the CON input ring (K1). A proper AUX binding would separate it. | `tos-api/src/bios/chardev.rs:24-32` |
| K10 | NOTE | **`v_choice` reads the keyboard instead of returning constant 1.** EmuTOS's `v_choice` is a DRI dummy returning `TERM_CH=1` unconditionally; NewTOS blocks on a key read in REQUEST and returns it (0 in SAMPLE). Divergence from the documented dummy behaviour. | `tos-api/src/vdi/input.rs:77-85` |
| K11 | NOTE | **Middle mouse button + wheel dropped.** USB byte-0 bit 2 (middle) and byte 3 (wheel) are discarded in the USB→IKBD synthesis; only left/right map into the 3-byte relative packet. Extended buttons 3-5 (`CONF_WITH_EXTENDED_MOUSE`) would need separate status packets. | `kernel/src/drivers/usb/hid.rs:106-112` |
| K12 | NOTE | **`Bcostat` MIDI/IKBD swap unobservable.** The handle-3↔4 status-slot swap is contract-relevant only if MIDI/IKBD-TX exist; both are absent on the Pi, so handles 3 and 4 both return 0. No explicit swap table needed today, but re-check if MIDI/IKBD ever return. | `tos-api/src/bios/chardev.rs:76-82` |
| K13 | TEXT | **Stale "not wired / parked cursor" notes.** `tos-api/src/vdi/mouse.rs` module doc + `set_state` note and the `_mouse-packet.md`/`_mouse.md` notes state the mouse is "not yet wired" and `vq_mouse` returns a parked cursor — stale since the USB-HID mouse feeds `feed_relative`. `_mouse-packet.md` also says packets arrive "on UART2 and are discarded" (the shelved plan); they arrive over USB-HID and move the cursor. Update the code comments + spec prose. | `tos-api/src/vdi/mouse.rs:1-10,99-115`; `docs/api/vdi/input/_mouse-packet.md`, `_mouse.md` |

<!-- Subsequent binding agents append their subsystem section below. -->
