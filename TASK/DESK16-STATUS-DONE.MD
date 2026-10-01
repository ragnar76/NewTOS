# DESK16 — STATUS: DONE (2026-09-23)

**Auftrag:** Das Raster fuer `IG_REMOV` (5) aus `references/emutos/desk/icon.rsc`
auf demselben Weg uebernehmen, den die uebrigen sieben genommen haben,
`BUILTIN_ICONS` erweitern und den Rueckfall in `default_icon_kind` fuer diesen
Fall entfernen.

Basis: Branch `desk-rebuild`, HEAD `a8d4b42`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset`.

Messung: [`measurements/measurement-2026-09-23-desk16-desk17-desk21/`](../measurements/measurement-2026-09-23-desk16-desk17-desk21/).
Spezifikation: `docs/api/desktop/deskapp.md` (neuer Abschnitt
„the built-in icon set and `app_parse`'s icon clamp"),
`docs/api/desktop/deskins.md` (§ „The icon set"), `docs/api/desktop/deskwin.md`.

---

## 1. Die Analyse des Tickets — gegen die Quelle geprueft

**Richtig:** `icon.rsc` traegt acht freigeschaltete `ICONBLK`s (sechs weitere
stehen unter `#if 0`), NewTOS hatte sieben, und die Nummern sind Dateiformat —
`app_parse` liest sie (`deskapp.c:359-365`), `app_save` schreibt sie (`:1258`),
`desk_inf_data2` buchstabiert `07`/`02`/`06` woertlich.

**Zu ergaenzen, und leicht falsch zu machen:** ICONBLK[5] hat **keine eigene
Maske**. Die Ressource zeigt auf `rs_iconmask0`, die der Festplatte
(`icons.c`, `icon_rs_iconblk[5]`), und `ib_xchar`/`ib_ychar` sind ebenfalls die
der Festplatte, `(13,16)` — ein Wechselmedium traegt seinen Laufwerksbuchstaben
also an derselben Stelle.

**Zu ergaenzen:** „den Rueckfall entfernen" verlangt eine Antwort fuer den
Fall, den er mitbediente — eine Nummer ausserhalb `0..7`. EmuTOS hat die
Antwort an einer anderen Stelle, naemlich in `app_parse`
(`deskapp.c:359-365`), das genau deshalb `app_blddesk` ohne Pruefung
indizieren laesst. Die fehlte in NewTOS und ist hier nachgezogen.

---

## 2. Was gebaut wurde

`kernel/src/desk/icons.rs`:

* `IconKind::Removable` und `REMOV_DATA` — `rs_icondata5`, aus
  `references/emutos/desk/icons.c` (dem C, das der EmuTOS-Bau mit `ird` aus
  `desk/icon.rsc` erzeugt) entnommen. `rasters` gibt dafuer **`HARD_MASK`**
  aus, wie die Ressource es tut; `char_spot` gibt `(13,16)`, ebenfalls wie sie.
* `BUILTIN_ICONS` ist acht Eintraege lang = `NUM_GEM_IBLKS` (`deskapp.h:62`),
  neu als Konstante.
* `icon_kind`s Dokumentation sagt jetzt, was `None` heisst: `-1`, das `FF`
  einer `ANODE`-Zeile, und was `app_parse` wegklemmt.

Bei der Gelegenheit alle sieben vorhandenen Raster byteweise gegen `icons.c`
nachgeprueft — `HARD_DATA`/`HARD_MASK`, `TRASH_DATA`, `APP_DATA`, `DOC_DATA`
und die uebrigen stimmen exakt.

`kernel/src/desk/inf.rs`: `clamp_icon` in `parse_anode`, an der Stelle, an der
`app_parse` klemmt — `a_aicon >= NUM_GEM_IBLKS` → `IG_APPL`,
`a_dicon >= NUM_GEM_IBLKS` → `IG_DOCU`. `-1` liegt **unter** der Grenze und
bleibt: es heisst „dieser Knoten hat kein solches Bild", nicht „Index ausserhalb".

`kernel/src/desk/app.rs`: `app_blddesk` loest ohne Rueckfall auf;
`default_icon_kind` ist entfallen.

`kernel/src/desk/ins.rs`, `kernel/src/desk/win.rs`: die Kommentare, die das
Loch benannten, sagen jetzt, was wirklich uebrig bleibt.

### Genannte Abweichung

`a_aicon == -1` auf einer Schreibtischzeile. Nur eine handgeschriebene
`NEWDESK.INF` kann das tragen (`build_inf` schreibt immer die Nummer,
`deskapp.c:723`); EmuTOS greift dann auf `G.g_iblist[-1]`. NewTOS nimmt
`app_parse`s eigene Antwort fuer ein unbrauchbares `a_aicon`, `IG_APPL`.

---

## 3. Belege

`measurements/measurement-2026-09-23-desk16-desk17-desk21/` — QEMU raspi4b, 640x480,
`-display none`, QMP-Screendump, scratch-Kopie mit dem H02-IKBD-Injektor,
Karte je Lauf frisch aus `tools/make-sd-image.sh`. **Jeder** Klickpunkt stammt
aus einem seriellen Dump (`method/probe_patch.py`); keiner ist geraten.

* Die serielle Zeile des Kastens: `curr_icon=0 of **6**` vorher,
  `of **7**` nachher (acht Bilder minus die Diskette, die `offered_icons`
  weglaesst, solange kein `A:`/`B:` gemeldet wird).
* Vier Klicks auf den Pfeil nach unten: `before-i-step4.png` zeigt das
  **Programmbild**, `after-l-step4.png` das **Wechselmedium**
  (`crop-*-step4.png` = dieselben Vorschauen 8-fach).
* `after-m-installed.png` — `OK` setzt es auf das `C:`-Symbol, Buchstabe
  erhalten. `method/NEWDESK.INF.after-save`: `#M 00 00 05 FF C DISK C@ @`.
* `after-o-cold.png` — **Kaltstart** auf derselben Karte: das Bild ist wieder da.
* `method/pixcheck.py` liest die Raster aus `references/emutos/desk/icons.c`
  und vergleicht **jedes maskengesetzte Pixel** mit dem Screendump
  (`serial/pixcheck.txt`): Vorschau nachher 654 Pixel / 0 Abweichungen gegen
  `rs_icondata5`, Vorschau vorher 1006 / 0 gegen `rs_icondata6`,
  Schreibtischsymbol nachher 618 / 0 gegen `rs_icondata5`, vorher 618 / 0
  gegen `rs_icondata0`.

`[EXC]`/`[PANIC]`: 0 in allen elf raspi4b-Protokollen und im
qemu-virt-Bootprotokoll. `fsck_msdos -n` auf der FAT16-Partition jeder
geschriebenen Karte: Phase 1/2/3, keine Fehler (`serial/fsck.txt`, zehn
Abbilder). Kein `*.img` im Messverzeichnis.

### Bau

    cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat
    cargo xtask build --board rpi4        tos-api 64 / kernel 94   (Basis 64/94)
    cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat
    cargo xtask build --board qemu-virt   tos-api 64 / kernel 94   (Basis 64/94)

    cargo clippy --features board-rpi4       tos-api 80 / kernel 125 (Basis 80/125)
    cargo clippy --features board-qemu-virt  tos-api 80 / kernel 125 (Basis 80/125)
    Treffer in kernel/src/desk/: 0

    cargo run -p xtask -- test   938 bestanden / 0 fehlgeschlagen / 1 ignoriert

---

## 4. Offen

* Pi-4-Punkt **45** in `docs/api/desktop/_hw-test-2026-09-17.md`: das neue Bild
  in 1920x1080, das Schrittwerk mit der echten Maus, mehrere Laufwerke mit
  verschiedenen Bildern (QEMU hat nur `C:`), und die handgeschriebene
  Bildnummer ausserhalb `0..7`, die auf `IG_APPL` klemmen muss.

## 5. Nebenbefunde (nicht angefasst)

* `offered_icons` laesst die Diskette weg, solange kein `A:`/`B:` gemeldet
  wird. Das ist eine eigene, in `deskins.md` dokumentierte NewTOS-Regel und
  war nicht Gegenstand dieses Tickets — auf einer Maschine mit Diskette
  laufen jetzt acht Eintraege, auf dieser sieben.
