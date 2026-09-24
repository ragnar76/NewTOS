# DESK29 — STATUS: DONE (2026-09-24) — Praemisse zurueckgezogen, kein Patch

**Auftrag:** An der Quelle klaeren, wann EmuTOS die Auswahl beim Toppen
loescht, und ob unser unbedingtes Loeschen eine Vereinfachung ist; dann die
Bedingung angleichen.

**Ergebnis: es ist keine Vereinfachung.** EmuTOS loescht unbedingt, an zwei
Stellen, und unsere beiden Stellen sind wortgleich. Kein Code geaendert.

Basis: Branch `desk-rebuild`, HEAD `3f6b6ef`. Nichts geflasht, nichts
ausgeworfen, keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset`.

Messung:
[`measurement-2026-09-24-desk22-desk23-desk28-desk29/`](../measurement-2026-09-24-desk22-desk23-desk28-desk29/)
§5. Spezifikation: `docs/api/desktop/deskmain.md`, Abschnitt *a selection does
not survive a window change (DESK29)*.

---

## 1. Die Quelle — zwei Stellen, beide unbedingt

```c
/* deskmain.c:1282-1292 */
case WM_TOPPED:
    desk_clear(G.g_cwin);
    pw = win_find(handle);
    if (pw) { wind_set(handle, WF_TOP, 0,0,0,0); win_top(pw);
              desk_verify(pw->w_id, FALSE); change = TRUE; }
    break;

/* deskmain.c:957-959 */
wh = wind_find(mx, my);
if (wh != G.g_cwin)
    desk_clear_all();
```

`G.g_cwin` ist im `WM_TOPPED`-Arm noch das **alte** Fenster: gesetzt wird es
erst in `desk_verify` (`G.g_cwin = wh;`, `desksupp.c:196`), also nach dem
Loeschen. Und `hndl_button` geht sogar weiter: jeder Klick, dessen Fenster
nicht das aktuelle ist, loescht mit `desk_clear_all` (`desksupp.c:160-169`)
die Auswahl **aller** Fenster und die des Desktops.

`kernel/src/desk/main.rs::top_msg` (:1956-1966) und `::hndl_button`s
`desk_clear_all()` (:152) sind beides wortgleich — inklusive der Reihenfolge.

## 2. Die Begruendung des Tickets, widerlegt

Das Ticket nahm an, EmuTOS' `do_aopen` setze voraus, „dass die Auswahl aus
einem anderen Fenster stammen kann". Das tut es nicht:

```c
/* desksupp.c:896-902 */
if (!G.g_appdir && pw)      /* pw = win_ontop() (:878) */
    p = pw->w_pnode.p_spec;
else
    p = pathname;
```

Die zweite Stellung von *Default dir* nimmt das Verzeichnis des **obersten
Fensters** — unabhaengig davon, wo die Auswahl liegt. Erreichbar ist der Fall
immer dann, wenn das geoeffnete Objekt **nicht** im obersten Fenster liegt:

* ein **Desktop**-Icon bei offenem Fenster — `do_open`s erster Zweig
  (`G.g_cwin == DESKWH`, `desksupp.c:1205`), `win_ontop()` liefert trotzdem
  das Fenster;
* ein **Ziehen**, dessen Ziel ein anderes Fenster oder ein Desktop-Icon ist
  (`fun_drag` → `do_aopen`, `deskfun.c:967, 1034, 1249`).

Die DESK18-Notiz „unerreichbar" war eine Fehllesung und ist damit
richtiggestellt.

## 3. Was gemessen wurde

Zwei Fenster auf `C:\` (die QEMU-Karte hat eine Partition; das zweite Fenster
kommt aus einem zweiten Doppelklick auf dasselbe Laufwerkssymbol —
`do_dopen` ruft `win_alloc` unbedingt, `desksupp.c:1041-1063`):

| Bild | Zustand |
|---|---|
| `d29-C-win2.png` | zwei Fenster, Fenster 2 oben |
| `d29-D-selected.png` | `README.TXT` in Fenster 2 ausgewaehlt, Infozeile „64 bytes used in 1 selected items." |
| `d29-E-after-top.png` | ein Klick in Fenster 1s unverdeckte Arbeitsflaeche (60,300) — Fenster 1 oben |
| `d29-F-win2-back.png` | Fenster 2 wieder oben: **keine** Auswahl, „424568 bytes used in 28 items." |

Die AES-Spur zwischen `MARK D_SELECTED` und `MARK E_AFTER_TOP`
(`serial/d29-serial.log`) zeigt die EmuTOS-Reihenfolge:

    [aes] op 105 in=(2,3,…)    wind_set(2, WF_INFO)   <- desk_clear -> win_sinfo
    [aes] op 105 in=(1,10,…)   wind_set(1, WF_TOP)    <- top_msg

also `desk_clear` auf dem **alten** Fenster vor `WF_TOP` auf dem neuen.
0 `[EXC]`/`[PANIC]`.

## 4. Pruefung

Kein Code geaendert; die Bau- und Testwerte dieses Stapels stehen in der
DESK28-Quittung §4.

## 5. Offen

Hardwarebestaetigung: Punkt **58** in
`docs/api/desktop/_hw-test-2026-09-17.md` — dort mit zwei Fenstern auf
**verschiedenen** Laufwerken (C-F), was die QEMU-Karte nicht hergibt, und mit
dem Desktop-Icon-Fall aus §2.

## 6. Was in ein neues Ticket gehoert

Der Desktop-Icon-Fall aus §2 — „Default dir = top window, Programm vom
Desktop gestartet, Verzeichnis des obersten Fensters" — ist mit DESK18
gebaut, aber nie **gefahren** worden, weil DESK18 ihn fuer unerreichbar
hielt. Er braucht ein Desktop-Icon, das eine Datei ist, also die
Desktop-Verknuepfungen (`CONF_WITH_DESKTOP_SHORTCUTS`) — und die sind bereits
als **DESK30** erfasst (`kernel/src/desk/ins.rs`, Kopfkommentar „What is not
here, and why"). Als Abnahmefall dort nachzutragen, kein neues Ticket.
