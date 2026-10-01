# V12 — STATUS: DONE (Hosttests rot→grün, QEMU-Gegenmessung 0/182) / Pi-Abnahme offen

Der in V11 belegte Produzent — das Kernel-Gerätepixelmodell — ist lokal
korrigiert: `OVERLAY_BIT` (Bit 5 des 16-Bit-Bildschirmworts) lebt jetzt im
X-Byte des 32-bpp-Framebuffer-Worts; jedes 16-Bit-Wort läuft exakt durch den
Speicher. Keine ODER-Maske auf `v_get_pixel`, keine globale
Farbengine-Neufassung. Bearbeitet 2026-09-21, Branch `desk-rebuild`, Basis
HEAD `194b057`, **nicht committet**, nichts geflasht.

## Was gebaut ist

* **`kernel/src/vdi_backend.rs`** — `OVERLAY_BIT` (`vdi_fill.c:26`),
  `OVERLAY_X = 0xFF00_0000`; `word_to_device(u16) -> u32`: 5→8 Bit je Kanone
  (`(c5<<3)|(c5>>2)`, wie bisher) plus Bit 5 in alle acht X-Bits
  (`0xFF` gesetzt / `0x00` frei); `device_to_word(u32) -> u16`: obere 5 Bit je
  Kanone plus Bit 31 → Bit 5. `rgb565_to_device`/`device_to_rgb565`/
  `pel_to_rgb`/`read_pixel` benutzen das Paar (`pal555_to_xrgb` entfällt).
  `xor_pixel` komplementiert **alle 32 Bit** (vorher die unteren 24) — EmuTOS
  komplementiert das ganze Wort inklusive Bit 5 (`swblit_rect_common16`
  `vdi_line.c:494,501`, `draw_line16` `:1670,1730`, Glyph-Blitter
  `vdi_textblit.c:806,1120`); der Weiß/Schwarz-Rundlauf (A85) bleibt:
  `0xFFDF ↔ 0x0020`, `0xFFFF ↔ 0x0000`. `raster_op`/`copy_rect` unverändert —
  weil das 32-Bit-Wort eine reine bitweise Expansion des 16-Bit-Worts ist,
  sind alle 16 Operationen darauf dieselben Operationen auf dem Falcon-Wort
  (`ALL_BLACK` → `0xFF_FFFFFF` = `0xFFFF`). `clear_screen_word` liefert so
  `0xFF_FFFFFF`, eine Stift-0-Füllung `0x00_FFFFFF`.
* **`kernel/src/hal/framebuffer.rs`** — Modul-Doc: der X-Byte-Vertrag
  (`0xXX_RR_GG_BB`; X vom Display nicht gezeigt: `ramfb` DRM `XRGB8888`,
  VideoCore Tiefe 32 ohne Alpha-Mode-Tag; Eigentümer `vdi_backend`);
  `rgb()` packt X = 0 (einziger Aufrufer war `vdi_backend`).
  `drivers/bcm2711/framebuffer.rs`: Kommentar.
* **`tos-api/src/vdi/{backend,inquiry,contour,raster}.rs`** — nur
  Doc-Kommentare (VDI-OVL-001 stand dort als offen).
* **Spec** — `docs/api/vdi/workstation/03-v_clrwk.md` (Zeilen 76–78/97–100
  alt: „Füllbyte irrelevant", „PEL 0 via fill_rect" — falsch, berichtigt mit
  Zitaten und der Gerätepixelregel, datiert V11/V12 2026-09-21);
  `docs/api/vdi/raster/_raster.md` § „Pi/BCM2711 binding": Feldtabelle des
  Gerätepixels (wo Bit 5 im 32-bpp-Wort liegt), XOR-Absatz, Memory-MFDB-Wort.
* **Tests** — `tests/vdi_reference/overlay_bit.rs`: Transkription auf die
  neuen Kernelfunktionen nachgezogen (Zeilenangaben, „keep in sync"-Hinweis);
  `tests/vdi_reference/harness.rs` `MemBackend`: vom Sechs-Grünbit-Modell auf
  das Kernelmodell (`screen_word`, beide Konvertierungen);
  `cases_v_series.rs` Kopf.

Gewählt statt „X = 0x01" (unter NOT/XOR nicht kanonisch) und statt sechs
Grünbits (würde jede gezeichnete Farbe ändern; die 5-Bit-Quantisierung ist
EmuTOS-treu, Kontrolltest).

## Prüfung

* Hosttests: V11-Binär **vorher 1 grün / 3 rot → nachher 4 grün**;
  `cargo run -p xtask -- test` **841 / 0 / 1**, rc 0 (837 + 3 + V03-Unit-Test).
  Schwarz (`0x0000`, Fälle 270/272 `cases_v_series.rs`), Weiß (`0xFFDF`,
  Kontrolltest), Bit-5-Kanten (`0xFFFF`/`0xFFDF`, `⊕ = 0x0020`) und alle
  bisherigen Farbfälle (`vdi_reference`, `vdi_raster_reference`, AES-Suiten)
  grün.
* Build: `cargo xtask build --board rpi4` (dev und `--release`): **tos-api 64 /
  kernel 94**, unverändert; Clippy `-p kernel --features board-rpi4` kernel
  125 / tos-api 80, keine Meldung in einer geänderten Datei; `-p tos-api
  --tests`: Treffer in `harness.rs` nur `:814/:875` (`cmp_field`, vorbestehend,
  außerhalb der Hunks). Kein qemu-virt-Build (Lead).
* QEMU raspi4b, Release-Kernel, `#Z`-Dumper (`measurements/measurement-2026-09-21-v11-v12/`):
  `tosdiff` gegen `EMUTOS14-VDI-all-falcontc.BIN` **0 von 182** (vorher 3);
  gegen den NewTOS-Dump vom Vormittag genau 54/160/161 `intout[1]`
  `0xFFDF → 0xFFFF`, sonst wortgleich. Raster gegen TT-Referenz **1 von 59**
  (Fall 157, D-VDI-25/R01, unverändert), `RASTER.BIN` byteidentisch zum
  vorigen NewTOS-Dump. Desktop-Idle und TeraDesk-Idle: Vorher-Kernel
  (HEAD-Produktdateien per `git show`, `kernel8.img` byteidentisch zum Kernel
  der Vormittagsmessung) gegen Nachher-Kernel **PNG byteidentisch, 0 px**;
  0 EXC/PANIC.

## Offen — Pi-Abnahme (die Abnahme verlangt eine neue Pi-Messung)

Nicht geflasht (Lead/User). Auf der Pi zu belegen: `DUMPVDI` Fälle 54/160/161
= `0xFFFF` **und** ein unverändertes Bild. Restrisiko benannt: das Verhalten
des VideoCore zum obersten Byte bei Tiefe 32 steht in keiner Track-B-Quelle
(`docs/hardware/mailbox-framebuffer.md`, `references/circle`); bisher stand
überall `0xFF`, jetzt tragen gezeichnete Pixel `0x00` und gelöschte/
komplementierte `0xFF`. Sollte die Pi Alpha auswerten, ist das dokumentierte
Mittel ein zusätzliches Tag im Allokations-Telegramm: set alpha mode
`0x0004_8007` = 2 („ignored") — nicht gesendet, weil nicht belegt.

**Nachtrag 2026-09-21 (später am Tag): das Alpha-Tag ist gelandet.** Belegt
über die Firmware-Wiki *Mailbox property interface* (Quelle 4 in
`docs/hardware/mailbox-framebuffer.md`, neuer § 3c; Tag-IDs zusätzlich in
`references/circle/addon/linux/raspberrypi-firmware.h:81,102`).
`drivers/bcm2711/framebuffer.rs::build_request` sendet set alpha mode
`0x0004_8007` = 2 vor `allocate`, prüft das Echo und fragt get alpha mode
`0x0004_0007` zurück → Bootzeile `[fb] alpha mode = N`. QEMU raspi4b:
`[fb] alpha mode = 2`, Allokation unverändert, Desktop-Idle-Screendump
byteidentisch zu `screendumps/desk-after-idle.png` dieser Messung. Das
Restrisiko oben ist damit auf der Pi nicht mehr eine Frage des
undokumentierten Defaults, sondern der dokumentierte Modus; Pi-Prüfung =
HW-Checkliste Punkt 30 (`docs/api/desktop/_hw-test-2026-09-17.md`).
