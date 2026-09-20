# A64 — STATUS: DONE

Der Dateiauswaehler faehrt EmuTOS' `fs_start`-Durchgang jetzt vollstaendig —
Flags **und** Geometrie, in derselben Reihenfolge. Bearbeitet 2026-09-17,
Branch `desk-rebuild`, Basis HEAD `c4b25e9`, Baum vorher sauber.
Messlauf: [`measurement-2026-09-17-fsel/`](../measurement-2026-09-17-fsel/RUNINFO.TXT).
Gemeinsam mit [A63](A63.md) bearbeitet, wie das Ticket verlangt; A64 zuerst
(Begruendung unten).

## Der Fehler, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `references/emutos/aes/gemfslib.c:59-152` (`fs_start`, `#if CONF_WITH_3D_OBJECTS`) |
| EmuTOS (Soll), Gegenpart | `references/emutos/aes/gemfslib.c:383-396` (`fs_format`, Schieber) |
| `ADJ3DSTD` | `references/emutos/include/obdefs.h:136` = 2 |
| `IP_4PATT` | `references/emutos/include/obdefs.h:36` = 4 |
| Warum Geometrie noetig ist | `references/emutos/aes/gemoblib.c:504-509` (`just_draw` waechst ein 3D-Objekt um `ADJ3DSTD` je Seite) |
| NewTOS (vorher) | `tos-api/src/aes/fsel.rs::set_3d_flags` — zehn Flags, **keine** Koordinate |
| NewTOS (nachher) | `tos-api/src/aes/fsel.rs::fs_start` — 13 Geometrieanpassungen, 14 Flags, 2 Musterwechsel |

Gepinnte Quelle: `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`.

## Was jetzt drinsteht — Zeile fuer Zeile

| EmuTOS-Zeile | Wirkung | Pruefwert im Test |
|---|---|---|
| `:72-73` | `FCLSBOX` FL3DACT, `x/y += ADJ3DSTD` | `(2, 2, 16, 16)` |
| `:78-80` | `FTITLE` FL3DBAK, `x += 2*ADJ3DSTD-1`, `height += 2*ADJ3DSTD` | `(19, 0, 160, 20)` |
| `:81-82` | FTITLE-TEDINFO `te_color = (te_color & 0xff8f) \| IP_4PATT<<4` | `0x11A1` → `0x11C1` |
| `:83-84` | `FILEBOX` `y += 3*ADJ3DSTD-1`, `width += ADJ3DSTD` | `(0, 21, 154, 176)` |
| `:86-87` | `SCRLBAR` `x += 2*ADJ3DSTD-1`, `y += 3*ADJ3DSTD-1` | `(155, 21, 24, 176)` |
| `:89-92` | `FUPAROW` FL3DACT, `x/y += ADJ3DSTD`, `width -= 2*ADJ3DSTD` | `(2, 2, 20, 16)` |
| `:94-95` | `FSVSLID` `y += 3*ADJ3DSTD`, `height -= 6*ADJ3DSTD` | `(0, 22, 24, 132)` |
| `:98` | `FSVSLID.ob_spec = (ob_spec & 0xffffff8f) \| IP_4PATT<<4` | `0x00FF1111` → `0x00FF1141` |
| `:99-101` | `FSVELEV` FL3DACT, `x += ADJ3DSTD`, `width -= 2*ADJ3DSTD` (y/h dynamisch) | `(2, 0, 20, 16)` |
| `:103-106` | `FDNAROW` FL3DACT, `x += ADJ3DSTD`, `y -= ADJ3DSTD`, `width -= 2*ADJ3DSTD` | `(2, 158, 20, 16)` |
| `:108-110` | `FILEAREA` `y -= 6`, `width += 2*ADJ3DSTD-1`, `height += 3*ADJ3DSTD-1` | `(24, 106, 179, 197)` |
| `:117-128` | 26 Laufwerkskaesten FL3DIND, `x += 2*ADJ3DSTD`, `y += 2*ADJ3DSTD + row*(3*ADJ3DSTD-1)`, `row` je Spalte zurueckgesetzt | A `(4,4)`, B `(4,25)`, I `(4,172)`, J `(36,4)`, Z `(68,151)` |
| `:130-131` | `FSDRIVES` `height += 2*ADJ3DSTD + DRIVE_ROWS*(3*ADJ3DSTD-1)`, `width += 4*ADJ3DSTD` | `(216, 96, 96, 193)` |
| `:135-138` | `FSDIRECT`/`FSSELECT`/`FSDRVTXT` FL3DBAK | Flagpruefung |
| `:140-147` | `FSOK` FL3DACT, `x = FILEAREA.x`, `y += ADJ3DSTD`; `FSCANCEL` FL3DACT, rechtsbuendig auf `FILEAREA`, gleiche `y` | `(24, 322, 72, 16)` / `(131, 322, 72, 16)` |
| `:149-150` | `ROOT` FL3DBAK, `height += 2*ADJ3DSTD` | `(0, 0, 320, 356)` |

Dazu der **Gegenpart in `fs_format`** (`:388-391`, `size_elevator`): `y += ADJ3DSTD`,
`h -= 2*ADJ3DSTD`, Mindesthoehe `gl_hbox`. Nicht im Ticket genannt, aber ohne ihn
haette der von `:99-101` verschmaelerte `FSVELEV` zwei Bildpunkte **ueber** seiner
Schiene gestanden — genau die Inkonsistenz, die A64 beseitigen soll. Der frueher
verwendete Mindestwert 8 ist durch `gl_hbox` (19) ersetzt, weil EmuTOS' Kommentar
an `:390` sich ausdruecklich darauf beruft (`2*ADJ3DSTD < gl_hbox`).

## Das `#if` ist entschieden, nicht vergessen

EmuTOS gattert den ganzen Durchgang mit `CONF_WITH_3D_OBJECTS` und hat einen
`#else`-Zweig, der stattdessen die Schieberbreiten auf `gl_wbox` angleicht.
NewTOS baut das Falcon030/TOS4-Profil, dort ist der Schalter 1
(`docs/api/aes/resource-fileselector.md`, "Build-time switches"), also existiert
nur der 3D-Zweig — und das steht jetzt als Begruendung im Kopfkommentar von
`fs_start`, nicht bloss in diesem Ticket.

## Reihenfolge: A64 vor A63 — Begruendung

1. Die Geometrie ist mechanisch und in Koordinaten pruefbar, also isoliert
   testbar. Der Schablonentest wird erst aussagekraeftig, wenn die Kaesten dort
   sitzen, wo EmuTOS sie hinstellt — ein Schablonenfeld in einem zu kleinen
   Kasten sieht anders aus und haette jede Bildabnahme zweideutig gemacht.
2. `fs_start` laeuft **einmal** beim Relokieren, `set_ted` bei **jedem** Aufruf.
   Die Ladezeit-Aenderung zuerst haelt die beiden Aenderungen im Diff getrennt.

Das deckt sich mit der Vermutung des Auftrags.

## Test vor Fix

`tos-api/src/aes/fsel.rs`, Modul `tests` (in-crate, weil `fs_start` privat ist und
keinen Rasterhintergrund braucht). Rote Ausgabe vollstaendig in
[`measurement-2026-09-17-fsel/build/newtest-RED.txt`](../measurement-2026-09-17-fsel/build/newtest-RED.txt):

```
---- fs_start_applies_every_geometry_adjustment_of_the_3d_pass ----
assertion `left == right` failed: FCLSBOX
  left: (0, 0, 16, 16)
 right: (2, 2, 16, 16)

---- fs_start_sets_every_3d_flag_emutos_sets ----
assertion `left == right` failed: FL3DACT on 34
  left: 0
 right: 1536

---- fs_start_switches_ftitle_and_fsvslid_to_the_50_percent_pattern ----
assertion `left == right` failed: FTITLE te_color
  left: 4513
 right: 4545
```

Danach gruen, siehe
[`newtest-GREEN.txt`](../measurement-2026-09-17-fsel/build/newtest-GREEN.txt).

## Folgeaenderung in `aes/object.rs` — eine Zeile, begruendet

`FTITLE` bekommt durch `:78` FL3DBAK. Unsere `*TEXT`-Zeichnung nullte die
Randstaerke fuer **jede** 3D-Art (`let mut th = if d3 != 0 { 0 } else {
ted.te_thickness }`), also haette FTITLE durch A64 seinen schwarzen 1-Punkt-Rahmen
verloren — im Referenzbild ist er da (wurzelrelativ Zeile 105, Spalte 203).
EmuTOS nullt eine Randstaerke nie wegen 3D (`gemoblib.c:661-667` zeichnet den Rand
bei jedem `th != 0`; nur die `G_TEXT`→`G_BOXTEXT`-Befoerderung setzt `tmpth = th = 0`,
`:597-606`). Der Schalter haengt jetzt an `FL3DOBJ` statt an `FL3DMASK` — das ist
dieselbe Regel, die der Kastenzweig derselben Funktion fuer `G_BOX` schon
ausformuliert (D7). Reichweite im ganzen Baum: `FSDIRECT`/`FSSELECT`
(`te_thickness` 0, unveraendert) und `FTITLE`; jeder andere FL3DBAK-Traeger im
Quelltext ist ein `G_BOX` und geht durch den anderen Zweig.

## Bildabnahme — was gemessen wurde

Referenz: `docs/analysis/fsel-reference-2026-09-17/fselectr.png` (`rscview`,
EmuTOS' eigene AES-Ressource, 3D, deutsch).

**Der Auftrag hielt einen direkten Pixelvergleich fuer unredlich, weil das Bild
mit `gl_hchar = 8` gerendert sei. Das trifft nicht zu, und es ist nachrechenbar:**
das Bild ist 326x362, der Wurzelkasten darin misst 320x356. `320` ist die Breite,
die unser konvertierter Blob fuer `ROOT` traegt, `356 = 352 + 2*ADJ3DSTD` ist
genau, was `:150` daraus macht, und `326 = 320 + 2*ADJ3DOUT` / `362 = 356 + 2*ADJ3DOUT`
ist der `OUTLINED`-Ring. Das Bild ist also mit **8x16** gerendert, derselben
Metrik wie unser Bau — und es zeigt den Zustand **nach** `fs_start`. Ein
wurzelrelativer Vergleich ist damit zulaessig.

Gemessen mit
[`method/compare-geometry.py`](../measurement-2026-09-17-fsel/method/compare-geometry.py)
(Schwarzlaeufe >= 10 Punkte je wurzelrelativer Zeile, also Kastenraender ohne
Textglyphen und ohne Dither):

```
root box: ours at (160,71) w=320   reference at (3,3) w=320
root height: ours 356  reference 356  (EQUAL)
338 root-relative rows identical, 18 different (runs of >= 10 black px)
```

Die 18 abweichenden Zeilen, einzeln zugeordnet — **keine** davon ist eine
`fs_start`-Koordinate:

| Zeilen | Ursache |
|---|---|
| 97, 118, 139, 140, 160, 181, 202, 223, 244, 265, 286 | der 1-Punkt-Rahmen der 26 Laufwerkskaesten (`G_BOXCHAR` + FL3DIND). Unser Zeichner gibt FL3DIND/FL3DACT-**Kastentypen** `th = 0` und ersetzt den Rand durch die Fase (D7, `object.rs`, vorbestehend). Innenrechteck und Lage stimmen punktgenau: Referenz Rand 217/248, innen 218..247 — unsere Fase sitzt auf denselben 218..247. |
| 105 | derselbe Grund fuer `FCLSBOX` (Referenzlauf 23..203, unserer 42..203 — die 19 fehlenden Punkte sind der Rahmen des Schliessers, der FTITLE-Anteil ab 42 stimmt) |
| 282 | derselbe Grund fuer `FDNAROW`; die Fuellung beginnt in beiden Bildern bei Zeile 283 |
| 146, 148, 167 | der 2-Punkt-Einzug des Schiebers aus `fs_format`. `rscview` rendert die Ressource, nicht einen laufenden Dialog, kann `fs_format` also gar nicht gefahren haben; der Unterschied ist das gewollte Ergebnis. |
| 161, 174 | der Mauszeiger im kopflosen Abzug (Sanduhr ueber der Liste) |

**Nur angesehen, nicht gemessen:** die Farbwerte. Unsere Palette gibt LWHITE
`(189,189,189)` und LBLACK `(140,140,140)`, `rscview` `(204,204,204)` und
`(135,135,135)`. Ein Farbvergleich haette nichts ausgesagt, deshalb vergleicht
das Programm ausschliesslich Schwarz.

## Beleglage

| | |
|---|---|
| Bild nachher | [`screendumps/fsel-after.png`](../measurement-2026-09-17-fsel/screendumps/fsel-after.png) (640x480) |
| Bild, auf Referenzgroesse beschnitten | [`screendumps/fsel-after-crop326x362.png`](../measurement-2026-09-17-fsel/screendumps/fsel-after-crop326x362.png) |
| Vergleichsausgabe | [`screendumps/compare-geometry.txt`](../measurement-2026-09-17-fsel/screendumps/compare-geometry.txt) |
| Serielles Protokoll | [`screendumps/fsel-after-BOOT-SERIAL.txt`](../measurement-2026-09-17-fsel/screendumps/fsel-after-BOOT-SERIAL.txt) |
| Hosttests | 551 → **557 gruen / 0 rot / 1 ignoriert** |
| Bau rpi4 / qemu-virt (release, frisch) | beide `rc=0`, tos-api 64 rustc, kernel 94 rustc — Grundlinie |
| Clippy | tos-api 81, kernel 125 (beide Boards) — Grundlinie |

## Rueckbau der Messverdrahtung

`fsel::preview()` war fuer den Abzug voruebergehend in `kernel/src/lib.rs`
verdrahtet und ist **von Hand** wieder entfernt (kein `git checkout`):

```
vorher   MD5 (kernel/src/lib.rs) = 5434f7b25c5641c7fab1fce0b1c98b85
nachher  MD5 (kernel/src/lib.rs) = 5434f7b25c5641c7fab1fce0b1c98b85
```

`git status` fuehrt `kernel/src/lib.rs` nicht mehr als geaendert.

## Offen

- Abnahme durch den Tester (Boot + Selbsttests + Schirmabzuege + `fsck_msdos`,
  beide Boards).
- Nicht committet; die Arbeit liegt im Baum (`tos-api/src/aes/fsel.rs`,
  `tos-api/src/aes/object.rs`, `docs/STATUS.md`, `TASK/`,
  `measurement-2026-09-17-fsel/`). Der *Recent-Decisions*-Eintrag traegt deshalb
  noch keinen Commit-Hash.

## Erratum (2026-09-20, A72)

Zeile 142 dieser Quittung nennt „Referenz Rand 217/248, innen 218..247" fuer
den 1-Punkt-Rahmen der Laufwerkskaesten. Das ist falsch abgeschrieben.
Gemessen — sowohl in dieser Quittung eigener Rohdatei
[`measurement-2026-09-17-fsel/screendumps/compare-geometry.txt`](../measurement-2026-09-17-fsel/screendumps/compare-geometry.txt)
(Zeilen 4, 8, 10, 18, 26, 28, 30, 32, 34, 38: `ref =[(217, 246), …]`) als
auch von [A67](A67-STATUS-DONE.MD) unabhaengig nachgemessen — ist der Rand
bei **217/246**, innen **218..245** (A67: „Rand 217/246, Fase 1 px auf
218/245, Flaeche 219..244"). Ob die damalige NewTOS-eigene Fase (vor dem
A67-Fix vom Folgetag) tatsaechlich auf 218..245 statt auf die hier
faelschlich genannten 218..247 traf, ist mit den Mitteln dieses Auftrags
nicht neu geprueft — nur die **Referenzzahlen** dieser Quittung werden hier
berichtigt. Keine Ruecknahme der Quittung, nur diese Richtigstellung;
Auftrag [A72](A72.md).
