# Real-hardware test plan — IKBD keyboard + mouse on the GEM desktop

> Manual RPi4 test plan for the Phase-14 desktop driven by a native Atari
> IKBD keyboard + mouse on UART2. Record results in
> [`docs/HW-TEST-LOG.md`](../HW-TEST-LOG.md). Companion: the wiring +
> open-items in [`ikbd-uart.md`](ikbd-uart.md).

## ⚠️ Before you wire anything — Rule 6 caveat

The UART2 base address and GPIO pin/ALT assignment in
`kernel/src/drivers/bcm2711/ikbd_uart.rs` follow the standard BCM2711
pattern but were **not re-read from the BCM2711 datasheet** (the datasheet
is not in `references/`). Treat the pins below as the *current best guess*,
not confirmed. If they are wrong, **Test 0b simply produces nothing** (no
damage, just no input) — that is the signal that the pin/ALT choice needs a
datasheet cross-check.

**Electrical safety:** the IKBD keyboard signals at **5 V**; the Pi GPIO is
**3.3 V only**. The keyboard's TX line **must** go through a divider
(1 kΩ series + 2 kΩ to GND) into the Pi — never 5 V straight onto a GPIO.
Power the keyboard from the header's +5 V (pin 2/4) and GND.

## Wiring (current assumption — verify against the datasheet first)

| Signal | Pi side | Notes |
|---|---|---|
| Keyboard TX → Pi **RXD2** | GPIO 1 (header pin 28), ALT4 | **via 5 V→3.3 V divider** |
| Pi **TXD2** → Keyboard RX | GPIO 0 (header pin 27), ALT4 | only needed for mouse-enable cmds; needs a real level shifter |
| +5 V | header pin 2 or 4 | keyboard power |
| GND | header pin 6/9/… | common ground |
| **Serial console (diagnostics)** | UART0 = GPIO14/15 (pins 8/10), 115200 8N1 | watch `[desk]` log lines |

Mouse rides the same IKBD serial stream — no extra wiring. After reset the
IKBD streams relative-mouse reports by default, so RXD2-only should already
deliver mouse motion.

## Diagnostics — watch the serial console

Every desktop action logs to the serial console (UART0, 115200). Keep a
terminal on it; the `[desk] …` lines tell you what the kernel actually
saw, which disambiguates "did my click register?" from "did the render
update?". Key lines: `desktop drawn (N drives)`, `click → open C:`,
`opened C:\ (N entries)`, `created C:\NEWFLDR`, `deleted C:\…`,
`renamed C:\ entry`, `Set background: desktop = 0x4X`,
`Save desktop: wrote NEWDESK.INF`, `NEWDESK.INF #Q loaded`,
`shutdown — halting`.

## Test table

Legend: **HDMI** = what you should see on screen, **Serial** = the log line
to confirm it. Tests marked **✍ writes SD** modify the card — use throwaway
test files or back the card up first.

| # | Test | Do this | Expected (HDMI / Serial) | Verifies | Result |
|---|---|---|---|---|---|
| 0a | Boot | Power on with the SD card | HDMI: green desktop, menu bar, C:/D:/Trash/Printer. Serial: `desktop drawn (N drives)` | boot + SD + render | |
| 0b | **Input alive** | Move the mouse | Cursor moves on screen | IKBD RX + UART2 wiring (the gate — if dead, pins/datasheet) | |
| 0c | Mouse direction | Move up / right | Cursor goes up / right (not inverted) | Y-direction (known open item — if inverted, it's fixable) | |
| 1 | Open a menu | Click the **File** title | Dropdown opens, title highlighted | menu_intercept | |
| 2 | Cancel a menu | Open a menu, click empty desktop | Dropdown closes, title un-highlights | menu cancel | |
| 3 | Menu action | **Desk → Desktop info…** | Alert "NewTOS Desktop"; click **OK** closes it | MN_SELECTED + form_alert + button click | |
| 4 | Open a drive | Click the **C:** icon | Directory window opens with the file list. Serial: `click → open C:`, `opened C:\ (N entries)` | drive hit-test + window + FS read | |
| 5 | Move a window | Drag the window's **title bar** | Window follows, listing redraws. Serial: (WM_MOVED) | drag_move + wind_set | |
| 6 | Top a window | Open **D:** too, then click the **C:** window | C: comes to the front | WM_TOPPED / Z-order | |
| 7 | Close a window | Click the **closer** box (top-left of the frame) | Window closes, desktop repaints under it | WM_CLOSED + background hook | |
| 8 | Cycle windows | Open 2+ windows, **File → Cycle windows** | The back window comes to the front | cycle_windows | |
| 9 | Show as icons | **View → Show as icons** | Listing becomes an icon grid (folders + document icons) | view toggle + icon blit | |
| 10 | Show as text | **View → Show as text** | Back to name + size rows | view toggle | |
| 11 | Sort | **View → Sort by size** (then by name) | List reorders: directories first, then by the key | sort_entries | |
| 12 | Size to fit | **View → Size to fit** | Window resizes to fit the listing | size_to_fit | |
| 13 | Select a file | Click a file row (text view) | The row inverts (selected) | select_at + highlight | |
| 14 | New folder ✍ writes SD | **File → New folder** | `NEWFLDR` appears as `<DIR>`. Serial: `created C:\NEWFLDR` | Dcreate write | |
| 15 | Delete ✍ writes SD | Select a **test** file → **File → Delete** → **OK** | File disappears. Serial: `deleted C:\…` | confirm + Fdelete write | |
| 16 | Drag-to-trash ✍ writes SD | Select a **test** file, drag it onto **Trash**, release → **OK** | File deleted | drag + trash_at + delete | |
| 17 | **Rename (keyboard)** ✍ writes SD | Select a **test** file → **File → Info/Rename…** → **type a new name** → **OK** | File renamed. Serial: `renamed C:\ entry` | **IKBD keyboard** via form_do + Frename | |
| 18 | Set background | **Options → Set background** (a few times) | Desktop colour cycles (green→blue→red→…). Serial: `Set background: desktop = 0x4X` | g_patcol live change | |
| 19 | Save desktop ✍ writes SD | **Options → Save desktop** | Serial: `Save desktop: wrote NEWDESK.INF` | save_inf → NEWDESK.INF | |
| 20 | **Persistence** | After Test 19, **reboot the Pi** | Desktop comes up in the **saved** colour. Serial: `NEWDESK.INF #Q loaded` | newdesk.inf round-trip on real SD | |
| 21 | Shutdown | **File → Shutdown** | Desktop exits. Serial: `shutdown — halting` | loop exit | |

## If Test 0b fails (no cursor movement)

The keyboard/mouse stream is not reaching the decoder. Most likely the
UART2 base/GPIO/ALT assumption is wrong (needs the datasheet) or the wiring
/ divider is off. This is the expected failure mode of the un-verified
pins — capture it and we cross-check the BCM2711 datasheet, fix
`ikbd_uart.rs`, and re-flash. (No software change can verify the pins;
only the datasheet + this test can.)

## Known caveats to expect

- **Mouse Y may be inverted** (Test 0c): the IKBD defaults to Y-at-bottom;
  fixable by inverting `dy` or sending IKBD cmd `0x10`.
- **Single vs double click:** the desktop currently opens a drive on a
  *single* click (the simplified evnt_multi has no click-count yet).
- Keyboard does nothing on the bare desktop except inside the Rename dialog
  (no menu hotkeys wired yet).
