# DESK42 — STATUS: DONE (2026-09-25) — implementiert, Pi-Abnahme offen

**Auftrag:** `ADDISKIN` und `ADFFINFO` aus `references/emutos/desk/desk_rsc.c`
Objekt fuer Objekt nachbauen — dieselbe Reihenfolge, dieselben Objektnummern,
dieselben Beschriftungen, so wie DESK03 es fuer `ADINSDSK` und DESK05 fuer
`ADDESKCF` gemacht haben. DESK38 hatte die Kaesten funktional gebaut, nicht in
ihrer Form; der Anwender: *„funktioniert, aber das Fenster hat mit dem von
EmuTOS rein gar nichts gemeinsam."*

Basis: Branch `desk-rebuild`, HEAD `0b179c3`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen, keine
Pi gefahren, nichts committet; kein `git checkout/restore/clean/stash/reset`
auf dem gemeinsamen Baum, nur die lesenden `git diff` und `git status`. Die SD-
Karte unter `/Volumes/BOOT 1` wurde nicht angefasst; alle Abbilder liegen im
Job-Scratch.

Messung:
[`measurements/measurement-2026-09-25-desk42/`](../measurements/measurement-2026-09-25-desk42/).
Spezifikation:
[`docs/api/desktop/deskinf.md`](../docs/api/desktop/deskinf.md) (§ *die zwei
Kaesten sind die Ressourcenbaeume*, DESK42) und
[`desksupp-redraw.md`](../docs/api/desktop/desksupp-redraw.md).
Hardwarepunkt **69**.

---

## 1. Die Masse stehen gepackt in der Ressource

`ob_x`/`ob_y`/`ob_width`/`ob_height` sind in `desk_rsc.c` **gepackt**: das
untere Byte zaehlt Zeichenzellen, das obere (vorzeichenbehaftet) addiert Pixel
(`fix_chpos`, `aes/gemrslib.c:53-83`). `ADDISKIN`s Wurzelhoehe `2060` ist
`(8 << 8) | 12` — zwoelf Zeilen plus 8 px, nicht „2060". Wer das uebersieht,
baut den Kasten falsch.

| Baum | Zellen | Pixel (8x16) | gemessenes ROOT |
|---|---|---|---|
| `ADDISKIN` (`:709-768`) | 38 x (12 + 8 px) | **304 x 200** | (168, 149) |
| `ADFFINFO` (`:622-703`) | 40 x 16 | **320 x 256** | (160, 121) |

Vorher: `inf_disk` 340 x 230 mit sieben `G_STRING`-Zeilen, `inf_file_folder`
320 x 200 mit einem Namensfeld, drei Prosazeilen und zwei Knoepfen.

## 2. Die Baeume, Objekt fuer Objekt

**`ADDISKIN`, zehn Objekte** (`desk_rsc.h:75-84`): ROOT (`G_BOX`, `FL3DBAK`,
`OUTLINED`, `ob_spec` 69888), `DITITLE`, sechs `G_FBOXTEXT`-Felder mit
`FL3DBAK` (`DIDRIVE`, `DIVOLUME`, `DINFOLDS`, `DINFILES`, `DIUSED`,
`DIAVAIL`), `DIOK` und `DICNCL` mit `FL3DACT`.

**`ADFFINFO`, vierzehn Objekte** (`desk_rsc.h:61-72`): ROOT, `FFTITLE`,
`FFNAME` (das einzige `EDITABLE`), `FFSIZE`, `FFDATE`, `FFTIME`, `FFNUMFOL`,
`FFNUMFIL`, der namenlose `G_IBOX` und darin `FFRWRITE`/`FFRONLY` als
`RBUTTON`-Paar, dann `FFOK`, `FFSKIP`, `FFCNCL`.

Beide Baeume werden zur Laufzeit Objekt fuer Objekt ausgegeben
([`serial/dlg-dumps.txt`](../measurements/measurement-2026-09-25-desk42/serial/dlg-dumps.txt)),
damit man sie gegen die Ressource halten kann, ohne unseren Quelltext zu lesen.
Typ, Flaggen und Zustand stimmen Zeile fuer Zeile mit `desk_rsc.c` ueberein.

Der `G_IBOX` ist nicht Zierde: `RBUTTON`-Abwahl geschieht **je Elternobjekt**
— dieselbe Begruendung, aus der `ADDESKCF`s zwei Gruppen je einen eigenen
haben (DESK18/DESK19).

## 3. Warum die Zeilen nicht buendig links stehen

`te_just` tut **zweierlei**, und das Zweite uebersieht man leicht: `ob_format`
fuellt die Unterstriche der Schablone vom linken oder rechten Ende her, und
`gr_gtext`/`gr_just` (`gemgraf.c:567-589`) setzen die **fertige Zeichenkette**
links, mittig oder rechts in das Objektrechteck.

`ADDISKIN`s sechs Felder sitzen alle in Zeichenspalte 3 mit demselben
32-Zellen-Rechteck, aber nur `DINFOLDS`' Schablone ist 32 Zeichen lang; die
kuerzeren `TE_RIGHT`-Schablonen werden um ihren Ueberschuss nach rechts
geschoben. Schablonenlaengen 31/25/32/30/25/30 ergeben linke Kanten
32/24/24/40/80/40. Die Treppe auf dem Screendump ist **die Ressource**, kein
Layoutfehler.

Ebenso: `Disk label:  NEWTOS__.___` zeigt die nicht gefuellten Unterstriche der
8.3-Schablone. Das ist EmuTOS' Optik, nicht ein halb gefuelltes Feld.

## 4. Drei Hilfsfunktionen, die die Baeume brauchten

* **`fmt_str`/`unfmt_str`** (`util/optimize.c:43-93`) — `SAMPLE.PRG` ↔
  `SAMPLE  PRG`. `FFNAME` zeigt die Schablonenform, `inf_file_folder` rechnet
  sie vor dem Umbenennen zurueck. `set_tedinfo_name` (`deskinf.c:424-430`) ist
  `fmt_str` + `inf_sset` und fuellt `DIVOLUME`.
* **`inf_numset`** (`:409-419`) — `sprintf("%*lu", te_txtlen-1, value)`.
* **`align_title`** (`deskmain.c:1731-1747`) — Objekt 1 auf dem Kasten
  zentriert. Gemessen: `FILE INFORMATION` bei x 256, `FOLDER INFORMATION` bei
  x 248, `DISK INFORMATION` bei x 256 — genau `(Breite − len*8)/2`.

`fmt_time` hat ein Argument mehr bekommen: den Trenner zwischen `hh` und `mm`.
EmuTOS variiert ihn ueber die `sprintf`-Formatzeichenkette, die es uebergibt —
Textansicht `"%02d:%02d%s"` (`:293`), `inf_dttm` `"%02d%02d%s"` (`:467`), weil
`ADFFINFO`s Schablone `Time:   __:__ __  ` den Doppelpunkt selbst traegt. Der
Aufruf der Textansicht uebergibt `":"` und ist unveraendert.

## 5. Zwei benannte Abweichungen von DESK38 sind damit geschlossen

Ein Baum mit Knoepfen, hinter denen nichts steht, waere ein Stub. Also ist
`inf_file_folder` wieder EmuTOS' Funktion: `inf_file_folder(ppath, pf) -> i16`
mit der `while(1)` aus `deskinf.c:551-635` **in** ihr, und der Aufrufer
(`desksupp.c:1335-1337`) ist auf die zwei Zeilen der Quelle geschrumpft. Damit
faellt auch `InfFF` weg — EmuTOS' `0`/`1`/`-1` reicht.

* **Attributrueckschreibung** (`:585-605`) — `dos_chmod(srcpth, F_SETMOD,
  attr)`, neue Bindung auf GEMDOS `Fattrib` (0x43) in `desk::dosbind`. Das
  Ergebnis wird absichtlich nicht geprueft: „like Atari TOS, we don't check the
  return code from dos_chmod()" (`:592-593`).
* **`STRENAME`-Retry** (`:628-634`) — ein gescheitertes `dos_rename` bringt
  `[1][Rename error!][ Retry | Cancel ]` (`desk_rsc.c:2557`), und *Retry*
  springt an den Anfang der Schleife zurueck.

Dazu zwei Dinge, die die neuen Objekte erst fuellen: fuer einen **Ordner**
zaehlt der Kasten jetzt den Inhalt (`count_ffs` → `inf_fifosz` in
`FFNUMFIL`/`FFNUMFOL`/`FFSIZE`, `:519-533`), fuer eine **Datei** bekommen die
beiden Zaehler `DISABLED` und leeren Text, sodass die Schablone ausgegraut
durchscheint (`:535-545`). `FFSKIP` — den dritten Knopf gab es bei uns nicht —
ist `inf_what(tree, FFSKIP)`s `1`: weiter mit dem naechsten markierten Eintrag.

## 6. Die eine Abweichung, die bleibt

EmuTOS' `pf` **ist** der lebende `FNODE` des Fensters; unserer ist die Kopie,
die `do_info` daraus gemacht hat. (Die Form ist die der Quelle: fuer eine
Verknuepfung baut `desksupp.c:1326` genau so eine Kopie auf dem Stapel.) Das
Fenster wird von demselben `-1` aufgefrischt, das ohnehin faellig ist —
`fun_mark_for_rebld` + `fun_rebld_marked` lesen das Verzeichnis neu ein —,
also stimmen beide wieder ueberein, bevor sie jemand liest. Gemessen: nach dem
Setzen von *Read-Only* zeigt derselbe Kasten beim zweiten Oeffnen `Read-Only`
als gewaehlt.

## 7. Zeilen ohne Inhalt: keine

Der Auftrag verlangt, jede Zeile zu benennen, die bei uns leer bleibt, weil der
Wert darunter fehlt. **Es gibt keine.** Alle sechs Felder von `ADDISKIN` und
alle sechs von `ADFFINFO` haben einen Wert; die zwei Zaehler einer Datei sind
absichtlich leer, und das ist EmuTOS' eigenes `inf_sset(tree, FFNUMFIL, "")`
plus `DISABLED`, nicht eine Luecke bei uns.

## 8. Nebenbefund — GEMDOS, nicht Desktop, **nicht hier behoben**

Wird das Namensfeld geleert und OK gedrueckt, ist das Ziel das blosse
Verzeichnis `C:\` — EmuTOS hat dafuer ebenfalls keinen Sonderfall
(`deskinf.c:600-626`). EmuTOS' GEMDOS weist das ab
(`xrename`: `if (!ixsfirst(p2,FA_SUBDIR,0)) return EACCDN`,
`bdos/fsdir.c:912-913`). **Unseres nicht:**
`Frename("C:\README.TXT", "C:\")` liefert `E_OK` und schreibt einen
Verzeichniseintrag mit **vollstaendig leerem** 8.3-Namen; im Fenster steht dann
ein Dateisymbol ohne Beschriftung.

Ursache ist allgemein, nicht umbenennspezifisch:
`tos-api/src/bdos/fs/dir.rs:148` `fcb_name("")` liefert `Some([b' '; 11])`, und
jeder GEMDOS-Einstieg mit einem letzten Pfadbestandteil geht dort durch.

Das ist ein GEMDOS-Defekt und gehoert nicht in DESK42s Umfang. DESK42 macht ihn
**erreichbar**: der bisherige NewTOS-Kasten wies einen leeren Namen selbst ab,
was EmuTOS nicht tut, also wurde der Aufruf nie abgesetzt. **Braucht ein
eigenes Ticket.** Beleg:
[`measurements/measurement-2026-09-25-desk42/`](../measurements/measurement-2026-09-25-desk42/) §6, Lauf
`d42e` (Screendump + Kartenabzug). `fsck_msdos -n` meldet den Eintrag nicht —
ein durchgehend leerer Name ist FAT-strukturell gueltig, nur als 8.3-Name
unzulaessig.

---

## Geaenderte Dateien

* `kernel/src/desk/inf.rs` — `inf_disk` und `inf_file_folder` neu gebaut;
  neu: `fmt_str`, `unfmt_str`, `set_tedinfo_name`, `inf_numset`, `inf_fifosz`,
  `rsc_ted`, `align_title`, `DISABLED`, `TE_LEFT`/`TE_RIGHT`, `LEN_ZFNAME`;
  `fmt_time` bekommt den Trenner als Argument; `InfFF`, `NAME_MAX` und
  `G_BOXTEXT` entfallen
* `kernel/src/desk/main.rs` — `do_info_file_folder` baut einen
  `dir::FileInfo`; `show_item_info` ist auf `desksupp.c:1335-1337` geschrumpft
* `kernel/src/desk/dosbind.rs` — `dos_chmod` + `F_SETMOD` + `FATTRIB` (neu)
* `kernel/src/desk/win.rs` — der eine `fmt_time`-Aufruf uebergibt `":"`
* `docs/api/desktop/deskinf.md` — neuer DESK42-Abschnitt; der DESK38-Absatz,
  der die `G_STRING`-Bauweise begruendete, ist richtiggestellt (er beschrieb
  einen Baum, den es nicht mehr gibt), ebenso die DESK38-Abweichungsnotiz und
  der `fmt_time`-Absatz
* `docs/api/desktop/desksupp-redraw.md` — dieselbe Abweichungsnotiz dort
* `measurements/measurement-2026-09-25-desk42/` — neu

`docs/STATUS.md` **nicht** angefasst (Lead fuehrt es).

## Geprueft

* `cargo run -p xtask -- test` — **944 passed / 0 failed / 1 ignored**.
* Frischer Build je Board (`board-qemu-virt`, `board-rpi4`): **tos-api 64 /
  kernel 94** Warnungen, Delta **0**.
* `clippy` beide Feature-Saetze: **tos-api 80 / kernel 125**, Delta **0**, null
  Treffer in `kernel/src/desk/`.
* Rauchprobe je Board mit dem Produktkern (frisch je Board gebaut):
  `raspi4b` bootet bis `[desk] desktop up`, `virt` bis zur EmuCON-Ruecklage
  (kein Bildschirm auf diesem Board) — 0 `[EXC]`, 0 `[PANIC]`. Das ist eine
  billige Fruehwarnung, kein Beleg; die Bootprotokolle lagen im Job-Scratch und
  sind mit ihm geloescht. Das verbindliche Boot-/Selftest-Urteil faellt der
  Tester.
* QEMU `raspi4b`, 640x480, IKBD-Injektor in der Kratzkopie, **neun** Laeufe
  (`probe`, `d42a1`, `d42a`, `d42b1`, `d42b`, `d42c`, `d42d1`, `d42d`, `d42e`):
  0 `[EXC]`, 0 `[PANIC]` in allen neun Protokollen, `fsck_msdos -n` sauber auf
  allen zehn Kartenabbildern (`serial/fsck.txt`).

| Lauf | was | Screendump |
|---|---|---|
| `d42a` | `ADDISKIN` auf dem C:-Symbol | `d42a-c-diskinfo.png` |
| `d42b` | `ADFFINFO` fuer `C:\README.TXT` und `C:\SUB` | `d42b-c-fileinfo.png`, `d42b-f-folderinfo.png` |
| `d42c` | *Read-Only* setzen, Karte liest `attr` 0x20 → 0x21 | `d42c-g-again.png` |
| `d42d` | `STRENAME` + *Retry* bringt denselben Kasten zurueck | `d42d-f-strename.png`, `d42d-g-retry.png` |
| `d42e` | Nebenbefund §8 | `d42e-e-afterok.png` |

## Offen

* **Hardwareabnahme**, Punkt 69 — ein echter Mausklick auf einen Radioknopf,
  eine echte Ruecktaste im Namensfeld und `count_ffs` ueber die vier echten
  Partitionen.
* **Der Nebenbefund aus §8 braucht ein eigenes Ticket** (GEMDOS `Frename` /
  `fcb_name`, leerer Namensbestandteil).
* `TASK/GUI05.md` bleibt offen: `align_title` ist hier bewusst in der
  klassischen TOS-Auspraegung gebaut (zentriert, ohne Linie), wie alle anderen
  NewTOS-Dialoge auch. Der Umstieg auf
  `CONF_WITH_ALT_DESKTOP_GRAPHICS` betrifft alle sechs Kaesten auf einmal und
  gehoert nicht in dieses Ticket.
