# VDI V-Serie (V01–V15): Einordnung jeder Zelle, gemessen statt gefiltert (2026-09-21)

Auditor-Auswertung der Tickets `TASK/V01.md` … `TASK/V15.md`. Grundlage sind
**drei frische Dumps derselben Tabelle** (`tosdump/cases/vdi-all.tsv`, 182 Fälle,
CRC `D135ADAF`), alle am 2026-09-21 auf `desk-rebuild` @ `2567942` gefahren
und in `measurement-2026-09-21-vdi-qemu/` abgelegt (`RUNINFO.TXT`,
`manifest.sha256`):

| Kürzel | Maschine | Profil | Herkunft |
|---|---|---|---|
| **TT** | EmuTOS 1.4 (Release-ROM `etos512de.img`) auf Hatari TT | 640×480, 4 Planes, 16 Stifte | `emutos-tt-hatari/VDI.BIN` — **bitgleich** mit der echten TT-Referenz `tosdump/reference/EMUTOS14-VDI-all.BIN` (cmp), also der validierte Ersatz |
| **FALCON-TC** | EmuTOS 1.4 (dasselbe Release-ROM) auf Hatari Falcon030, VIDEL Truecolor (`$FF8266 = 0100`) | 640×480, 16 Planes, 256 Stifte | `emutos-falcon-tc-hatari/release-1.4-rom/VDI.BIN`; Referenzkopie `tosdump/reference/EMUTOS14-VDI-all-falcontc.BIN`. Der gepinnte Quellstand (`etos512us.img`) liefert **0 von 182** Fällen anders |
| **NEWTOS** | NewTOS auf QEMU `raspi4b`, headless, `#Z`-Autorun | 640×480, 16 Planes, 256 Stifte | `newtos-qemu-raspi4b/VDI.BIN`, Kernel `--release` @ `2567942` |

**Kurzantwort.** Die 13 Fälle, in denen NEWTOS von der TT-Referenz abweicht,
sind **exakt** die 13 Fälle — Zelle für Zelle —, in denen **EmuTOS selbst** zwischen
TT-Medium und Falcon-Truecolor abweicht (`DIFF-emutos-tt-vs-emutos-falcontc.txt`
gegen `DIFF-emutos-tt-vs-newtos-qemu.txt`). Gegen das Falcon-Truecolor-Orakel
bleiben **3 von 182** Fällen: `v_get_pixel` nach `v_clrwk` liefert `0xFFDF`
statt `0xFFFF` (Fälle 54, 160, 161). Das ist der bekannte, bisher nur
diagnostizierte Defekt VDI-OVL-001 (V11/V12), jetzt mit Referenzwert
**gemessen** und mit drei roten Hosttests belegt. Alles andere ist
Geräteparameter nach EmuTOS' eigener Arithmetik — nicht nach Formelvermutung,
sondern durch dasselbe ROM auf zwei Geräteprofilen gezeigt. Versionsdelta
1.4-Release ↔ gepinnte Quelle: **keines** (0 von 182; `git log VERSION_1_4..HEAD`
für die betroffenen VDI-Dateien enthält nur `49ec9233`/`401c3561`, beide
VRAM-Allokation in `bios/screen.c`, keine VDI-Logik).

Es gibt **keine nicht-ticketierte** abweichende Zelle: jede Zeile der beiden
TT-Diffs ist unten einem Ticket zugeordnet.

---

## 1. Die Tabelle — eine Zeile je Ticket und Zelle

Werte aus `measurement-2026-09-21-vdi-qemu/cells.txt`. „QEMU“ = NEWTOS bei
640×480; die Pi-Werte (1920×1080) stehen zum Vergleich in den Tickets.
Testnamen: `tos-api/tests/vdi_reference/cases_v_series.rs` (grün, Binär
`vdi_reference`) und `tos-api/tests/vdi_reference/overlay_bit.rs` (rot, Binär
`vdi_reference_overlay_bit`).

| Ticket | Fall / Zelle | TT-Ref | QEMU | Falcon-TC-Ref | Klasse | EmuTOS-Herleitung | NewTOS-Stelle / Spec | Test |
|---|---|---|---|---|---|---|---|---|
| V01 | 33 `intout[0]`,`[1]` | 639, 479 | 639, 479 | 639, 479 | **Geräteparameter (Geometrie)** — bei 640×480 gleich; Pi 1919/1079 ist dieselbe Formel | `DEV_TAB[0] = V_REZ_HZ - 1`, `[1] = V_REZ_VT - 1` (`vdi/vdi_control.c:203-204`) | `workstation.rs:284-285` (`vq_extnd`), `:195-196` (`init_wk`); Spec `100-v_opnvwk.md:159-160` | `v01_v02_v03_case_0033_…` |
| V02 | 33 `intout[13]` | 16 | 256 | **256** | **Geräteparameter (Planes)** | `DEV_TAB[13] = (v_planes<8) ? 1<<v_planes : 256` (`vdi_control.c:206`); 16 Planes → 256 | `workstation.rs:289` (`ncol` = `backend.num_colors()`, Kernel `vdi_backend.rs:33` = 256); Spec `100-v_opnvwk.md:172` | dito (Byte-Vergleich gegen Falcon-TC; zusätzlich Formel-Ersatz `harness.rs::substitute` für Opcode 1/100) |
| V03 | 33 `intout[39]` | 4096 | 0 | **0** | **Geräteparameter (VIDEL-Modus)** — siehe Anmerkung A | `DEV_TAB[39] = get_palette()` (`vdi_control.c:208`); VIDEL, weder `VIDEL_COMPAT` noch `VIDEL_4BPP` → `return 0` (`bios/screen.c:831-849`) | `workstation.rs:30` (`PALETTE_PRECISION = 0`), `:291`; Spec `100-v_opnvwk.md:180,192-205,404` | dito |
| V04 | 34/250/252 `intout[4]` | 4 | 16 | **16** | **Geräteparameter (Planes)** | `INQ_TAB[4] = v_planes` (`vdi_control.c:210`) | `workstation.rs:301` (`backend.planes()`, Kernel `vdi_backend.rs:40` = 16); AES `ap_planes` liest dieselbe Zelle; Spec `102-vq_extnd.md:211` | `v04_v05_v06_case_0034_…`, `_0250_…`, `_0252_…` |
| V05 | 34/250/252 `intout[5]` | 1 | 0 | **0** | **Geräteparameter (Planes)** | `INQ_TAB[5] = (v_planes==16 \|\| MON_MONO) ? 0 : 1` (`vdi_control.c:211-213`) | `workstation.rs:306`; Spec `102-vq_extnd.md:212` | dito |
| V06 | 34/252 `ptsout[0..3]` = 0,0,639,479; 250 `ptsout[0..3]` = 10,20,300,400; `intout[19]` = 0/1/0 | gleich | gleich | gleich | **Geräteparameter (Geometrie)** — bei 640×480 in allen drei Dumps identisch; ein ignorierter `vs_clip` fiele in Fall 250 auf (Test vergleicht alle 12 `ptsout`-Wörter und `intout[19]`) | `vs_clip`: `min(xres, x2)`, aus: `xmx_clip = xres` (`vdi_control.c:235-250`, `xres = DEV_TAB[0]`, `vdi_defs.h:94`); Ausgabe `vdi_control.c:641-651` | `attributes.rs:49-75`, `workstation.rs:311-314`; Spec `102-vq_extnd.md:24,67` | dito |
| V07 | 42 `vsl_color 99` `intout[0]` | 1 | 99 | **99** | **Geräteparameter (numcolors)** | `validate_color_index`: `<0 \|\| >= numcolors → 1` (`vdi_control.c:224-230`), `numcolors = DEV_TAB[13]` (`vdi_defs.h:98`); `vdi_line.c:143-149` | `attributes.rs:421-426`, `vwk.rs:166-172`; Spec `_color-pixel-input.md:44` (für `vq_color`; Setter-Regel in `attributes/_line-marker.md`) | `v07_case_0042_…` + Grenzen `v07_v08_v09_pen_boundaries_follow_numcolors` (−1→1, 0→0, 255→255, 256→1, 999→1) |
| V08 | 113 `vsm_color 99` `intout[0]` | 1 | 99 | **99** | **Geräteparameter (numcolors)** | wie V07; `vdi_marker.c:72-78` | `attributes.rs:139-144` | `v08_case_0113_…` + Grenzen wie V07 |
| V09 | 72 `vq_color 255,0` `intout[0]` | −1 | 255 | **255** | **Geräteparameter (numcolors)** | `colnum >= numcolors → INTOUT[0] = -1` (`vdi_col.c:845-859`) | `inquiry.rs:93-104`; Spec `_color-pixel-input.md:44,57,147` | `v09_case_0072_…` (alle `intout[0..3]`) + Grenzen (−1→−1, 255→255, 256→−1, 999→−1); Fall 73 (999) ist auf allen drei −1 |
| V10 | 270/272 `intout[0]`,`[1]` | 15, 1 | 0, 0 | **0, 0** | **Geräteparameter (TRUECOLOR_MODE)** — nicht Palettenindex+Pen, sondern 0 + Rohwort; Stift 1 = `palette[MAP_COL[1]=255]` = Schwarz `0x0000` | `vdi_fill.c:1095-1115` (`TRUECOLOR_MODE`-Zweig `:1105-1110`), `vdi_defs.h:22`, `vdi_col.c:737` | `inquiry.rs:183-200`; Spec `_color-pixel-input.md:74-85` (Tabelle korrekt) | `v10_case_0270_…`, `v10_case_0272_…` |
| **V11** | 54/160/161 `intout[1]` | 0 | **0xFFDF** | **0xFFFF** | **DEFEKT, CONFIRMED** (VDI-OVL-001) — siehe § 2 | `v_clrwk`: `memset(v_bas_ad, 0xff, size)` (`vdi_control.c:599-618`, Füllbyte `:611`); `pixelread` gibt das Wort ungemaskt zurück (`vdi_fill.c:671-681`); `OVERLAY_BIT 0x0020` (`vdi_fill.c:26`); Stift-0-Weiß ist `(31<<11)\|(31<<6)\|31 = 0xFFDF` (`vdi_col.c:601-613`) | `workstation.rs:392-396` ruft richtig `clear_screen_word(0xFFFF)`; verloren geht das Bit im Kernel-Gerätepixel `kernel/src/vdi_backend.rs:397-424` (XRGB8888, `hal/framebuffer.rs:16-18`); Spec `03-v_clrwk.md:76-78,97-100` **falsch** („Füllbyte irrelevant“, „PEL 0 via fill_rect“) | **rot:** `the_device_pixel_round_trips_the_cleared_word_0xffff`, `v_clrwk_then_v_get_pixel_reports_the_falcon_word_0xffff`, `a_cleared_pixel_and_a_pen_0_pixel_differ_by_overlay_bit_alone`; grün (Kontrolle): `pen_0_fill_then_v_get_pixel_reports_the_pseudo_palette_white_0xffdf` |
| V12 | (Reparatur zu V11) | — | — | — | **bereit für den Entwickler** | s. V11 | s. V11; Optionen in `docs/STATUS.md` (2026-09-06) | rot → grün ist die Abnahme; Kontrolltest muss grün bleiben |
| V13 | 173 `vq_chcells` `intout[0]`,`[1]` | 30, 80 | 30, 80 | 30, 80 | **Geräteparameter (Geometrie/Font)** — Pi 67/240 ist dieselbe Ganzzahldivision | `INTOUT[0] = v_cel_my+1`, `[1] = v_cel_mx+1` (`vdi_esc.c:62-67`); `v_cel_mx = V_REZ_HZ/max_cell_width - 1`, `v_cel_my = V_REZ_VT/form_height - 1` (`bios/font.c:80-81`) | `escape.rs:39-44` (`width()/8`, `height()/16`); Spec `_escape.md:124-135` | `v13_case_0173_…` |
| V14 | 274 `v_opnvwk` `intout[0]`,`[1]`,`[13]`,`[39]` | 639,479,16,4096 | 639,479,256,0 | **639,479,256,0** | **Geräteparameter** — dieselben vier Regeln wie V01–V03; die zweite Workstation trägt dieselbe `DEV_TAB` | `init_wk` kopiert `DEV_TAB`/`SIZ_TAB` (`vdi_control.c:344-352`) | `workstation.rs:141-202` (`init_wk`, von `v_opnvwk` gerufen); Spec `100-v_opnvwk.md` | `v14_v15_case_0274_…` (alle 45 `DEV_TAB`- und 12 `SIZ_TAB`-Wörter gegen Falcon-TC; danach `v_clsvwk`) |
| V15 | alle 182 `contrl_before[6]`/`contrl[6]` (+59 Raster) | (2,2)×181, (2,3)×1 | (2,2)×181, (2,3)×1 | (2,2)×181, (2,3)×1 | **Messauswertung: identisch** — Gleichheit innerhalb des Aufrufs in 181 Records; nur `v_opnvwk` (274) schreibt ein neues Handle (3), auf allen drei Maschinen; Raster 59×(2,2) TT wie NewTOS (`handles-vdi.txt`, `handles-raster.txt`) | `vdi_v_opnvwk` schreibt `CONTRL[6] = handle` (`vdi_control.c:456`) | `workstation.rs:254-262`; Dumper schreibt `contrl[6]` selbst (`tosdump/core/runner.c`) | `v14_v15_case_0274_…` prüft Identität statt Zahl: neues Handle ≠ 1, ≠ 2; lebt bis `v_clsvwk`, danach No-op |

**Anmerkung A (V03, keine Beanstandung im Sinne eines Defekts, aber das
Ticket-Abnahmekriterium ist wörtlich nicht erfüllt).** `TASK/V03.md` verlangt
„weder 4096 noch 0 global fest verdrahten“. NewTOS verdrahtet `DEV_TAB[39]` als
`const PALETTE_PRECISION: i16 = 0` (`workstation.rs:30`), während `DEV_TAB[13]`
aus `backend.num_colors()` abgeleitet wird. Der Wert ist für das einzige
existierende Backend (16 Planes, `vdi_backend.rs:40`) richtig und durch die
Falcon-TC-Messung belegt; durch die API ist heute kein anderer Wert
beobachtbar. Der Formel-Ersatz im Harness (`harness.rs::substitute`, „`PLANES >
8 → 0`, sonst 4096“) prüft eine Ableitung, die das Produkt nicht enthält.
Sollte ein Backend mit ≤ 4 Planes (VIDEL 4bpp/ST-kompatibel) hinzukommen,
bliebe `[39]` stillschweigend 0. Einordnung: **PLAUSIBLE, latent, nicht
beobachtbar** — Empfehlung: `[39]` wie `[13]` aus `planes()` ableiten
(`planes() <= 4 → 4096`, sonst 0, nach `bios/screen.c:840-846`), oder das
Ticket mit der Begründung „ein Profil, Wert gemessen“ schließen.

---

## 2. Der Defekt: V11/V12 — `0xFFDF` statt `0xFFFF` nach `v_clrwk` (CONFIRMED)

**Was EmuTOS tut (Falcon030/TOS4, `CONF_WITH_VDI_16BIT`, `TRUECOLOR_MODE`):**

```
vdi/vdi_control.c:599-618   vdi_v_clrwk: fill = 0xff (TRUECOLOR_MODE); memset(v_bas_ad, fill, size)
vdi/vdi_fill.c:671-681      pixelread:   TRUECOLOR_MODE -> return *addr  (ungemaskt)
vdi/vdi_fill.c:1095-1110    vdi_v_get_pixel: INTOUT[0] = 0; INTOUT[1] = pel
vdi/vdi_col.c:601-613       set_color16: palette[palnum] = (r<<11)|(g<<6)|b   (Bit 5 nie gesetzt)
vdi/vdi_fill.c:26           #define OVERLAY_BIT 0x0020
```

Der gelöschte Schirm ist `0xFFFF`; ein in Stift 0 gezeichneter Punkt ist
`0xFFDF`; die beiden unterscheiden sich genau um `OVERLAY_BIT`. EmuTOS hält
sie auseinander — der Truecolor-Seed-Fill von `v_contourfill` verlässt sich
darauf und maskiert das Bit auf beiden Seiten vor dem Vergleich
(`vdi_fill.c:702-709,726-733,755`).

**Gemessen** (nicht gefolgert): Falcon-TC-Referenz, Fälle 54/160/161
(`v_get_pixel` an (5,5), (0,0), (639,479) nach dem `v_clrwk` des Resets) →
`INTOUT[1] = 0xFFFF`, in beiden ROMs. NewTOS QEMU → `0xFFDF`
(`DIFF-emutos-falcontc-rel-vs-newtos-qemu.txt`). Die drei Zellen sind der
gesamte Rest des Diffs gegen das Orakel.

**Wo es verloren geht.** `tos-api` ist richtig: `v_clrwk` ruft
`backend.clear_screen_word(0xFFFF)` (`workstation.rs:392-396`), `v_get_pixel`
liest `device_to_rgb565(get_device_pixel())` (`inquiry.rs:183-194`). Der Kernel
speichert das Gerätepixel als XRGB8888 mit festem X-Byte `0xFF`
(`kernel/src/hal/framebuffer.rs:16-18`), expandiert drei 5-Bit-Kanonen
(`pal555_to_xrgb`, `vdi_backend.rs:176-182`, gerufen aus `rgb565_to_device`
`:419-424`) und nimmt beim Lesen die obersten 5 Bits jeder Kanone
(`device_to_rgb565`, `:397-417`). Bit 5 des 16-Bit-Worts hat in diesem Pixel
keinen Platz; `0xFFFF` und `0xFFDF` landen beide auf `0xFFFFFFFF`.

**Beobachtbare Folgen** (aus `docs/STATUS.md` 2026-09-06, hier nur die erste
gemessen): (1) `v_get_pixel` nach `v_clrwk`; (2) `vro_cpyfm` Screen→Memory eines
gelöschten Bereichs schreibt `0xFFDF`-Wörter in die App-MFDB; (3) `v_contourfill`
im Seed-Modus auf dem gelöschten Hintergrund füllt, wo EmuTOS nichts zeichnet.

**Die roten Tests** (`tos-api/tests/vdi_reference/overlay_bit.rs`, Binär
`vdi_reference_overlay_bit`; eigenes Binär, weil `install_host` einen Host je
Prozess erlaubt und das Backend-Double des Harness sechs Grünbits hält und den
Defekt deshalb nicht sehen kann; das Double dort ist das Kernel-Pixelmodell,
Ausdruck für Ausdruck transkribiert, mit Zeilenangaben):

```
test overlay_bit::the_device_pixel_round_trips_the_cleared_word_0xffff ... FAILED
  assertion `left == right` failed: the cleared-screen word 0xFFFF (OVERLAY_BIT set, vdi_fill.c:26;
  memset 0xff in v_clrwk, vdi_control.c:610-618) must survive the 16->32->16 round trip; ...
  left: 65503   right: 65535
test overlay_bit::v_clrwk_then_v_get_pixel_reports_the_falcon_word_0xffff ... FAILED
  assertion `left == right` failed: case 54: v_get_pixel(5,5) after v_clrwk -- EmuTOS Falcon
  truecolor reference (EMUTOS14-VDI-all-falcontc.BIN) = 0xFFFF, NewTOS = 0xFFDF
  left: 65503   right: 65535
test overlay_bit::a_cleared_pixel_and_a_pen_0_pixel_differ_by_overlay_bit_alone ... FAILED
  assertion `left == right` failed: cleared 0xFFDF and pen-0 0xFFDF must differ by OVERLAY_BIT (0x0020) alone
  left: 0   right: 32
test overlay_bit::pen_0_fill_then_v_get_pixel_reports_the_pseudo_palette_white_0xffdf ... ok
```

Der grüne Kontrolltest ist Teil der Abnahme: eine Reparatur, die Stift-0-Weiß
auf `0xFFFF` hebt (sechs Grünbits), ist die falsche — die 5-Bit-Quantisierung
ist EmuTOS-treu (`set_color16`/`vq_color16`, `vdi_col.c:601-614,813-838`).

**Für V12 (Entwickler):** Reparaturort ist das Kernel-Gerätepixelmodell
(`kernel/src/vdi_backend.rs`, Konvertierungspaar + Speicherformat), nicht
`tos-api`. Nach der Reparatur das transkribierte Double in `overlay_bit.rs`
auf die neuen Kernel-Ausdrücke nachziehen; die drei Tests werden nur grün,
wenn das Modell `0xFFFF` rund trägt **und** der Kontrolltest grün bleibt.
Danach Gegenmessung: derselbe QEMU-Lauf muss gegen
`EMUTOS14-VDI-all-falcontc.BIN` **0 von 182** ergeben.

**Spec-Befund (an den Researcher):** `docs/api/vdi/workstation/03-v_clrwk.md`
Zeilen 76–78 („Die `0x00`/`0xff`-Unterscheidung ist ein Referenzdetail …
NewTOS schreibt Hintergrundfarbe Index 0 … unabhängig davon“) und 97–100 („Die
EmuTOS-Füllbytewahl ist hier irrelevant“) sind falsch: das Füllwort ist über
`v_get_pixel`, `vro_cpyfm` und `v_contourfill` beobachtbar, und der Code
(`workstation.rs:394`) tut inzwischen das Richtige (`0xFFFF`), die Spec
beschreibt den alten Stand (`fill_rect` mit PEL 0, „`workstation.rs:193-196`“).

---

## 3. Bereit für den Entwickler (Defekte mit roten Tests)

| Ticket | Test (rot heute) | Reparaturort |
|---|---|---|
| V11/V12 | `cargo test -p tos-api --test vdi_reference_overlay_bit` — 3 rot, 1 grün | `kernel/src/vdi_backend.rs` (Gerätepixel: Platz für `OVERLAY_BIT`), danach Double in `tos-api/tests/vdi_reference/overlay_bit.rs` nachziehen; Spec `03-v_clrwk.md:76-78,97-100` berichtigen |

Hinweis zur Suite: `cargo run -p xtask -- test` läuft fail-fast; das rote
Binär heißt `vdi_reference_overlay_bit`, damit es **nach** `vdi_raster_reference`
und `vdi_reference` läuft. Alle `tos-api`-Binäre laufen durch (820 grün), erst
danach bricht die Suite ab — `tos-types`/`xtask` (17 Tests, 1 ignoriert) werden
bis zur Reparatur unter fail-fast übersprungen. `cargo test --no-fail-fast`
über die drei Pakete: **837 grün / 3 rot / 1 ignoriert**.

## 4. Als Geräteparameter schließen (Satz für die Quittung)

Für jedes der folgenden Tickets ist derselbe Satz belegt:

> EmuTOS 1.4 (identisches ROM) liefert für diese Zelle auf einem TT (4 Planes,
> 16 Stifte) den Referenzwert und auf einem Falcon030 in VIDEL-Truecolor
> (16 Planes, 256 Stifte) exakt den NewTOS-Wert
> (`measurement-2026-09-21-vdi-qemu/DIFF-emutos-tt-vs-emutos-falcontc.txt`,
> `cells.txt`); die Zelle ist damit nach EmuTOS' eigener Herleitung
> (\<Zeile aus § 1\>) ein Geräteparameter, und der Hosttest
> \<Name aus § 1\> vergleicht sie gegen die Falcon-Truecolor-Referenz
> `tosdump/reference/EMUTOS14-VDI-all-falcontc.BIN`.

| Ticket | Zelle | EmuTOS-Zeile für den Satz | Test |
|---|---|---|---|
| V01 | `DEV_TAB[0..1]` | `vdi_control.c:203-204` | `v01_v02_v03_case_0033_…` |
| V02 | `DEV_TAB[13]` | `vdi_control.c:206` | dito |
| V03 | `DEV_TAB[39]` | `vdi_control.c:208`, `bios/screen.c:831-849` | dito (+ Anmerkung A) |
| V04 | `INQ_TAB[4]` | `vdi_control.c:210` | `v04_v05_v06_case_00{34,250,252}_…` |
| V05 | `INQ_TAB[5]` | `vdi_control.c:211-213` | dito |
| V06 | Clip-`PTSOUT[0..3]`, `INQ_TAB[19]` | `vdi_control.c:235-250,641-651` | dito |
| V07 | `vsl_color` Bereich | `vdi_control.c:224-230`, `vdi_line.c:143-149` | `v07_case_0042_…`, `v07_v08_v09_pen_boundaries_follow_numcolors` |
| V08 | `vsm_color` Bereich | `vdi_control.c:224-230`, `vdi_marker.c:72-78` | `v08_case_0113_…`, Grenzen dito |
| V09 | `vq_color` Bereich | `vdi_col.c:845-859` | `v09_case_0072_…`, Grenzen dito |
| V10 | `v_get_pixel` Format | `vdi_fill.c:1095-1110`, `vdi_defs.h:22` | `v10_case_0270_…`, `v10_case_0272_…` |
| V13 | `vq_chcells` | `vdi_esc.c:62-67`, `bios/font.c:80-81` | `v13_case_0173_…` |
| V14 | `v_opnvwk`-`DEV_TAB` | `vdi_control.c:344-352` + V01–V03 | `v14_v15_case_0274_…` |
| V15 | Handle-Identität | `vdi_control.c:456` | `v14_v15_case_0274_…` (+ `handles-vdi.txt`, `handles-raster.txt`) |

---

## 5. Was gefahren wurde, was nicht

Gefahren: drei vollständige Dumps derselben Tabelle (TT/Hatari validiert,
Falcon-TC in zwei ROMs, NewTOS/QEMU), ein Raster-Dump NewTOS/QEMU für die
59 Handle-Paare, fünf `tosdiff`-Läufe, Handle- und Zellauswertung per Skript,
15 neue grüne und 3 rote Hosttests, die volle Suite mit und ohne fail-fast.

Nicht gefahren / offen:

* **Echte Pi 4 (1920×1080)**: keine Hardware-Aktion; die Geometriezellen
  (V01, V06, V13, V14 `[0..1]`) sind bei 640×480 gleich und bei 1920×1080 nach
  derselben Formel 1919/1079, 240/67 — die Formel ist getestet, die Zahl nicht
  gemessen (Ticket-Pi-Werte vom 2026-09-09 stimmen mit ihr überein).
* **Folgen (2) und (3) von VDI-OVL-001** (`vro_cpyfm` Screen→Memory,
  `v_contourfill` Seed-Modus) sind nicht neu gemessen; die Raster-Tabelle enthält
  kein Screen-MFDB (Section-C-Fälle laufen auf Speicherpuffern), und
  `vdi-all` hat keinen Seed-Fill auf dem gelöschten Grund mit negativem
  `INTIN[0]`. Ein Falcon-TC-Referenzwert dafür wäre mit derselben Methode in
  einer neuen Tabelle billig zu haben.
* **V14, „Workstation anschließend korrekt geschlossen“**: der Dumper schließt
  das in Fall 274 geöffnete Handle 3 nicht (letzter Fall der Tabelle); im
  Hosttest wird es geschlossen und die Totheit geprüft. Am Gerät nicht gemessen.
* **Raster-Diff**: Fall 157 weicht wie bekannt ab (D-VDI-25 / R01, zurückgestellt) —
  außerhalb dieses Auftrags, nicht verfolgt.

## 6. Außerhalb des Auftrags beobachtet, nicht verfolgt

* `*.BIN` in **allen** `measurement-*`-Verzeichnissen sind gitignoriert
  (Wurzel-`.gitignore` `*.bin` + `core.ignorecase=true`); der Kommentar dort
  behauptet das Gegenteil. Die Rohdumps früherer Messungen sind nicht im Repo.
* `tosdump/reference/EMUTOS14-VDI-all-falcontc.BIN/.LOG` ist neu und untracked;
  der Test `cases_v_series.rs` bindet sie per `include_bytes!` ein — beim
  Commit mitnehmen.
* Der V17-Befund (`aes-m07`) ist hier nicht berührt; `vq_mouse` steht in keiner
  VDI-Tabelle.
