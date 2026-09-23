# DESK21 — STATUS: DONE (2026-09-23)

**Auftrag:** `Set background...` ans Ende des View-Menues verschieben,
Trennstrich davor wie im Baum; `shortcut_mapping` und `men_update`
entsprechend mitziehen. Screendump beider Menues vorher/nachher.

Basis: Branch `desk-rebuild`, HEAD `a8d4b42`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset`.

Messung: [`measurement-2026-09-23-desk16-desk17-desk21/`](../measurement-2026-09-23-desk16-desk17-desk21/).
Spezifikation: `docs/api/desktop/deskmain.md` (neuer Abschnitt
„`BACKGRND` belongs to *View*").

---

## 1. Die Analyse des Tickets — gegen die Quelle geprueft

**Richtig, und am Baum selbst nachgeprueft statt nur an der Nummernliste:**
Objekt 34 ist der View-Klappkasten mit `ob_head` 35 und `ob_tail` **46**
(`desk_rsc.c:446`), `SEP_VW2` (45) ist der Trennstrich und `BACKGRND` (46) das
letzte Kind (`:473-483`). Objekt 47 ist der Options-Kasten, sein erstes Kind
`IICNITEM` (48) — `desk_rsc.h:35-48`.

**Richtig:** die Kuerzelzuordnung ist semantisch. `shortcut_mapping[]` fuehrt
`BACKGRND` unter `/* 'View' menu */` als letzten Platz, hinter `FITITEM`
(`deskmain.c:211-217`) — NewTOS' `SHORTCUT_MAPPING` ebenso, seit DESK05.
**Also keine Aenderung dort.**

---

## 2. Was gebaut wurde

`kernel/src/desk/menu.rs`:

* `MENUS` — `Sep` + `Act(Item::SetBackground, …)` ans Ende des View-Eintrags,
  aus dem Kopf des Options-Eintrags entfernt. Die View-Beschriftungen sind
  auf die Breite des neuen laengsten Eintrags gepolstert (19 Zeichen), damit
  `adjust_menu`s Messung die richtige Kastenbreite ergibt.
* `Item::SetBackground` steht im Aufzaehlungstyp jetzt im View-Block; die
  Diskriminanten und `ALL_ITEMS` sind unveraendert (er stand ohnehin
  zwischen `SizeToFit` und `InstallIcon`).
* `item_title` nennt ihn `Title::View` — davon haengt die Verteilung ab.
* `item_enabled`: unveraendert schwarz, mit einer Zeile, die sagt warum.

`kernel/src/desk/main.rs`: der Arm ist von `do_optnmenu` nach `do_viewmenu`
gewandert, wie in EmuTOS (`deskmain.c:823-828`).

### Die Kastenbreite, und warum sie sich aendern **musste**

`copy_menu_items` polstert jeden Eintrag eines Menues auf
`max_strlen + 1 + SHORTCUT_SIZE` (`deskmain.c:1781`), `adjust_menu` nimmt die
gepolsterte Laenge und addiert eins (`:1608-1620`). Mit
`"  Set background..."` (19 Zeichen) darin ist der View-Kasten 19+1+3 = **23**
Zeichenzellen breit; vorher waren es 20. Der Lauf misst genau das:

    vorher   [probe] box 2 abs (112,19) 160x160   (10 Eintraege)
    nachher  [probe] box 2 abs (112,19) 184x192   (12 Eintraege)
    vorher   [probe] box 3 abs (160,19) 240x192   (12 Eintraege)
    nachher  [probe] box 3 abs (160,19) 240x176   (11 Eintraege)

### Genannte Abweichung — im Praedikat, nicht in der Einordnung

EmuTOS' Arm lautet `case BACKGRND: if (inf_backgrounds()) rc =
BACKGROUND_HAS_CHANGED;`. `inf_backgrounds` gibt **nur** dann TRUE zurueck,
wenn sich der **Fenster**hintergrund geaendert hat (`deskinf.c:910-916`); eine
Aenderung des *Schreibtisch*hintergrunds setzt es selbst um, zeichnet mit
`do_wredraw(DESKWH, &G.g_desk)` neu und gibt FALSE zurueck (`:900-908`).

NewTOS' `ADBKGND`-Ersatz bietet nur die Schreibtischhaelfte an — der
`#Q`-Fenstereimer wird durch die Datei gereicht, hat aber keinen Verbraucher
—, und sein `true` meint genau diese Aenderung, die `inf::set_background`
bereits auf demselben Weg neu gezeichnet hat. Das EmuTOS-gleiche `rc` ist fuer
jeden Ausgang dieses Kastens also `0`; `BACKGROUND_HAS_CHANGED` hat keine
NewTOS-Entsprechung zu setzen, und ein `desk_all` wuerde hier jedes Fenster
fuer eine Aenderung neu zeichnen, die keines betrifft.

Die Fensterhintergrund-Haelfte von `ADBKGND` bleibt die Luecke, die sie vorher
war — unveraendert durch dieses Ticket, aber hier benannt.

---

## 3. Belege

`measurement-2026-09-23-desk16-desk17-desk21/` — QEMU raspi4b, 640x480,
`-display none`, QMP-Screendump, scratch-Kopie mit dem H02-IKBD-Injektor,
Karte je Lauf frisch aus `tools/make-sd-image.sh`. Die beiden Titelpunkte
stammen aus dem seriellen Menue-Dump (`method/probe_patch.py`); der Zeiger
faehrt von unten an seinen eigenen Titel heran und kreuzt keinen anderen.

| | vorher | nachher |
|---|---|---|
| View | `before-b-view.png` endet bei `Size to fit` | `after-b-view.png` + Trennstrich + `Set background...` |
| Options | `before-c-options.png` beginnt mit `Set background...` | `after-c-options.png` beginnt mit `Install icon...` |

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

* Pi-4-Punkt **47** in `docs/api/desktop/_hw-test-2026-09-17.md`: der breitere
  Kasten in 1920x1080 (er muss unter seinem Titel bleiben und darf nicht ueber
  den rechten Rand laufen), der Kasten selbst mit der echten Maus, und ein im
  Konfigurationsdialog vergebenes Kuerzel, das ueber einen Kaltstart an
  seinem `#K`-Platz bleibt.

## 5. Nebenbefunde (nicht angefasst)

* Die uebrigen drei Klappmenues sind weiter **eine Zelle breiter** als
  EmuTOS' Rechnung ergibt: ihre Beschriftungen in `MENUS` tragen ein
  Fuellzeichen mehr als der laengste Text, das `adjust_menu`s `maxlen`
  mitzaehlt (Desk 22 statt 21, File 23 statt 22, Options 30 = richtig). View
  ist mit diesem Ticket auf EmuTOS' 23 gekommen, weil der neue Eintrag
  ungepolstert der laengste ist. Rein kosmetisch, ausserhalb dieses Tickets —
  gehoert in ein eigenes.
