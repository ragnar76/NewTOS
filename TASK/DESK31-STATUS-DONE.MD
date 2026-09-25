# DESK31 — STATUS: DONE (2026-09-25) — implementiert, Pi-Abnahme offen

**Auftrag:** die sechs Reste des Dateiwerks — `d_dofileren` fragt nicht,
`STILLDIR` fehlt, `desk_verify` fehlt in `rebuild_window`,
`fun_rebld_marked`/`WN_REBUILD` gibt es nicht, zwei Wortlaute weichen ab, und
der Papierkorb behaelt einen NewTOS-Vorbau.

Basis: Branch `desk-rebuild`, HEAD `12fddc1`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen,
keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset` auf dem gemeinsamen Baum.

Messung:
[`measurement-2026-09-25-desk-rest/`](../measurement-2026-09-25-desk-rest/) §4.
Spezifikation: [`docs/api/desktop/deskdir.md`](../docs/api/desktop/deskdir.md)
und [`deskfun.md`](../docs/api/desktop/deskfun.md). Hardwarepunkt **71** in
[`_hw-test-2026-09-17.md`](../docs/api/desktop/_hw-test-2026-09-17.md).

---

## 1. `d_dofileren` fragt (`deskdir.c:785-825`)

War ein blankes `dos_rename(old, new) == E_OK`. Ist jetzt EmuTOS' Schleife:
`dos_rename` → bei Fehlschlag `item_exists(new, is_folder)`, und wenn nichts
im Weg steht `illegal_op_msg()` + `false`; steht etwas im Weg,
`get_new_name(new)` — derselbe `ADCPALER`-Baum, den der Dateikonflikt benutzt.
`Stop`/`Skip` brechen ab; `OK` mit **unveraendertem** Namen und
`oldname != newname` loescht das Ziel, damit die naechste Runde umbenennen
kann.

Signatur: `d_dofileren(old: &str, new: &mut String, is_folder: bool,
ctx: &mut OpCtx) -> bool`. `new` ist `&mut`, weil `get_new_name` es an Ort und
Stelle aendert, wie EmuTOS' `newname`. `d_dofoldren` ist kein eigenes Stueck
mehr: bei EmuTOS existiert es nur, um die `\*.*` abzuschneiden, die sein
Aufrufer traegt, und unser Laeufer traegt dort keine.

**Was `false` heisst.** Bei EmuTOS „die ganze Operation anhalten" — fuer den
echten Fehler (der Alert steht schon) wie fuer ein *Skip*. Beide Aufrufstellen
in `dir_op` setzen darum `ctx.stop` und lassen `ok` in Ruhe; ein `ok = false`
wuerde die NewTOS-Zusammenfassung „Some items could not be processed."
dazuschalten, die hier weder Fehler noch EmuTOS-Verhalten waere.

## 2. `STILLDIR` (`deskdir.c:998-1004`)

`dir_op`s `source_is_parent`-Zweig setzte `ok = false` und ging weiter. Jetzt:
`illegal_dir_op_alert()` — `[1][Illegal directory operation!][ Skip | Abort ]`
(`desk_rsc.c:2496`) —, *Skip* laesst die Auswahl weiterlaufen, *Abort* setzt
`ctx.stop`. Keine der beiden Antworten ist ein Fehler; `ok` bleibt wie es war,
genau wie EmuTOS' `more` bei seinem `break` `TRUE` bleibt.

Der Alert hat zwei Knoepfe, also baut er seinen Text selbst — `fun_alert`
klammert fest in einen Ein-Knopf-Alert; dieselbe Loesung wie bei
`delete_disk_alert`/`STDELDIS`.

## 3. `desk_verify` in `rebuild_window` (`deskfun.c:140`)

Wir hatten `win_bldview` allein. `desk_verify(handle, work, true)` tut
dasselbe **und** setzt `g_cwin`/`g_croot`/`g_wlastsel`
(`desksupp.c:175-199`) — beobachtbar, weil `fun_rebld` jedes passende Fenster
neu aufbaut und ein Menuebefehl direkt danach auf das zuletzt aufgebaute wirkt.

## 4. `fun_mark_for_rebld` / `fun_rebld_marked` (`deskfun.c:119-167`)

Neu. `WN_REBUILD` (`deskwin.h:51`) gab es schon als Konstante ohne Leser.
Der Aufschub ist ihr Zweck: ihr einziger EmuTOS-Aufrufer ist `do_info`
(`desksupp.c:1336`), und `do_info` laeuft in einer Schleife ueber die Auswahl
eines Fensters — ein Neuaufbau mittendrin wuerfe genau die `FNODE`s weg, die
sie abschreitet. Damit ist DESK31 Punkt 4 die Voraussetzung fuer DESK38.

Der Pfadvergleich ist wie bei `fun_rebld` der auf das Verzeichnis, nicht auf
die volle Suchmaske — die Form, in der jeder NewTOS-Aufrufer seinen Pfad hat.

## 5. Der Wortlaut von `illegal_op_msg`

Drei Stellen sagten „Operation not allowed.", EmuTOS sagt an allen dreien
`STILLOP` („Illegal operation!", `desk_rsc.c:2497`):
`fun_op`s fehlgeschlagener Zaehllauf (`deskfun.c:794`), `fun_file2desk`s
`default` (`:1075-1076`) und `fun_desk2desk`s „weder Platte noch Papierkorb"
(`:1286`). Die dritte stand nicht im Ticket — es ist dieselbe Funktion, und
sie mit einem abweichenden Text stehen zu lassen waere der Fehler gewesen, den
das Ticket beseitigen will.

## 6. `wants_to_delete_files` statt `fun_confirm`

`STRMVDEL` (`desk_rsc.c:2523-2524`):

```text
[2][Do you want to remove icons|or delete files?][Icons|Files|Cancel]
```

Drei Antworten statt zwei, andere Reihenfolge, anderer Wortlaut. Der
`MN_SELECTED`-Umweg der Antwort *Icons* liegt jetzt **in** der Funktion
(`deskfun.c:1443`), nicht bei ihren Aufrufern — so bekommt ihn auch der
Papierkorb-Zweig von `fun_desk2desk`, der bis hierher `fun_confirm` hatte.
`DeleteChoice` und `fun_confirm` sind weg: die Aufzaehlung hatte nur noch
einen Leser, und `fun_confirm` hatte nach dem Umbau keinen und keine
EmuTOS-Entsprechung.

Die Bedingung `if (icontype_src >= 0)` (`:1048`) ist in `fun_desk2desk` immer
wahr — `fun_file2any` reicht `an_src->a_type` durch (`:1179-1180`) —, also
fragt der Papierkorb fuer ein Laufwerk wie fuer eine Verknuepfung, und er
fragt unabhaengig von *Confirm deletes*.

---

## Geaenderte Dateien

* `kernel/src/desk/dir.rs` — `d_dofileren`, `illegal_dir_op_alert`,
  `illegal_op_msg` (nur Doku + drei Aufrufstellen), `dir_op` (beide
  Rename-Zweige, `source_is_parent`-Zweig), `wants_to_delete_files`,
  `fun_del_desktop`, `fun_desk2desk`, `rebuild_window`, `fun_mark_for_rebld`,
  `fun_rebld_marked`, `selftest` (angepasste Signatur), `fun_confirm` entfernt
* `docs/api/desktop/deskdir.md`, `deskfun.md`,
  `docs/api/desktop/_hw-test-2026-09-17.md` (Punkt 71)

## Geprueft

* `cargo run -p xtask -- test` — **944 passed / 0 failed / 1 ignored**, wie die
  Basis.
* Frischer Build je Board nach `cargo clean -p kernel -p tos-api`: `--board
  rpi4` und `--board qemu-virt`, beide **tos-api 64 / kernel 94**, Delta 0.
* `cargo clippy --features board-rpi4` und `--features board-qemu-virt`:
  **tos-api 80 / kernel 125**, Delta 0, **null** Treffer in
  `kernel/src/desk/`.
* QEMU raspi4b, vier Laeufe (`d31a`, `d31b`, `d31c`, `d31d`): alle vier
  sichtbaren Stellen mit Screendump, die Dateiwirkung von Punkt 1 mit
  `method/lsimg.py` aus dem Rohabbild zurueckgelesen. Null `[EXC]`, null
  `[PANIC]`, `fsck_msdos -n` sauber.

## Offen

* **Hardwareabnahme**, Punkt 71: die vier Gesten mit echter Maus und echter
  Control-Taste auf der Karte C:–F:.
* Punkt 3 und 4 haben kein eigenes Bild — sie sind Buchhaltung. Punkt 4 wird
  von DESK38s Schleife gefahren, Punkt 3 von jedem Fensteraufbau in jedem der
  Laeufe.
