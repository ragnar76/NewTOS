# DESK24 — STATUS: DONE (2026-09-23)

**Auftrag:** Den *Ordner*-Namenskonflikt bauen — EmuTOS' `output_path`/
`get_new_name` (`references/emutos/desk/deskdir.c:700-779`) —, weil NewTOS
bisher wortlos in einen bestehenden Ordner hineinmischte.

Basis: Branch `desk-rebuild`, HEAD `50714b7`, Aenderungen **uncommitted** im
Baum (eine Datei, `kernel/src/desk/dir.rs`). Nichts geflasht, nichts
ausgeworfen, keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset` auf dem gemeinsamen Baum. Einziges
Git-Kommando: das lesende `git show HEAD:kernel/src/desk/dir.rs`, mit dem der
„Vorher"-Kern gebaut wird.

Messung: [`measurements/measurement-2026-09-23-desk24-desk25-desk26/`](../measurements/measurement-2026-09-23-desk24-desk25-desk26/) §2.
Spezifikation: `docs/api/desktop/deskdir.md`.

---

## 1. Die Analyse des Tickets — gegen die Quelle geprueft

**Richtig, und nachgemessen.** Der Vorher-Lauf auf dem HEAD-Kern
(`serial/d24-before-serial.log`) zeigt den zaehlenden Kasten und danach
**keine** `CPALER`-Zeile; die Karte hat hinterher `C:\FOLD\{EXIST.TXT,
INNER.TXT}` — hineingemischt, ungefragt.

**Praezisiert** hat die Quelle zwei Punkte:

1. **Nur der oberste Ordner wird gefragt.** `output_path` wird ausschliesslich
   aus `dir_op`s Ordnerzweig gerufen (`deskdir.c:1002`). `d_doop`s rekursiver
   Zweig legt seine Unterordner mit einem nackten `dos_mkdir` an und mischt
   still, wenn sie schon da sind (`:626-632`). Das bisherige Verhalten war
   also nicht „falsch ueberall", sondern „falsch auf der obersten Ebene".
2. **Die Reihenfolge ist Teil des Verhaltens.** `output_path` kommt **vor**
   dem Selbstvergleich `strcmp(srcpth,dstpth)==0` (`:1002-1010`). Ein Ordner,
   der auf seinen eigenen Elternordner gezogen wird, wird darum erst gefragt
   und danach still verworfen. NewTOS hatte den Selbstvergleich vorn; er ist
   an EmuTOS' Platz gerueckt.

---

## 2. Was gebaut wurde

Alles in `kernel/src/desk/dir.rs`.

### 2.1 `get_new_name(dstpth, ctx) -> NewName`

`deskdir.c:700-723`. Nimmt den Blattnamen des Zielpfades, fuellt **beide**
Felder des `ADCPALER`-Kastens damit (`inf_sset(CACURRNA)`/`inf_sset(CACOPYNA)`,
`:705-709`), fuehrt `do_namecon` aus und schreibt den — moeglicherweise
geaenderten — Namen zurueck in `dstpth`. Rueckgabe ist EmuTOS' `2`/`1`/`0`/`-1`
als `NewName::{Changed, Unchanged, Stop, Skip}`.

Es ist **derselbe** Kasten wie beim Dateikonflikt, weil EmuTOS
`desk_rs_trees[ADCPALER]` fuer beide wiederverwendet; die Ueberschrift bleibt
darum `NAME CONFLICT DURING COPY`. Der inhaltliche Unterschied: beim
Dateikonflikt steht in `CACURRNA` der **Quell**name (`:304-307`), beim
Ordnerkonflikt der **Ziel**name.

### 2.2 `output_path(op, dstpth, ctx) -> More`

`deskdir.c:734-779`. Die Schleife mit ihren zwei Zweigen:

| Fall | Abbruch |
|---|---|
| Kopieren/Verschieben (Laufwerkswechsel) | `dos_mkdir` gelingt → fertig. Sonst existiert der Ordner; `OK` auf unveraendertem Namen (`ret == 1`) heisst **hineinmischen** und bricht ab. |
| `Op::Rename` (Verschieben per `Frename`) | Das Ziel darf **nicht** existieren. Ein unveraenderter Name bricht darum *nicht* ab — es wird erneut gefragt, bis ein freier Name kommt oder `Stop`/`Skip`. |

`Stop`/`Skip` kommen als `More::Stop`/`More::Skip` zurueck und heissen in
`dir_op` genau EmuTOS' `0`/`-1`: `Stop` beendet den Auftrag, `Skip`
ueberspringt diesen einen Ordner.

EmuTOS haengt zum Schluss `\*.*` an; NewTOS' Maschine nimmt Verzeichnispfade
mit `\` am Ende, das haengt die Aufrufstelle mit `as_dir` an.

### 2.3 `item_exists(path, is_folder)`

`deskdir.c:186-208`. Die zweite Form der Probe war bisher weggelassen (mit
einer Notiz, die genau dieses Ticket vorwegnahm). Das angehaengte `\*.*` ist
das, was sie ordner-**exklusiv** macht: ein `Fsfirst` auf `C:\FOO` findet auch
eine *Datei* `FOO`, `C:\FOO\*.*` nur ein Verzeichnis — ueber dessen
`.`-Eintrag, den unser `Dcreate` anlegt (`tos-api/src/bdos/fs/dirops.rs`).

### 2.4 `illegal_op_msg()`

`deskdir.c:220-224`. `STILLOP` (`desk_rsc.c:2497`, *„Illegal operation!"*) und
das `FALSE`, das der Aufrufer als „anhalten" liest. Fuer den Fall, dass
`dos_mkdir` fehlschlaegt, **ohne** dass etwas im Weg steht — voller
Wurzelordner, unzulaessiger Name.

### 2.5 Die Verdrahtung

`dir_op`s Ordnerzweig fuer `Copy`/`Move`/`Rename`: `source_is_parent` →
`output_path` → Selbstvergleich → `d_dofoldren`/`d_doop`, in EmuTOS'
Reihenfolge (`:999-1018`). Das fruehere `let _ = dos_mkdir(&dst_sub); //
output_path (create/merge)` ist ersetzt.

---

## 3. Die Nachweise

Messlauf, QEMU raspi4b, 640x480, scratch-Kopie mit dem IKBD-Injektor; jede
Antwort **am Kartenabbild** gelesen (`method/lsimg.py`, `serial/cards.txt`),
nicht vom Bildschirm.

    vorher   kein CPALER; C:\FOLD = {EXIST.TXT, INNER.TXT}       d24-before-d-after.png
    nachher  [d24] CPALER ... curr="FOLD" copy="FOLD"            d24-merge-d-conflict.png

  * `OK` unveraendert → `output_path(Copy, "C:\FOLD") -> Ok, dst now "C:\FOLD"`;
    Karte: `C:\FOLD\{EXIST.TXT 27, INNER.TXT 34}`, Quelle unberuehrt.
  * `2` getippt → `copy now "FOLD2"`,
    `output_path(Copy, "C:\FOLD") -> Ok, dst now "C:\FOLD2"`;
    Karte: neuer Ordner `C:\FOLD2\INNER.TXT 34`, `C:\FOLD` nur `EXIST.TXT`.

`[EXC]`/`[PANIC]`: 0. `fsck_msdos -n` auf beiden geschriebenen Karten:
Phase 1/2/3, rc=0.

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

* Pi-4-Punkt **48** in `docs/api/desktop/_hw-test-2026-09-17.md`: `Skip`/`Stop`
  am Ordnerkasten, der `Op::Rename`-Zweig (Verschieben auf demselben
  Laufwerk), und die Tastatur im Namensfeld unter der echten Aufloesung.

## 5. Nebenbefunde (nicht angefasst)

* **`d_dofileren` fragt nicht.** EmuTOS' `d_dofileren` hat eine eigene
  `item_exists`/`get_new_name`-Schleife samt „Ziel loeschen, damit `Frename`
  gelingt" (`deskdir.c:785-825`); NewTOS' `d_dofileren` ist ein blankes
  `dos_rename`. Betrifft das Verschieben einer **Datei** auf demselben
  Laufwerk ueber eine gleichnamige. Eigenes Ticket wert.
* **`STILLDIR` fehlt.** `dir_op`s `source_is_parent`-Zweig setzt bei uns nur
  `ok = false`; EmuTOS fragt `Skip | Abort` (`deskdir.c:999-1003`).
* **Zwei Wortlaute fuer `STILLOP`.** Der neue `illegal_op_msg()` nimmt EmuTOS'
  Text *„Illegal operation!"*; die zwei aelteren Stellen, die bei EmuTOS
  dieselbe Funktion rufen (`deskfun.c:794` in `fun_op`, `:1075` in
  `fun_file2desk`), sagen weiter *„Operation not allowed."*. Angleichen waere
  eine Zeile je Stelle, gehoert aber nicht in dieses Ticket.
* **`d_doop`s Rekursionszweig meldet keinen Fehler.** EmuTOS prueft dort
  `dos_mkdir`s Fehlschlag gegen `item_exists` und meldet notfalls
  `illegal_op_msg` (`:626-632`); NewTOS verwirft den Rueckgabewert.
