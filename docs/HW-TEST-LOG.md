# HW-TEST-LOG — Real Raspberry Pi 4 bring-up

> Manual hardware verification log + setup guide for booting NewTOS on
> a physical Raspberry Pi 4 (BCM2711) over a serial console. Closes
> PHASE-01 **EC2** once the boot banner appears on real hardware.
>
> **First metal boot succeeded 2026-06-17** (graphics path: VideoCore
> framebuffer test pattern on HDMI; serial console not yet wired). The
> boot/MMU/heap path and the BCM2711 mailbox framebuffer run on real
> silicon. Serial-console verification (boot banner + self-tests over
> PL011) still pending the USB-TTL adapter — see §7.

---

## 1. What you need

- A Raspberry Pi 4 (any RAM size — the kernel currently uses the first
  1 GiB unless the firmware-supplied DTB reports more).
- A micro-SD card (any size; we only use a small FAT32 boot partition).
- A **3.3 V** USB-to-TTL serial adapter (e.g. CP2102 / FT232 / CH340).
  **Do not** drive the Pi's RX with 5 V signalling — the GPIO is 3.3 V
  and 5 V will damage it. If the adapter is level-selectable
  (e.g. a "3.3 / 5 V" cable), select **3.3 V**.
  - **In use here:** Waveshare USB-TO-TTL (D), **FT232RNL**, USB-A,
    SH1.0 3-pin (TXD / RXD / GND), 3.3 / 5 V selectable — use the
    **3.3 V** setting. FTDI wire convention: black = GND, yellow = the
    adapter's RXD, orange = the adapter's TXD (verify against the
    cable's own labels).
- A host terminal at **115200 baud, 8N1, no flow control**.
  - **macOS** (FT232R works with the built-in driver — no install on
    recent macOS):
    ```sh
    ls /dev/tty.usbserial-*                  # find the device
    screen /dev/tty.usbserial-XXXX 115200    # exit: Ctrl-A then K
    ```
  - Alternatives: `minicom`, `picocom -b 115200 /dev/tty.usbserial-XXXX`,
    or PuTTY.

## 2. Wiring (USB-TTL ↔ Pi 4 GPIO header)

The kernel drives **PL011 / UART0** on GPIO 14/15. Connect three wires
(TX↔RX crossed), sharing a common ground. **Leave the adapter's 5 V /
3.3 V power pin disconnected** — the Pi is powered separately.

| USB-TTL pin | Pi 4 GPIO (BCM) | Pi 4 header pin |
|-------------|-----------------|-----------------|
| RX  (adapter in)  | GPIO 14 — TXD0 | pin 8  |
| TX  (adapter out) | GPIO 15 — RXD0 | pin 10 |
| GND               | GND            | pin 6  |

(Adapter RX ← Pi TX, adapter TX → Pi RX. If you only see output but
typing does nothing, you likely swapped TX/RX or left GND off.)

## 3. SD-card layout (FAT32 boot partition)

Format the card with a single FAT32 partition and copy:

1. **Raspberry Pi 4 firmware** (from the official `raspberrypi/firmware`
   `boot/` folder): `start4.elf` and `fixup4.dat` (the minimum for a
   bare-metal 64-bit kernel; `bootcode.bin` is **not** used on the Pi 4).
2. **`config.txt`** — use [`boot/rpi4/config.txt`](../boot/rpi4/config.txt)
   from this repo. The `init_uart_clock=48000000` line is mandatory.
3. **`kernel8.img`** — built below.

That's it. No `cmdline.txt` is required for a bare-metal kernel.

## 4. Build `kernel8.img`

```sh
cargo xtask build --board rpi4 --release
# → produces ./kernel8.img at the workspace root (raw binary via objcopy)
```

Copy that `kernel8.img` to the SD card's boot partition.

## 5. Boot

1. Insert the SD card, connect the USB-TTL wires, open the host terminal
   at 115200 8N1.
2. Power the Pi.
3. **Expected first output** (within a second or two):

   ```
   RB
   NewTOS booting on rpi4
   Phase 02 reached.
   [mm] ...
   ```

   `RB` is the **pre-MMU boot heartbeat** — the boot stub writes it to
   the PL011 data register as its very first action, before any MMU /
   stack / BSS setup. Seeing `RB` confirms the firmware loaded
   `kernel8.img`, the CPU reached our entry point, and the serial path
   + baud are correct. The banner and self-test sequence follow (the
   same one QEMU prints: `[selftest p04] ... 10 passed`, `EC4/D3/D6/D7
   PASS`, `D3/D4D5 PEXEC PASS`, `EINVFN OK`, then the `[cconin]` and
   `RX echo` prompts — type a key to see it echoed).

## 6. Troubleshooting

| Symptom | Likely cause |
|---|---|
| **Nothing at all — not even `RB`** | The CPU never reached our entry, or serial is dead: wrong baud (must be 115200); TX/GND not connected or TX/RX swapped; `config.txt` missing (`enable_uart=1` / `arm_64bit=1`); firmware files (`start4.elf`/`fixup4.dat`) or `kernel8.img` missing/misnamed. |
| **`RB` then nothing** | Serial + firmware + entry are **fine** — a boot stage *after* the heartbeat faulted (MMU stage-1, BSS, stack, or the high-half jump). This narrows it to `boot.rs` / `mmu.rs`; capture exactly how far it got. |
| **Garbage / random bytes** | UART clock mismatch — confirm `init_uart_clock=48000000` is in `config.txt`. This is the #1 Pi 4 cause. |
| **Output but typing does nothing** | Adapter TX → Pi RX (GPIO 15) not connected, or TX/RX swapped. |
| **Banner then hangs / faults** | A real-hardware MMU / cache / boot-stage difference vs QEMU — capture the full log and the `[EXC]` line if any, and record it below. |

The pre-MMU `RB` heartbeat (boot stub) makes the silent-early-failure
case diagnosable: **no `RB`** = serial/firmware/entry problem; **`RB`
then silence** = a fault in our own early boot code.

---

## 7. Test log

Record each metal boot attempt here (date, firmware version, RAM size,
what appeared, any fault).

| Date | Pi model / RAM | Firmware | Result | Notes |
|------|----------------|----------|--------|-------|
| 2026-06-17 | RPi 4 Model B | `start4.elf`/`fixup4.dat` (card, 2026-04-17) | ✅ **First metal boot — VideoCore framebuffer test pattern visible on HDMI** (red / green+white-box / blue, 640×480 on a 1080p screen) | **Graphics path only** (no serial yet). Confirms on real silicon: EL2→EL1 drop (firmware enters at EL2), mailbox property-interface FB allocation against real VideoCore firmware, **Non-Cacheable FB mapping** (without it the display DMA would not see the cached pixels — the exact failure QEMU's TCG cannot reproduce), BGR pixel order, and the whole MMU/heap boot path. SD card written from this repo (`kernel8.img` + `init_uart_clock=48000000` added to an existing RPi4 `config.txt`). |
| 2026-06-17 | RPi 4 Model B | as above | ✅ **First full VDI frame on metal** — the Phase-12 VDI demo (16 GEM palette tiles via `vr_recfl` + line grid/border via `v_pline`, every pixel through `svc #2`) renders correctly on HDMI. | **Found + fixed a real HW-only cache bug en route:** the first attempt showed the filled tiles sharp but the **lines as dashes / missing** — dense `fill_rect` writes evict whole cache lines (visible), sparse single-pixel `put_pixel` writes stay stranded in untouched lines (invisible). The Non-Cacheable FB mapping was *not* effective on real silicon for these writes; QEMU's TCG has no cache so it could never surface there. Fix: `mmu::clean_dcache_range` + clean each drawn region (`dc cvac` to PoC) after `fill_rect`/`draw_line`. After the fix the grid lines are solid. The diagnostic signature (dense OK, sparse broken) pinpointed it without a serial console. |
| 2026-09-05 | RPi 4 Model B | as above | ✅ **BIOS group proven from EL0** — `APITEST.NCP` (kernel `2908bbe`): **86 PASS / 0 FAIL / 1 SKIP**. All twelve trap-13 functions, 80 checks, documented return *and* documented error case each. Better than QEMU's 82/0/5: the five interactive `Bconin(2)` checks ran (BIOS-CHR 25 → 30) and `Drvmap` saw four drives. The one skip is `T29.3` (needs a child program that is not on the card). Yesterday this group stood at **zero** proven functions of twelve. |
| 2026-09-05 | RPi 4 Model B | as above | ✅ **An application can draw to the screen itself** — `PHYSBASE.NCP`: asks `Physbase()` over the TOS API, writes three colour bands with a plain store loop, **no cache maintenance**. Bands appear. This is ADR-023 (`2cf4fac`) plus the uncached exposure (`92e6276`) working on real caches, which QEMU cannot test — it has no cache model. |
| 2026-09-05 | RPi 4 Model B | as above | ✅ **Page flipping on real VideoCore firmware** — `SCRFLIP.NCP` (`2908bbe`): painted the hidden page, screen showed **nothing** (`HIDDEN PAGE PAINTED — screendump now must NOT show the bands`), then `Setscreen` flipped and the image stood full-screen. Nothing copied, only the scan-out moved. Mailbox tag `0x0004_8009`, sourced from `references/circle`. The last piece the October demo needed. |
| 2026-09-06 | RPi 4 Model B | as above | ✅ **The demo holds 60 Hz with nineteen times the headroom** — `BALL.NAP` (milestone 1, `cff7223`) at 1920×1080, two pages, ball d=306 px (the specified 16 % of screen width): **2118 frames in 35.30 s = 60.00 fps presented, 0 dropped**, and 0.86 ms of the 16.7 ms budget — 5 %. Free-running headroom 1160 fps. This is the first hardware measurement of what the uncached framebuffer exposure (ADR-023, `92e6276`) actually costs a drawing program, and the answer is: little, because the compose-in-RAM design sends exactly one rectangle per frame instead of individual pixels. It also settles that 16 % ball diameter carries at 1080p — no fallback to a smaller value needed — and leaves room for milestone 2 (the reflection doubles the rectangle to ~1.7 ms; texture mapping is pre-rendered and costs nothing per frame). Log kept as `sdk/apps/ball/BALL-hw-2026-09-06.LOG`. |
| 2026-09-07 | RPi 4 Model B | as above | ✅ **The textured sphere holds 60 Hz for a full minute with zero dropped frames** — `BALL.NAP` (milestone 2a, `8d5a972`) at 1920×1080: **3605 frames in 60.08 s = 60.01 fps, 0 dropped** in every 5 s interval, at **1.47 ms** of the 16.7 ms budget (free-running headroom 680 fps). The real texture mapping, the 45° spin tilt and the light-consistent shadow cost 1.7× milestone 1's flat-shaded 0.86 ms (`18d1bde`) and still leave eleven times the headroom. The start-up sphere map is 73568 px in 20.00 ms, one-time and invisible. **This also settles the open QEMU question**: QEMU raspi4b showed 0–6 dropped frames per 5 s interval where milestone 1 showed none; hardware shows zero across 60 s, so that was emulator jitter, not the renderer. Texture came from the built-in generator — no `C:\BALL.NTX` exists yet, the artwork is still to be supplied. Milestone 2b is comfortably funded: the reflection roughly doubles the dirty rectangle to ~2.9 ms. Log kept as `sdk/apps/ball/BALL-hw-2026-09-07-textured.LOG`. |
| 2026-09-07 | **Atari ST** + PiStorm + ET4000 emulation | 640×480×8, GEM desktop | ✅ **First run of the m68k build ever, and the runtime format probe worked** — `BALL.PRG` (`9e6deb4` + 2b) reports `pixel format MEASURED packed-8, fd_wdwidth = pixels/16, pen map MEASURED, VBL running (Vsync used)`. That answers C1, C2, C3 and C6 of `sdk/apps/ball/BALL-TT-CONFIRM.md` **by measurement, not assumption** — the `v_get_pixel` probe grips on real hardware instead of falling through to `assume_format()`. Ran **18421 frames in 370.01 s, 227 bounces, no crash**, at 49.79 fps presented with **4.00 ms of headroom per frame** (250 fps free-running), so the rate is capped by the ~50 Hz display, not the CPU. 275 frames dropped (1.5 %): a steady 1 per 5 s interval, which looks like a beat between the refresh and the 200 Hz tick rather than a real hitch, plus bursts of 7–13 every 15–20 s. **A second run without the screen recorder** (`sdk/apps/ball/BALL-pistorm-et4000-2026-09-07-norec.LOG`, 4523 frames / 90.84 s) refutes the recorder as the cause: the bursts persist, and they land at **the same timestamps in both runs** (15, 35, 50, 65, 85 s). Environment noise would not repeat; the demo is deterministic by design (fixed seed, fixed timestep), so this is tied to the animation itself and is reproducible. **RESOLVED (`7e6295c`): the bursts were never dropped frames.** Every interval holds exactly 249 frames, the quiet ones and the ones reporting 13 alike — 13 genuinely skipped retraces would cost 13 frames and 52 ticks; measured is zero frames fewer and one tick more. The cause is ours: `DROP_STEPS` is derived from 60 Hz as 4 ticks, but at 50 Hz 4 ticks *is* a frame's nominal duration, so the threshold sits exactly on the normal case (the code comment already said "a 50 Hz machine would want 5"). That also explains the Pi's zero without comparing drivers: at 60 Hz nominal is 3.33, two thirds of a tick clear. The ~16.7 s period is a beat between two clocks — the retrace period derives from the log sums as 4.0012 ticks (49.985 Hz), whose phase walks through one 200 Hz tick every 16.8 s, with no free parameter. Compose time is flat across the run (17.65–19.35 µs, bursts marginally *cheaper*), so nothing in `ball.c` is periodically expensive. **One real stutter was in there**: the steady 1 per interval is the demo writing its own log line to disk, absent from the first interval of the second run; the log is now buffered in RAM. None of this refutes the PiStorm developer on the 8-bit driver — 3.85 ms at 640×480×8 against 1.47 ms at 1920×1080 is ~24× the time per pixel — but that is a mean cost, not a burst. **The host is an Atari ST**, which cannot run this binary natively at all — it needs a 68020 and an FPU, and has neither, nor 256 colours; PiStorm supplies the CPU and the fast RAM, the ET4000 emulation the screen. So this proves the demo runs on an ST-with-PiStorm, a far more common machine at an Atari meeting than a TT. **It settles nothing about a real TT**: the sphere map took 25 ms here against the 4–8 s estimated for a 32 MHz 68030, because PiStorm executes the 68k far faster, and the ET4000 driver is not the NOVA. Texture was still `from generated` — `C:\BALL.NTX` had not been copied. Log kept as `sdk/apps/ball/BALL-pistorm-et4000-2026-09-07.LOG`. |
| 2026-09-08 | **Atari TT030 + NOVA** | 640×480×8, GEM desktop | ✅ **First run ever on the real target machine, and it inverts the assumption everyone held.** `BALL.PRG` (`b872afb`, split dirty rect + BALL.CFG) reports `pixel format MEASURED packed-8, fd_wdwidth = pixels/16, pen map MEASURED, VBL running` — the `v_get_pixel` probe grips on a **third** distinct driver, and the NOVA is chunky rather than planar. **The VME bus is not the bottleneck**: measured per frame, `compose 137.21 ms` against `blit 10.40 ms` — our own composition on the 68030 costs fourteen times the transfer everyone assumed was the constraint. The blit self-measures at `393.61 ns/px + 122.70 µs/call`, about 2.5 MB/s effective, consistent with the ~5 MB/s the user quoted. Result **5.91 fps**, 549 frames in 92.93 s, 2.09 rectangles per frame covering 7.79 % of the screen. **The sphere-map estimate was right**: 4–8 s was derived from the generated code, hardware measures **5.355 s** for 8184 px. Unmeasured until now and substantial: the room image takes **13.37 s** to paint, so start-up is nearly twenty seconds before the first bounce. Levers already identified in `ball.c` and untouched: `b_sqrt` runs Newton with four `fdivx` where the 68882 has `fsqrt` (~14× per root), and `float` forces constant conversions on m68k where `double` would be faster — both hit the sphere map hardest. For frame rate the immediate lever is `C:\BALL.CFG` with `ball 0.10`. Log kept as `sdk/apps/ball/BALL-tt-nova-2026-09-08.LOG`. |
| _pending_ | | | _serial console_ | Boot banner + self-tests over PL011 once the Waveshare FT232RNL USB-TTL (D) adapter is wired (ordered 2026-06-16). Heartbeat will read `RB2` (EL2 entry digit). |
