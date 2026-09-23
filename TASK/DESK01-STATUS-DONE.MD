# DESK01 — STATUS: DONE (2026-09-24)

**Auftrag:** `Desk → About` (Desktop-Info) als echten Dialog ueber die
DESK08-Kette, mit eigenem Inhalt und EmuTOS-naher Baumform.

Basis: Branch `desk-rebuild`, HEAD `8d271cb`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, kein `git checkout/restore/stash/reset`.

## Was EmuTOS an dieser Stelle tut

`desk/deskmain.c:612-651` `do_deskmenu(item)` hat genau einen Fall,
`ABOUITEM`:

```c
tree = desk_rs_trees[ADDINFO];
start_dialog(tree);
while (!done) {
    touchob = form_do(tree, 0) & 0x7fff;
    if (touchob == DEICON) { /* CONF_WITH_EASTER_EGG: play_sound(...) */ }
    else done = TRUE;
}
(tree + DEOK)->ob_state = NORMAL;
end_dialog(tree);
```

Der Baum `ADDINFO` (`desk/desk_rsc.c:772-873`): `OUTLINED`-Wurzelkasten
40x19 Zeichen, `- EmuTOS -` zwischen zwei `G_IMAGE`-Logos, ein
`Version`-Etikett + Wert, der Urheberrechtsblock, die Projektadresse, der
GPL-Hinweis und ein `DEOK`-Knopf (`desk_rsc.c:869-873`). Die Version steht
**nicht** im Baum: `desk_xlate_fix` zeigt `DEVERSN->ob_spec` beim Start auf
die BIOS-Zeichenkette `version[]`, die auch der Startschirm druckt
(`deskmain.c:1881`).

## Was NewTOS hatte

`main::do_deskmenu` ignorierte sein Argument und warf einen `form_alert`:

```rust
let text = b"[1][ NewTOS Desktop | EmuTOS-conformant rebuild | ADR-016 (Task #8) ][ OK ]\0";
super::form_alert(1, text.as_ptr());
```

Ein Alert ist kein Dialog: es ist der AES-eigene `DIALERT`-Baum mit eigenem
`bb_save`/Restore, ohne reserviertes Dialogrechteck und ohne `WM_REDRAW` an
die Fenster, die er verdeckt hat. Seit DESK08 gibt es die Kette, die genau
das leistet.

## Was getan wurde

**Neu: `kernel/src/desk/inf.rs::do_aboutdesk`** — der Baum liegt bei den
uebrigen `desk/`-Dialogen, weil dort jeder NewTOS-Dialogbaum liegt; der
Aufruf steht bei EmuTOS' Fall:

```rust
fn do_deskmenu(item: Item) {
    if item == Item::About {
        super::inf::do_aboutdesk();
    }
}
```

**Der Baum** (zehn Objekte, Wurzel im Ursprung, `form_center` setzt sie):

| # | Typ | Inhalt | Quelle |
|---|---|---|---|
| 0 | `G_BOX` | der Kasten, aus dem Text bemessen | |
| 1 | `G_STRING` | `- NewTOS -`, zentriert | |
| 2/3 | `G_STRING` ×2 | `Version` + `NewTOS 0.1` | `initinfo::VERSION_STRING` |
| 4/5 | `G_STRING` ×2 | `Machine` + Brettname | `initinfo::machine_name()` |
| 6/7 | `G_STRING` ×2 | der Unabhaengigkeitshinweis, am eigenen Gedankenstrich geteilt | `initinfo::INDEPENDENCE_NOTICE` |
| 8 | `G_STRING` | die Urheberzeile | `initinfo::AUTHORSHIP_LINE` |
| 9 | `G_BUTTON` | ` OK ` (`SELECTABLE\|DEFAULT\|EXIT\|LASTOB`) | = `DEOK` |

**Eine Quelle, zwei Leser.** Die drei Texte sind jetzt `pub(crate)`-Konstanten
in `kernel/src/initinfo.rs`, und der Startschirm liest sie von dort statt aus
eigenen Literalen. Das ist genau EmuTOS' Anordnung (siehe `deskmain.c:1881`)
und verhindert, dass die Version auf dem Desktop von der auf dem Startschirm
abweichen kann.

Ein `G_STRING` wird vom AES **linksbuendig an `ob_x`** gezeichnet
(`aes/gemoblib.c:817-845`) — es gibt kein Zentrierattribut, also wird eine
zentrierte Zeile *platziert*, nicht ausgezeichnet. Dafuer haelt `Line` die
Zeichenzahl neben dem NUL-terminierten Puffer.

## Drei benannte Abweichungen

1. **`inf_show` statt der `form_do`-Schleife.** EmuTOS schleift nur, weil
   `DEICON` ein `TOUCHEXIT` ist, das die Osterei-Melodie spielt und **nicht**
   schliesst (`CONF_WITH_EASTER_EGG`). Ohne so ein Objekt liefe die Schleife
   genau einmal, also sind es die drei Aufrufe von `inf_show`.
2. **Der Kasten wird aus seinem Text bemessen**, nicht auf 40 feste Spalten.
   Die Maschinenzeile ist brettabhaengig, es gibt also keine Breite zum
   Einbacken. EmuTOS loest dasselbe Problem fuer seine Menues zur Laufzeit
   (`adjust_menu`, `deskmain.c:1608-1635`).
3. **Keine Logos.** `ADDINFO`s zwei `G_IMAGE` kommen aus den `BITBLK`s der
   Ressourcendatei; NewTOS hat keine Desktop-Ressourcendatei.

## Kein Baudatum — ein Befund, keine Auslassung

`ADDINFO` traegt `DECOPYRT`, gefuellt aus `COPYRIGHT_YEAR`. NewTOS hat
**nirgends** eine Baudatums- oder Baujahrskonstante:

* der OSHEADER-`os_date` ist ein ausdruecklicher 1980-01-01-Platzhalter, und
  die Luecke ist dort schon festgehalten
  (`tos-api/src/bdos/process/loader.rs:806-809`, „NewTOS pins no build-date
  constant; … Flagged to the lead");
* die `Boot time`-Zeile des Startschirms ist die GEMDOS-Uhr, nicht der Bau.

Eine der beiden zu drucken hiesse, etwas Unwahres ueber den Bau zu behaupten.
Die Zeile fehlt daher, bis es eine echte Konstante gibt. Eine solche waere
eine Aenderung an `kernel/build.rs` (und damit an der Reproduzierbarkeit des
Baus) und gehoert in ein eigenes Ticket.

## Pruefung

**Hosttest: keiner moeglich** (dieselbe Begruendung wie in DESK09/DESK10:
`kernel/` traegt `forced-target`, `cargo run -p xtask -- test` faehrt nur
`tos-api`/`tos-types`/`xtask`).

**Bau:** frisch je Board (`cargo clean -p kernel -p tos-api` davor);
`cargo xtask build --board rpi4` und `--board qemu-virt`: **tos-api 64 /
kernel 94 auf beiden**, rc 0 (Basis 64/94, Delta 0). Clippy tos-api **80**,
kernel **125** auf beiden (Basis 80/125), **null** Treffer in
`kernel/src/desk/`. `cargo run -p xtask -- test`: **932/0/1**.

**QEMU raspi4b** (Scratch-Kernel mit dem DESK08/09-IKBD-Injektor, 640x480,
`method/desk-desk01.actions`): `Desk` aufklappen → `Desktop info...` → `OK`.

* `screendumps/dlg-C-about.png`: der Kasten steht gemessen bei
  **(128, 111, 384, 276)** — genau `form_center`s Wert
  (`(640-384)/2 = 128`, `19 + (480-19-276)/2 = 111`). Er zeigt
  `- NewTOS -`, `Version  NewTOS 0.1`,
  `Machine  Raspberry Pi 4 (BCM2711)`, den Unabhaengigkeitshinweis in zwei
  Zeilen und die Urheberzeile ueber einem `OK`-Knopf.
* `screendumps/dlg-D-closed.png` gegen `dlg-A-idle.png`: **bytegleich**
  (md5 `710d7f24cc078d43e9351725a205a16e` auf beiden, pixdiff
  **0 differing pixels**). Der Kasten verschwindet rueckstandsfrei, ohne dass
  der Desktop neu aufgebaut wird.
* Null `[EXC]`/`[PANIC]`.

## Artefakte

* `kernel/src/desk/inf.rs` (`Line`, `do_aboutdesk`),
  `kernel/src/desk/main.rs` (`do_deskmenu`), `kernel/src/initinfo.rs`
  (drei `pub(crate)`-Konstanten, `machine_name` jetzt `pub(crate)`) —
  uncommitted im Baum.
* `docs/api/desktop/deskmain.md` — neuer Abschnitt „NewTOS implementation
  notes — `do_deskmenu`'s About box (DESK01)" (Baumtafel, die drei
  Abweichungen, die Baudatumsluecke, die Messwerte). Ausserdem ein
  **Erratum** im `do_optnmenu`-Absatz: dort stand `PREFITEM` → „`desk_all(FALSE)`
  unconditionally", die Quelle hat `if (inf_pref()) desk_all(FALSE);`
  (`deskmain.c:885-887`).
* `docs/api/desktop/_hw-test-2026-09-17.md` — Punkt **37**, Schritt 2.
* `docs/STATUS.md` — Phase-14-Zeile + *Recent Decisions* (2026-09-24).
* `measurement-2026-09-24-desk01-desk02/`.

## Offen

**Hardwareabnahme:** Pi 4, HW-Checkliste Punkt **37**, Schritt 2 — die
1920x1080-Optik des Kastens (gleiche Pixelgroesse, weiter mittig). Den
Vergleich gegen `rscview` hat dieses Ticket nicht gefahren: der Baum ist
**nicht** EmuTOS' `ADDINFO`, sondern eine eigene Fuellung derselben Form, ein
Bildvergleich waere also kein Soll.
