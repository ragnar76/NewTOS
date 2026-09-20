# A86 — STATUS: DONE

Abschnitt `_raster.md` § „Pi/BCM2711 binding" neu geschrieben und alle
gefundenen Kopien der veralteten 16-Farben-Aussage im vorgegebenen Suchraum
korrigiert. Bearbeitet 2026-09-21, Branch `desk-rebuild`, **nicht
committet**. Reine Dokumentation, keine Codeänderung.

## 1. Hauptziel — `docs/api/vdi/raster/_raster.md` § „Pi/BCM2711 binding"

Der Abschnitt (vormals Zeilen ~505-651) wurde komplett neu geschrieben,
gegen den aktuellen Quelltext gegengelesen:

| Aussage | Vorher (falsch) | Jetzt (Beleg) |
|---|---|---|
| Screen-MFDB `fd_nplanes` | **4** (16-Farben-Modell) | **16** — `kernel/src/vdi_backend.rs:35-40` (`PLANES`), EmuTOS `INQ_TAB[4]=v_planes` (`vdi_control.c:210`) |
| Farbzahl | `NUM_COLORS=16` | **256** — `vdi_backend.rs:29-33`, EmuTOS `DEV_TAB[13]=(v_planes<8)?(1<<v_planes):256` (`vdi_control.c:206`) — für `v_planes=16` ist das die Konstante 256, nicht widersprüchlich zu 16 Planes |
| Bildschirmauflösung `fd_w`/`fd_h` | 640×480 (tote Referenz `framebuffer.rs:23/24` — Konstanten existieren nicht mehr) | 1920×1080 (Default, verhandelt) — `kernel/src/drivers/mod.rs:31-33` (`BOOT_WIDTH`/`BOOT_HEIGHT`), geprüft für beide Boards |
| „Device pixel" | unklar (vermischte 4-Plane-Logik mit 32bpp-Store) | präzisiert: EmuTOS' Device-Pixel ist das **16-Bit-RGB565-Wort** (`RRRRRGGGGG0BBBBB`, `vdi_defs.h:153-158`), der physische Pi-Framebuffer ist ein **davon verschiedener, NewTOS-interner** 32-bpp-XRGB8888-Store; Brücke an genau zwei Stellen (`pal555_to_xrgb`/`device_to_rgb565`) |
| Palette | fehlte | neuer Abschnitt: 256 Pens, `vs_color`(14)/`vq_color`(26) pro-Workstation, Index 0/1 = Weiß/Schwarz als Konvention, worauf `init_3d` (`gemoblib.c:1327-1343`) sich verlässt (`LWHITE` statt `WHITE` bei `ws_ncolors>LWHITE`) |
| XOR-Semantik | verstreut | neuer Abschnitt: zwei getrennte Pfade — `WM_XOR` (Draw, 24-Bit-RGB-Komplement, `vdi_backend.rs:303-336`, EmuTOS `vdi_line.c:494`) vs. `vro_cpyfm` Op 6 (16-Op-Logiktabelle, Screen↔Screen im 32-Bit-, Memory-Pfade im 16-Bit-Domain — zwei verschiedene `raster_op`-Implementierungen im Code, `raster.rs:210-229` vs. `vdi_backend.rs:824-852`) |

**Nicht verifizierbar:** Die Aufgabenbeschreibung nannte „docs/MEMORY.md
ADR zur VDI-Truecolor-Workstation" als Quelle. Diese ADR existiert **nicht**
in `docs/MEMORY.md` (durchsucht, keine Treffer für RGB565/truecolor/
CONF_WITH_VDI_16BIT); der Code selbst verweist nur auf einen nie
aufgelösten Platzhalter `[ADR-0xx truecolor VDI]`
(`docs/api/vdi/workstation/102-vq_extnd.md:198`, unverändert). Im neuen
Abschnitt stattdessen auf den Code und die bereits aufgelöste
Diskrepanz D-VDI-7 (`docs/api/vdi/_discrepancies.md:26`, aufgelöst
2026-09-04) verwiesen — geflaggt, nicht erfunden.

## 2. Sweep — jede weitere Kopie der 16-Farben-Aussage

Suchbefehl exakt wie vorgegeben (`/usr/bin/grep -rn "16-colour\|16 colour\|
16-color\|fd_nplanes.*4\|NUM_COLORS=16\|4 planes\|four planes\|CLUT"
docs/api/vdi/ docs/api/aes/ docs/ARCHITECTURE.md docs/COMPATIBILITY.md`,
keine Treffer in den letzten beiden Dateien. Korrigiert (Blockquote mit
`**Correction (A86, 2026-09-21)**`, Originaltext sichtbar belassen, Muster
wie A72):

| Datei | Fund | Korrektur |
|---|---|---|
| `docs/api/vdi/workstation/102-vq_extnd.md` | „ADR-017"-Blockquote + „1080p later" behaupteten noch `INQ_TAB[4]/[5]` = 4 Planes / `num_colors()`, obwohl der Abschnitt direkt darüber („resolved 2026-09-04") das Gegenteil sagt — interner Widerspruch | beide Stellen auf `planes()=16`/`num_colors()=256` korrigiert |
| `docs/api/vdi/workstation/01-v_opnwk.md` | „ADR-017"-Blockquote + Bulletpoint zitierten „640×480×16" mit `[13]=16`,`[39]=16`; separat „single fixed 640×480 mode" | auf `[13]=256`,`[35]=1`,`[39]=0` (PALETTE_PRECISION) und verhandelte 1920×1080-Geometrie korrigiert |
| `docs/api/vdi/workstation/100-v_opnvwk.md` | größte Fundstelle: „ADR-017"-Blockquote, Tabellenüberschrift + Zeilen 13/39, „Open question" (Zeile 349 alt), Mismatch-Punkt 4 (`DEV_TAB[39]`-Semantik), „Palette / colour model"-Absatz, „1080p later" | alle korrigiert; Mismatch #4 als **aufgelöst** markiert — der aktuelle Code setzt `dev[39]=PALETTE_PRECISION=0` (`workstation.rs:30,202`), genau die EmuTOS-treue Antwort, die der Mismatch-Punkt seinerzeit forderte, nicht mehr `ncol` |
| `docs/api/vdi/_state.md` | „Open questions"-Absatz nannte 640×480×16-Farben als offen, obwohl D-VDI-7 bereits 2026-09-04 aufgelöst wurde | auf die aufgelösten Werte umgeschrieben |
| `docs/api/vdi/input/_mouse.md` | (a) Verifikationskriterium `mf_bg=99` „auf 16-Farben-Gerät" — bei 256 Farben ist 99 gültig, das Kriterium löst so **nicht** mehr aus; (b) „16 CLUT-Indizes" für die Cursor-Sprite-Farben | (a) Trigger auf `mf_bg=256` korrigiert; (b) auf 256 Pens / Pseudo-Palette korrigiert |
| `docs/api/vdi/attributes/_fill.md` | „640×480×16-Farben (4-Plane)"-Behauptung, `vsf_udpat`-Mehrplanenzahl „64 WORDs" | auf `INQ_TAB[4]=16` → `256` WORDs korrigiert (`attributes.rs:512-526`, EmuTOS `vdi_fill.c:155-160`) |
| `docs/api/aes/resource-fileselector.md` | §5.7-Blockquote behauptet noch „4-Plane, 16-Farben" für den **aktuellen** Backend — obwohl §5.9 (2026-09-19, TASK/A41) im selben Dokument bereits explizit sagt „§5.7's 'today 4 planes' is wrong for the live backend" | Korrekturblock direkt nach §5.7 eingefügt, der auf §5.9 verweist (keine Doppelung von §5.9s Analyse) |

**Bewusst unverändert gelassen** (kein Fund im Sinne des Auftrags):

- `docs/api/vdi/workstation/100-v_opnvwk.md:205` — STe/ST-Medium-Beispiel
  für EmuTOS' `get_palette()`-Semantik (generisches Beispiel, keine
  NewTOS-Aussage).
- `docs/api/vdi/raster/_raster.md:331` — Verifikationskriterium
  „source `fd_nplanes=1`, dest `fd_nplanes=4` → no-op": beliebiges
  Testbeispiel für die Plane-Mismatch-Regel, keine Aussage über den
  aktuellen Bildschirm.
- `docs/api/aes/window-graphics.md:436` — „auf der Referenz-TT, vier
  Planes" — explizit die TT-Referenz, wie in der Aufgabe verlangt
  ausgenommen.
- `docs/api/aes/resource-fileselector.md:908` — „jedes Icon in
  `cicons.rsc` ist eine einzelne 4-Plane(16-Farben)-Variante" — Aussage
  über den **Inhalt der .rsc-Datei auf der Platte** (gemessen), nicht über
  das aktuelle Bildschirmmodell.
- `docs/api/aes/object-form.md:500,511,512,530` — beschreibt EmuTOS'
  eigene Verzweigungslogik (`xor_is_ok`, ≤16 vs. >16 Farben) generisch;
  NewTOS nimmt korrekt den „>16 Farben"/„16-bit VDI"-Zweig, der in
  derselben Tabelle bereits richtig beschrieben ist.

## 3. Über den engen Suchbefehl hinaus (direkt anschließend gefunden, mitkorrigiert)

Beim Neuschreiben der Haupttabelle in `_raster.md` fiel auf, dass die
zitierten Bildschirm-Auflösungswerte (640×480) ebenfalls tote Zitate waren
(`framebuffer.rs:23/24` existiert nicht mehr; Geometrie ist heute ein
verhandeltes `AtomicU32`-Paar, Default 1920×1080,
`kernel/src/drivers/mod.rs:31-33`). Da diese Zahl in derselben Tabelle
steht wie die korrigierte Planzahl, wurde sie mitkorrigiert (sonst bliebe
die neu geschriebene Tabelle mit einem toten Beleg stehen). Dieselbe
Korrektur wurde konsequent in den vier direkt abhängigen
Workstation-Binding-Abschnitten nachgezogen (Dateien oben).

## 4. Nicht angefasst / gefunden, aber außerhalb des Auftrags

- **`docs/api/vdi/workstation/100-v_opnvwk.md`, Mismatch-Punkt 2 vs.
  `_discrepancies.md` D-VDI-7:** echter Widerspruch zwischen zwei
  Spec-Stellen — Mismatch #2 behauptet `DEV_TAB[3]/[4]` blieben am
  ROM-Default `372` hängen ("never patched"), D-VDI-7 (2026-09-04)
  behauptet sie seien wahrheitsgemäß auf `278/278` gepatcht. Der
  **Code** patcht sie tatsächlich (`workstation.rs:193,197-198`,
  `get_std_pixel_size`) — D-VDI-7 hat also recht, Mismatch #2 ist
  veraltet. Das ist ein Pixelgrößen-Fehler, **kein** Farbmodell-Fehler
  (außerhalb A86); im Text an der Fundstelle vermerkt, nicht behoben.
- **`docs/HARDWARE-PROFILE.md` §3.1** beschreibt „Geometrie (aktuelle
  Implementierung): 640×480, 32bpp" und „Farbmodell (unverändert durch
  ADR-017): 16-Farben, 4-Plane" — beides doppelt veraltet (Auflösung UND
  Farbmodell), aber diese Datei stand **nicht** im vorgegebenen Suchraum
  (`docs/api/vdi/`, `docs/api/aes/`, `docs/ARCHITECTURE.md`,
  `docs/COMPATIBILITY.md`) und ist ein deutlich größerer, eigener Fund —
  nicht angefasst.
- Generische „640×480"-Beispielrechnungen ohne Farb-/Plane-Bezug (z. B.
  `100-v_opnvwk.md:262` Verifikationskriterium, `_state.md:207`
  Cursor-Init-Position) — Formeln bleiben bei jeder Auflösung korrekt,
  keine Aussage über das Farbmodell, nicht Teil des Suchmusters, nicht
  angefasst.
- Kein `git checkout/stash/reset`, nichts committet, nichts geflasht.
