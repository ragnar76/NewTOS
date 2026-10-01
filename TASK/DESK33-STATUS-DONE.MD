# DESK33 — STATUS: DONE (2026-09-25) — implementiert, Pi-Abnahme offen

**Auftrag:** `process_funkey` (`deskmain.c:1087-1106`) nachbauen — F1…F20
starten die gebundene Anwendung. Seit DESK20 kann eine Anwendung eine Taste
tragen; ein Druck darauf tat nichts.

Basis: Branch `desk-rebuild`, HEAD `12fddc1`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen,
keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset` auf dem gemeinsamen Baum.

Messung:
[`measurements/measurement-2026-09-25-desk-rest/`](../measurements/measurement-2026-09-25-desk-rest/) §5.
Spezifikation: [`docs/api/desktop/deskmain.md`](../docs/api/desktop/deskmain.md).
Hardwarepunkt **70**.

---

## Der `ANODE`

EmuTOS laeuft `G.g_ahead` ab, seine eine Liste aus Schreibtischsymbolen **und**
installierten Anwendungen. NewTOS teilt die beiden, und nur die
Zuordnungshaelfte — `desk::inf`s Sitzungsmodell — kann ueberhaupt ein
`a_funkey` tragen: ein Schreibtischknoten wird mit `a_funkey = 0` angelegt
(`deskins.c:243,825,1037,1385`) und niemand setzt es je. `inf::app_by_funkey`
laeuft darum nur die Zuordnungsliste ab und findet genau denselben Knoten.
Zurueck kommen die beiden Felder, die `do_aopen` fuer eine installierte
Anwendung liest: `a_pappl` und `a_flags & AF_APPDIR`.

## Der Start

`do_aopen(pa, TRUE, -1, pathname, pfname, NULL)` — `isapp` schliesst den
`installed_datafile`-Arm aus (`desksupp.c:924`), `tail == NULL` den
Schwanz-Zweig. Uebrig bleiben drei Schritte: das Arbeitsverzeichnis (aus dem
eigenen `AF_APPDIR`/`a_pappl`, weil der Knoten eine installierte Anwendung
*ist*), das Zusammensetzen von `pathname` + `pfname`, und `pro_run`. Das ist
genau der Rumpf von `main::do_aopen_drop`, also ruft `process_funkey` ihn mit
leerem Schwanz auf statt ihn abzuschreiben — `pro_run` schreibt fuer
`tail == NULL` denselben leeren Schwanz `00 00 0D` (`deskpro.c:56-57`).

`curr = -1` unterdrueckt bei EmuTOS nur `pro_run`s `do_wopen`
(`deskpro.c:61-62`); NewTOS startet geschachtelt und zeichnet beim Ruecksprung
neu, also gibt es nichts zu unterdruecken.

`hndl_kbd` gibt die Antwort direkt zurueck (`deskmain.c:1224-1226`,
`if (done >= 0) return done;`) — wie CR/ESC/Pfeile darueber und anders als
alles darunter laeuft dieser Weg **ohne** `men_update`. Vorher stand dort ein
`men_update()`; unbeobachtbar, solange `check_function_key` immer `None` gab.

## Benannte Luecke, keine neue

`opn_appl`s Parameterabfrage fuer `.TTP`/`.GTP` (`desksupp.c:978-985`) gibt es
fuer `.NTP` auch beim Doppelklick nicht (`launch_native`, Stage A). Eine `.NTP`
an einer Taste startet also mit demselben leeren Schwanz wie beim Doppelklick
— dieselbe benannte Luecke, jetzt von einer zweiten Stelle aus erreichbar.

---

## Geaenderte Dateien

* `kernel/src/desk/main.rs` — `check_function_key` (ruft jetzt
  `process_funkey`), `process_funkey` (neu), `hndl_kbd` (kein `men_update`)
* `kernel/src/desk/inf.rs` — `FunkeyApp`, `app_by_funkey` (neu)
* `docs/api/desktop/deskmain.md`,
  `docs/api/desktop/_hw-test-2026-09-17.md` (Punkt 70)

## Geprueft

* `cargo run -p xtask -- test` — **944 passed / 0 failed / 1 ignored**.
* Frischer Build je Board: **tos-api 64 / kernel 94**, Delta 0.
* `clippy` beide Boards: **tos-api 80 / kernel 125**, Delta 0, null Treffer in
  `kernel/src/desk/`.
* QEMU raspi4b, Lauf `d33` auf einer Karte, deren `NEWDESK.INF` der Desktop
  selbst geschrieben hat (Lauf `d33-seed`, *Options → Save desktop*) und der
  eine Zeile angefuegt wurde:
  `#G 06 FF C:\ARGVTEST.NAP@ *.XYZ@ 107 @`. Der Zeiger ist den ganzen Lauf in
  der Ecke geparkt; die einzige Eingabe ist die Taste.

  ```
  [d33] process_funkey(7) -> C:\ARGVTEST.NAP appdir=true
  ARGVTEST: cmdlin len 0 raw ""
  ARGVTEST: argc 1
  [d33] process_funkey(6) -> no application holds it
  ```

  Null `[EXC]`, null `[PANIC]`, `fsck_msdos -n` sauber.

## Offen

* **Hardwareabnahme**, Punkt 70: die Taste ueber *Install application…*
  vergeben, echtes `F7`, und derselbe Test nach einem Kaltstart.
* Der Weg ueber den Dialog (Taste eintippen, Save desktop, F7) ist headless
  nicht gefahren worden — dafuer braucht es ein Textfeld und eine echte
  Tastatur. Die Datenhaelfte davon ist DESK20 und gemessen; hier ist die
  gelesene `#G`-Zeile der Beleg, dass beide Haelften zusammenpassen.
