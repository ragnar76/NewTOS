# DESK17 — STATUS: DONE (2026-09-23)

**Auftrag:** (1) `men_update` fehlt EmuTOS' `ntrash` — *Delete* muss gesperrt
sein, solange ein Papierkorb- oder Druckersymbol in der Auswahl ist
(`deskmain.c:533-535`). (2) Der Menuetitel bleibt invertiert, solange ein
Dialog offen ist; **zuerst gegen `hndl_menu` belegen**, wo EmuTOS den Titel
freigibt, dann nachziehen.

Basis: Branch `desk-rebuild`, HEAD `a8d4b42`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset`.

Messung: [`measurement-2026-09-23-desk16-desk17-desk21/`](../measurement-2026-09-23-desk16-desk17-desk21/).
Spezifikation: `docs/api/desktop/deskmain.md` (neuer Abschnitt, beide
Haelften).

---

## 1. Teil 1 — `ntrash`: die Analyse des Tickets stimmt, gebaut

`men_update` zaehlt in **einem** Durchlauf ueber die Auswahl vier Dinge
(`deskmain.c:496-521`), und der `AT_ISPRNT`/`AT_ISTRSH`-Zweig faellt in
`AT_ISDISK` **durch** (`:508-517`): ein Papierkorb und ein Drucker erhoehen
`ntrash` **und** `ndesk`, ein Laufwerk nur `ndesk`. `ntrash` sperrt danach
genau einen Eintrag:

    /* disable "Delete" iff either trash or printer is selected */
    if (ntrash)
        menu_ienable(tree, DELTITEM, FALSE);        /* deskmain.c:533-535 */

Gebaut, in demselben Durchlauf wie vom Ticket verlangt:

* `kernel/src/desk/win.rs` — `count_selected_desk_icons` gibt jetzt
  `(ndesk, ntrash)` zurueck; die Typverzweigung bildet EmuTOS' Durchfall ab.
* `kernel/src/desk/menu.rs` — `men_update` reicht beide Zahlen weiter,
  `item_enabled` schaltet `Item::Delete` auf `nsel >= 1 && ntrash == 0`, also
  die `ILL_NOSEL`-Mitgliedschaft plus die eine zusaetzliche Sperre.

---

## 2. Teil 2 — der Menuetitel: die Annahme des Tickets ist widerlegt

Das Ticket erwartete, dass EmuTOS den Titel **vor** dem Dialog freigibt. Es
tut das nicht. `hndl_menu` ruft erst den Bearbeiter, in dem der Dialog laeuft,
und gibt danach frei:

    static WORD hndl_menu(WORD title, WORD item)
    {
        ...
        case OPTNMENU:
            done = do_optnmenu(item);     /* hier steht der Dialog */
            break;
        }
        menu_tnormal(desk_rs_trees[ADMENU], title, 1);   /* deskmain.c:1036 */
        return done;
    }

Die Freigabe steht in `:1036`, **hinter** dem `switch` (`deskmain.c:1012-1038`).
Das ist kein Versehen, sondern der GEM-Vertrag: die AES laesst den Titel
invertiert, wenn sie `MN_SELECTED` schickt, und die Anwendung nimmt ihn
zurueck, wenn sie mit der Auswahl fertig ist — genau deshalb darf dazwischen
ein modaler Dialog stehen. Der Tastenweg macht das Spiegelbild und hebt
**zuerst** hervor (`menu_tnormal(..., title, 0)`, `:1240`).

`kernel/src/desk/main.rs` tut bereits beides an genau denselben Stellen:
`hndl_menu` gibt nach dem `do_*menu`-Aufruf frei, `hndl_kbd` hebt auf dem
Kuerzelweg vorher hervor.

**Also keine Codeaenderung.** Der im Ticket als Defekt zitierte Screendump
(`measurement-2026-09-24-desk03-desk04/screendumps/ico-B-dialog.png`) zeigt
konformes Verhalten. Trotzdem vorher **und** nachher neu gemessen, mit
offenem Kasten und nach dem Schliessen, damit der Befund belegt auf dem Tisch
liegt statt nur behauptet.

---

## 3. Belege

`measurement-2026-09-23-desk16-desk17-desk21/` — QEMU raspi4b, 640x480,
`-display none`, QMP-Screendump, scratch-Kopie mit dem H02-IKBD-Injektor,
Karte je Lauf frisch aus `tools/make-sd-image.sh`. Jeder Klickpunkt stammt aus
einem seriellen Dump (`method/probe_patch.py`).

*Delete*:

| Auswahl | vorher | nachher |
|---|---|---|
| Papierkorb | `before-e-file-trash.png` **schwarz** | `after-h-file-trash.png` **grau** |
| Laufwerk `C:` | — | `after-f-file-drive.png` **schwarz** |

*Open* und *Info/Rename...* sind in beiden Nachher-Bildern schwarz — die neue
Sperre ist nicht in `ILL_NOSEL` ausgelaufen. Dieselben Zustaende stehen als
`dis=` auf der `Delete...`-Zeile in den seriellen Protokollen:

    vorher   [probe]   1.11 obj 23 '  Delete...         ^D' ... dis=0
    nachher  [probe]   1.11 obj 23 '  Delete...         ^D' ... dis=1

Menuetitel: `before-f-dialog-title.png` / `after-i-dialog-title.png` zeigen
*Options* invertiert, waehrend `INSTALL DESKTOP ICON` steht;
`before-g-title-after.png` / `after-j-title-after.png` zeigen ihn nach
`Cancel` wieder normal. Vorher und nachher gleich — das ist die Aussage.

`[EXC]`/`[PANIC]`: 0 in allen elf raspi4b-Protokollen und im
qemu-virt-Bootprotokoll. `fsck_msdos -n` auf jeder geschriebenen Karte:
Phase 1/2/3, keine Fehler. Kein `*.img` im Messverzeichnis.

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

* Pi-4-Punkt **46** in `docs/api/desktop/_hw-test-2026-09-17.md`: die
  Graustufe in 1920x1080 mit der echten Maus, die **gemischte** Auswahl
  (Papierkorb + Laufwerk zusammen), `Ctrl-D` auf einem gesperrten Eintrag, und
  der Titel auf dem Kuerzelweg.

## 5. Nebenbefunde (nicht angefasst)

* Keine.
