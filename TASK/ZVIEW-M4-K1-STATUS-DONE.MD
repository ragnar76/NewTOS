# ZVIEW-M4-K1 — Quittung (FERTIG): die AES besitzt eine echte physikalische VDI-Workstation, `graf_handle` liefert sie

Datum 2026-10-07 · Basis `desk-rebuild` `81d0949` + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-07-k1](../measurements/measurement-2026-10-07-k1/RUNINFO.TXT)
· Befund: [ZVIEW-M4-STATUS-UNFINISHED.MD](ZVIEW-M4-STATUS-UNFINISHED.MD) § Blocker K1

## Was EmuTOS tut (Quelle, Baum `1a7484ab`)

* `run_accs_and_desktop` ruft zu Beginn jeder AES-Sitzung `gsx_init()`
  (`aes/geminit.c:675`), vor `load_accs`.
* `gsx_wsopen` öffnet die physikalische Workstation:
  `v_opnwk(intin, &gl_handle, &gl_ws)` (`aes/gemgsxif.c:169-186`) →
  `gl_handle = 1` (`vdi/vdi_control.c:544`).
* `gsx_start` richtet sie ein (`aes/gemgraf.c:383-428`): `vqt_attributes`,
  `vst_height(ws_chminh)`, `vst_height(ws_chmaxh)`, `vsl_type(7)`,
  `vsl_width(1)`, `vsl_udsty(0xffff)`.
* `graf_handle` gibt `gl_handle` zurück (`aes/gemsuper.c:287-293`); der
  Desktop zeichnet auf genau diesem Handle (`desk/deskmain.c:1956`).
* Sitzungsende: `gsx_wsclose()` = `v_clswk` (`geminit.c:749`) schließt alle
  virtuellen Workstations, die physikalische bleibt (`vdi_control.c:573-590`).

## Was geändert ist

* `tos-api/src/aes/gsxif.rs`: `gl_handle()`, `gsx_init()` (wsopen + die
  VDI-Aufrufe von `gsx_start`, über den VDI-Dispatcher mit einem
  Kernel-LP64-Block), `gsx_wsclose()`.
* `tos-api/src/aes/graf.rs`: `graf_handle` liefert `gsxif::gl_handle()`
  statt der Konstante 1.
* `kernel/src/lib.rs`: Sitzungsschleife ruft `gsx_init()` vor `load_accs`
  und `gsx_wsclose()` nach `free_accs` (auch auf dem Shutdown-Pfad).
* `kernel/src/desk/vdibind.rs`: der Desktop zeichnet auf dem Handle von
  `graf_handle`; das eigene `v_opnvwk` (die benannte Abweichung, Grund
  „Handle 1 nie offen“) ist entfernt. Damit belegt der Desktop nach einem
  Ziehen keinen virtuellen Handle mehr. Das erste `v_opnvwk` eines Programms
  bekommt wieder 2, wie in EmuTOS.
* Spec vor Code: `docs/api/aes/window-graphics.md` (§ `graf_handle`),
  `docs/api/aes/shell-loop.md`, `docs/api/vdi/workstation/01-v_opnwk.md`
  (Korrektur „opened once at graphics init“ + offener Punkt),
  `docs/api/desktop/deskact.md`.
* Tests: neu `tos-api/tests/aes_k1_gl_handle.rs` (3 Tests: WinDoms
  `ApplInit`-Folge `vq_extnd(graf_handle,1|0)` beantwortet; `gsx_start`-Zustand;
  Nummerierung 2/3, `v_clswk` am Sitzungsende, nächste Sitzung wieder 2).
  Gegen `HEAD` rot (3/3), danach grün. Die Prüfstände, die `graf_handle`
  aufrufen, führen jetzt den AES-Start `gsx_init()` aus.

## Messwerte

| Messung | Soll | Ist |
|---|---|---|
| Hosttests | 1172/0/1 + neu | **1175/0/1** |
| Release-Bau rpi4 / qemu-virt (tos-api/kernel) | 64/94 | **64/94** |
| clippy rpi4 / qemu-virt | 76/125 | **76/125**, 0 Treffer in geänderten Dateien |
| `desk-drift-probe.sh` | 31 | **31** |
| SDK-Hosttests | 16/0/0 | **16/0/0** |
| zView Produkt, memprot on | 15/15 | **15/15** pixelgenau |
| zView Produkt, strict (T.JPG T.PI1 T4.IMG) | 3/3 | **3/3** |
| APITEST | 106/0/5 | **106/0/5** |

Regression (QEMU raspi4b, memprot on): EmuDESK mit Fenster, Ordner und
Textansicht, Ziehen eines Icons mit Umriss, TeraDesk und QED („Neuer
Text“): alle Screendumps bytegleich zum HEAD-Kernel. LDGDEMO
(WinDom-Client): on = strict, Programm endet mit rc 0. Fünf Dumps weichen in
der Knopfzeile ab: „Compute“ und „Close“ liegen jetzt auf grauem statt
weißem Grund. Das ist die beabsichtigte Wirkung von K1. WinDom liest jetzt
`app.color = 256` und füllt `DRAW3D`-Objekte ab 16 Farben mit `actcol`
(`userdef.c:390-400`).

## Referenzzellen

Keine Zelle der VDI- und AES-Referenzsuiten hat sich verschoben. Der
VDI-Prüfstand öffnet `v_opnwk` selbst. Der AES-Prüfstand fährt jetzt wie die
Dumper-Maschine `gsx_init` vor `appl_init`; Fall 216 (`graf_handle` = 1)
stimmt.

## Offen

* `v_opnwk` leert `vwk_ptr[2..]` nicht (`vdi_control.c:544-545`). Das steht
  in `01-v_opnwk.md` als offener Punkt und gehört nicht zu K1.
* qemu-virt nur gebaut und geclippt, nicht gebootet. Ohne Framebuffer meldet
  `graf_handle` 0.
* Echte Pi: nicht gefahren. `docs/STATUS.md`: Lead.
