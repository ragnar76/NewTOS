# DESK26 — STATUS: DONE (2026-09-23)

**Auftrag:** An der Quelle klaeren, ob `fun_rebld` eine sechste
`win_sinfo`-Aufrufstelle ist oder ob eine der fuenf aus A110 mit dem falschen
Argument gerufen wird — **vor** jeder Aenderung —, dann die fehlende
Aktualisierung nachziehen.

Basis: Branch `desk-rebuild`, HEAD `50714b7`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset` auf dem gemeinsamen
Baum.

Messung: [`measurement-2026-09-23-desk24-desk25-desk26/`](../measurement-2026-09-23-desk24-desk25-desk26/) §4.
Spezifikation: `docs/api/desktop/deskwin.md`, `docs/api/desktop/deskfun.md`.

---

## 1. Die Klaerung an der Quelle — es ist der erste Fall

`fun_rebld` ruft `win_sinfo` **nicht selbst**; es ruft `rebuild_window`, und
das ist die Stelle:

```c
static void rebuild_window(WNODE *pwin)          /* deskfun.c:135-142 */
{
    pn_active(&pwin->w_pnode, TRUE);
    desk_verify(pwin->w_id, TRUE);
    win_sinfo(pwin, FALSE);
    wind_get_grect(pwin->w_id, WF_WXYWH, &gr);
    fun_msg(WM_REDRAW, pwin->w_id, gr.g_x, gr.g_y, gr.g_w, gr.g_h);
}
```

`rebuild_window` ist das, was `fun_rebld` (`:173-190`) und
`fun_rebld_marked` (`:150-167`) fahren. EmuTOS hat damit **sechs**
`win_sinfo`-Aufrufstellen, vier mit `TRUE` und zwei mit `FALSE`
(`deskmain.c:983`, `deskfun.c:141`, `:381`, `:628`, `desksupp.c:151`,
`:450`) — nachgezaehlt mit `/usr/bin/grep -rn win_sinfo references/emutos/desk/`.

A110 hat vier `TRUE`-Stellen und **eine** der beiden `FALSE`-Stellen gebaut.
Die Notiz in `docs/api/desktop/deskwin.md` behauptete, die `FALSE`-Variante
sitze in `set_window_strings` und decke „`do_diropen`/`rebuild_window`" ab —
das stimmt nur fuer `do_diropen` (`desksupp.c:450`). `set_window_strings` hat
in `rebuild_window` keinen Aufrufer; NewTOS' `rebuild_window`
(`kernel/src/desk/dir.rs`) las neu ein, baute die Ansicht neu und zeichnete
neu, ohne die Infozeile anzufassen.

Damit war das Ticket bestaetigt, **nicht** die zweite Lesart („eine der fuenf
mit falschem Argument"). Keine der fuenf wurde angefasst.

Warum `FALSE` richtig ist: das Neueinlesen wirft die alten `FNODE`s weg, die
Auswahl ist danach leer. `FALSE` heisst „zaehle gar nicht erst, nimm die
Summen des Verzeichnisses" — genau das, was statt des stehengebliebenen
`… in 1 selected items.` hin soll. (`TRUE` haette hier dasselbe Ergebnis,
weil `select_count == 0` ohnehin auf die Summen zurueckfaellt; gebaut ist
EmuTOS' Argument.)

---

## 2. Was gebaut wurde

Eine Zeile plus Begruendung in `kernel/src/desk/dir.rs`, in `rebuild_window`,
nach dem Neueinlesen und vor dem Neuzeichnen:

    // `win_sinfo(pwin, FALSE)` (`deskfun.c:141`), after the re-scan and before
    // the redraw.
    super::main::win_sinfo(handle, false);

Dazu die Berichtigung der A110-Notiz in `docs/api/desktop/deskwin.md`: die
Aufrufstellen stehen jetzt als zwei Tabellen (vier `TRUE`, zwei `FALSE`) da,
mit dem Hinweis, was an der alten Aussage falsch war.

---

## 3. Die Nachweise

Eine Datei aus dem Fenster `C:\DESKT24\` geloescht (`File → Delete...`, `OK`),
derselbe Ablauf auf beiden Kernen:

    vorher   [d26] win_sinfo(h=2, check_selected=true)  -> " 5 bytes used in 1 selected items."
             (danach nichts)
             d26-before-e-deleted.png:  " 5 bytes used in 1 selected items."

    nachher  [d26] win_sinfo(h=2, check_selected=true)  -> " 5 bytes used in 1 selected items."
             [d26] win_sinfo(h=2, check_selected=false) -> " 5 bytes used in 2 items."
             d26-after-e-deleted.png:   " 5 bytes used in 2 items."

Die beiden Bilder unterscheiden sich **nur** in dieser Zeile; der
Fensterinhalt (FOLD, ONE.TXT, nichts ausgewaehlt) ist derselbe. `5 bytes /
2 items` stimmt: FOLD (0) plus ONE.TXT (5).

`[EXC]`/`[PANIC]`: 0. `fsck_msdos -n` auf beiden Karten: Phase 1/2/3, rc=0.

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

* Pi-4-Punkt **50** in `docs/api/desktop/_hw-test-2026-09-17.md`: dieselbe
  Zeile nach einem Kopieren in das Fenster und nach *New folder...*, und die
  Laenge der Zeile in 1920x1080.

## 5. Nebenbefunde (nicht angefasst)

* **`desk_verify` fehlt in `rebuild_window`.** EmuTOS' zweiter Schritt
  (`deskfun.c:140`) hat in NewTOS keine Entsprechung; das Neuaufbauen der
  Ansicht laeuft ueber `win_bldview`. Nicht in diesem Ticket geprueft, ob das
  beobachtbar ist.
* **`fun_rebld_marked` gibt es nicht.** EmuTOS markiert Fenster mit
  `WN_REBUILD` und baut sie am Ende der Ereignisschleife auf
  (`deskmain.c:678`, `deskfun.c:150-167`); NewTOS baut sofort. Beruehrt die
  vorliegende Sache nicht — die `win_sinfo`-Zeile sitzt in `rebuild_window`
  und gilt fuer beide Wege.
