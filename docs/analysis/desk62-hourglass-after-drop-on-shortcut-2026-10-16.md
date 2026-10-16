# DESK62 — Sanduhr nach Drop auf eine Desktop-Verknuepfung: Ursache (2026-10-16)

Phase 1 von 2: Ursache mit Beleg. Kein Produktcode geaendert.
Messlauf: `measurements/measurement-2026-10-16-desk62/RUNINFO.TXT`.

## Ergebnis in einem Satz

Der Desktop **verwirft den Rueckgabewert des Starts** auf dem Pfad
„Datei aus einem Fenster auf ein Desktop-Symbol" (`fun_win2desk` →
`fun_file2desk`, Zweig `AT_ISFILE`): `pro_exec` hat die Sanduhr gesetzt und
`shel_write(SHW_EXEC)` war erfolgreich, aber `fun_drag` meldet `false`, also
verlaesst `deskmain` seine Ereignisschleife nicht, die AES-Shell startet das
Programm nie, und das `desk_busy_off()` beim naechsten Desktop-Start, das in
EmuTOS die Sanduhr zuruecknimmt, findet nie statt. Der Fehler liegt in
`programs/desktop/`, nicht in der AES.

## Beobachtung in QEMU (raspi4b, Injektor)

Karte: `C:\QED.NAP`, `C:\qed.rsc`, `C:\README.TXT`, `NEWDESK.INF` mit einem
Fenster `C:\*.*` und einer Verknuepfung `#X … C:\QED.NAP@ QED.NAP@`.
Spur: eine Zeile pro AES-Aufruf, im Kern vor dem Dispatch abgegriffen
(`method/d62-trace.py`, nur in der Scratch-Kopie).

| | Drop auf die Verknuepfung (`ds`) | Drop auf `QED.NAP` im Fenster (`dw`, Kontrolle) |
|---|---|---|
| Ziehen | `graf_mouse(4)` FLATHAND, am Ende `graf_mouse(0)` ARROW | gleich |
| `set_launch_dir` | `graf_mouse(2)`, `graf_mouse(0)` | gleich |
| `pro_exec` | `graf_mouse(2)` HGLASS, `shel_write(1,1)` | gleich |
| `pro_run` → `do_wopen` | `graf_growbox` (73) = die „Start-Animation" | gleich |
| danach | `wind_set`, `men_update`, **`evnt_multi` (25)**: Desktop bleibt in der Schleife | `wind_get`, `shel_get/put`, `menu_bar(0)`, **`appl_exit` (19)** |
| Shell | nichts | `desktop NEWDESK … exited`, `starting C:\QED.NAP` |
| Bild | Sanduhr bleibt (+15 s, nach Mausbewegung und Klick) | QED mit `C:\README.TXT`; nach Ctrl-Q Desktop mit Pfeil |

Bildbelege: `screendumps/ds-3-drop-15s.png`, `ds-4-after-click.png` (Sanduhr);
`dw-2-qed-before-quit.png` (QED hat die Datei geladen), `dw-4-desktop-back.png`
(Pfeil).

Die Sanduhr verschwindet erst, wenn der Desktop auf anderem Weg endet: ein
Doppelklick auf dieselbe Verknuepfung startet QED (ohne Datei,
`ds-5-after-dclick.png`), nach dessen Ende setzt der neu startende Desktop
`graf_mouse(0)` (`ds-8-desktop-back.png`).

**Abweichung zum Befund des Anwenders:** In QEMU startet QED nach dem Drop
**nicht**; die AES haelt nur den vorgemerkten Start (`sh_doexec = SHW_EXEC`,
`C:\QED.NAP`, Tail `README.TXT`). Sichtbar ist die Wachstumsbox aus `pro_run`
(`do_wopen`), die wie ein Programmstart aussieht. Ob auf der Pi tatsaechlich
QED lief, sollte der Anwender noch einmal pruefen (z. B. ob ein Doppelklick
dazwischen lag).

## Gegenueberstellung EmuTOS ↔ NewTOS

### Wo EmuTOS die Sanduhr setzt und wo sie wieder verschwindet

| Schritt | EmuTOS | NewTOS |
|---|---|---|
| Sanduhr an, `shel_write`, bei Fehler aus | `deskpro.c:35-45` `pro_exec` | `programs/desktop/src/pro.rs:88-95` `pro_exec` — identisch |
| `desk_busy_on/off` = `graf_mouse(HGLASS/ARROW)` | `desksupp.c:1840-1852` | `pro.rs:34-42` — identisch |
| Pfeil zurueck beim Desktop-Start | `deskmain.c:2071-2074` (`menu_bar`; `desk_busy_off()`) | `programs/desktop/src/lib.rs:411-417` — identisch |
| Ende der Ereignisschleife bei `done` | `deskmain.c:2101-2104` | `programs/desktop/src/main.rs:80-88` — identisch |

EmuTOS setzt die Sanduhr also bei **jedem** erfolgreichen Start und nimmt sie
**nie** im Desktop-Lauf zurueck, der den Start ausgeloest hat: die Annahme ist,
dass dieser Lauf jetzt endet. Genau diese Annahme bricht NewTOS auf dem
Drop-Pfad.

### Wo der Start-Erfolg zum Schleifenende wird

| EmuTOS | NewTOS |
|---|---|
| `deskfun.c:61` `static BOOL exit_desktop;` | keine Entsprechung |
| `deskfun.c:1304` `fun_drag`: `exit_desktop = FALSE;` | — |
| `deskfun.c:1322-1323` `fun_win2desk(wh, dobj, keystate);` | `dir.rs:1674` gleich, Ergebnis `()` |
| `deskfun.c:1117-1132` `fun_win2desk` → `fun_file2desk(...)` | `dir.rs:1996-2016` gleich |
| `deskfun.c:1021-1034` `case AT_ISFILE:` … **`exit_desktop = do_aopen(an_dest, TRUE, dobj, pathname, filename_start(an_dest->a_pappl), tail);`** | `dir.rs:2071-2072` `super::main::do_aopen_drop(…);` **`false`** — Rueckgabe verworfen |
| `deskfun.c:1338` `return exit_desktop;` | `dir.rs:1684` **`false`** (nur `fun_win2win`, `dir.rs:1666`, gibt seinen Wert zurueck) |
| `deskmain.c:977` `done = fun_drag(...)` | `main.rs:589` `let done = dir::fun_drag(...)` — gleich |

Der Aufrufweg ist in der Spur belegt, nicht erschlossen: `graf_mouse(4)`
(`gr_drgplns`, `main.rs:395`) → `graf_mouse(0)` (`main.rs:490`) →
`graf_mouse(2)`/`(0)` (`set_default_path`, `pro.rs:66-72`, aus
`set_launch_dir`, `main.rs:1024`) → `graf_mouse(2)` + `shel_write(1,1)`
(`pro_exec`) → `graf_growbox` (`pro_run` → `do_wopen`) → `evnt_multi`.

### AES-Seite

`graf_mouse` (`tos-api/src/aes/graf.rs:613`) setzt die Formen wie verlangt —
die Kontrolle zeigt HGLASS → ARROW ueber den Neustart des Desktops.
`shel_write` (`tos-api/src/aes/shell.rs:470`) merkt den Start vor wie
`gemshlib.c:168-176`; gestartet wird erst, wenn der Desktop endet
(`sh_main`/`sh_ldapp`). Die AES tut auf beiden Pfaden dasselbe; der
Unterschied entsteht ausschliesslich im Desktop.

### Dieselbe Luecke an zwei weiteren Stellen

- `fun_desk2win` (`dir.rs:2129-2139`): Desktop-Symbol auf ein Programm im
  Fenster — EmuTOS `deskfun.c:1249` setzt `exit_desktop`, NewTOS verwirft den
  Wert (`return;`). Gleiches Symptom erwartet, nicht gemessen.
- `fun_desk2desk` (`dir.rs:2171-2257`): EmuTOS geht ueber `fun_file2any` →
  `fun_file2desk` (`deskfun.c:1297`, `:1180`) und startet bei einer
  Programm-Verknuepfung als Ziel; NewTOS kennt nur Disk/Papierkorb und
  meldet sonst `illegal_op_msg`. Das ist ein **fehlender Zweig**, kein
  verworfener Wert — eigener Auftrag.

### Begruendung steht an mehr als einer Stelle

`docs/api/desktop/deskfun.md:566-567` nennt noch als „benannte Abweichung":
„NewTOS startet geschachtelt (AES-Luecke G1) und gibt `false` zurueck". Das
war vor DESK43 U3 (`e0069e5e`) richtig; seither startet auch NewTOS nur ueber
`shel_write` plus Desktop-Ende. Der Doc-Kommentar von `fun_drag`
(`dir.rs:1657-1661`: „the window-to-window branch is the one that can
launch") und der von `fun_file2desk` (`dir.rs:2016-2017`: „File/Folder-shortcut
… are Task #14") sind aus derselben Zeit. Der `false`-Rueckgabewert in
`fun_file2desk` kam mit DESK30 (`f85f9c84`, 2026-09-24), also vor U3; mit U3
wurde nur `fun_win2win` umgestellt. Deshalb besteht der Fehler seit dem
eingebauten Desktop nach U3 — passend zur Gegenprobe des Anwenders.

## Vorgeschlagener Fix (Phase 2)

1:1 nach EmuTOS (`deskfun.c:61,1304,1338`), in `programs/desktop/src/dir.rs`:

1. Ein modulweiter Merker `exit_desktop` (z. B. `static EXIT_DESKTOP:
   AtomicBool` oder ein Feld im Desktop-Zustand), den `fun_drag` am Anfang auf
   `false` setzt und am Ende zurueckgibt.
2. `fun_file2desk`, Zweig `AType::File` (`dir.rs:2071`): `exit_desktop =
   do_aopen_drop(...)` statt Verwerfen; der Zweig selbst gibt weiterhin
   `false` zurueck (EmuTOS: `operation` bleibt `-1`, `ret = FALSE`,
   `deskfun.c:1080-1083`), damit `fun_win2desk` kein `fun_rebld` ausloest.
3. `fun_desk2win` (`dir.rs:2129`): `exit_desktop = do_aopen_drop(...)`
   (`deskfun.c:1249`).
4. `fun_win2win` (`dir.rs:1958`): den Wert ebenfalls in `exit_desktop`
   schreiben statt direkt zurueckzugeben (`deskfun.c:967-968`), damit es nur
   noch eine Quelle gibt; `fun_drag` gibt in allen Zweigen `exit_desktop`
   zurueck.
5. Doku nachziehen: `fun_drag`- und `fun_file2desk`-Kommentar,
   `docs/api/desktop/deskfun.md:566-567`.

Nicht Teil des Fixes: der fehlende `fun_file2any`-Weg in `fun_desk2desk`
(eigener Auftrag). Die AES bleibt unberuehrt.

## Test, der vorher rot und nachher gruen ist

Der Injektorlauf dieses Messlaufs, unveraendert auf einem Kern aus dem
reparierten Baum:
`method/build.sh kds method/drop-shortcut.script launch:QED=method/qed-quit.script`
(mit `D62_ALL=1`), dann `method/run.sh kds <tag> …` wie in RUNINFO §3.

Gruen heisst:
- seriell nach `aes op=121 in0=1` (dem `shel_write` des Drops) ein
  `aes op=19` des Desktops, dann `[shell] sh_ldapp: desktop NEWDESK … exited`
  und `starting C:\QED.NAP` — **vor** dem Doppelklick (Injektor-Byte 69/+6000);
- das Bild ~4 s nach dem Drop zeigt QED mit dem Fenster `C:\README.TXT` (wie
  `dw-2-qed-before-quit.png`);
- nach Ctrl-Q Desktop mit Pfeil und `graf_mouse(0)` aus dem Desktop-Start.

Rot (heute): nach `op=121` folgt `evnt_multi`, kein `op=19`, keine
Shell-Zeile bis zum Doppelklick; `ds-3`/`ds-4` zeigen die Sanduhr.

Ein Selbsttest im Desktop (`selftest-desk`) waere moeglich, ruft aber
zwangslaeufig `shel_write(SHW_EXEC)` mit Sanduhr und Wachstumsbox auf und
muesste den vorgemerkten Start danach wieder aufheben; der Injektorlauf ist
der ehrlichere Beleg.
