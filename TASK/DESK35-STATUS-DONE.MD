# DESK35 — STATUS: DONE (2026-09-24) — implementiert, Pi-Abnahme offen

**Auftrag:** Am Quelltext klaeren, was `wm_init` und `sh_draw` im
verschachtelten Lauf anrichten wuerden — DESK06s Begruendung fuers Auslassen
**pruefen, nicht uebernehmen** —, dann den Schirm vor dem Start raeumen und
nach der Rueckkehr wiederherstellen. **Nicht** der Desktop-Neustartpfad
(DESK14 §4).

Basis: Branch `desk-rebuild`, HEAD `a84b0ba`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen,
keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset`.

Messung:
[`measurement-2026-09-24-desk34-desk35/`](../measurement-2026-09-24-desk34-desk35/)
§3 und §4. Spezifikation: `docs/api/aes/shell-loop.md`, Abschnitt *the
`gl_shgem` block for a GEM program launched by the built-in desktop (DESK35)*.
Hardwarepunkt 60 in `docs/api/desktop/_hw-test-2026-09-17.md`.

---

## 1. DESK06s Begruendung — was sie wert war

Sie steht in `kernel/src/shell_loop.rs` bei `sh_run_pending_console` und
lautete sinngemaess: `wm_init` und `sh_draw` bereiteten Fenstertabelle und
Schirm fuer einen Desktop vor, der gleich **neu gebaut** wird; der Desktop,
der diesen Lauf angestossen hat, steht aber noch mit seinen registrierten
Fenstern da, also zerstoerte jeder der beiden genau den Zustand, den er gleich
zeichnet.

Am Quelltext geprueft — und die Begruendung greift zu kurz, weil sie die
eigentliche EmuTOS-Bedingung nicht nennt. Die drei Anweisungen stehen in einer
`if`:

```c
sh_chdef(psh);                  /* gemshlib.c:712 */
sh_chgrf(psh);                  /* :713 */
if (gl_shgem)                   /* :715 */
{
    wm_init();                  /* :717 */
    ratinit();                  /* :718 */
    sh_draw(D.s_cmd, TRUE);     /* :719 */
}
rc = sh_ldapp(psh);             /* :725 */
```

Der Konsolendurchgang hat `gl_shgem` in `sh_chgrf` (`:513-523`) gerade auf
FALSE gesetzt — **EmuTOS ueberspringt den Block dort selbst.** Es kann sich
das leisten, weil der Textschirm die ganze Anzeige abdeckt und die Fenster,
die der Desktop in der AES-Tabelle zuruecklaesst, der **naechste**
Grafikdurchgang abraeumt. DESK06s Auslassung war fuer die Konsole also nicht
nur vertretbar, sie war EmuTOS-konform — aber aus einem anderen Grund als dem
aufgeschriebenen.

Fuer eine GEM-Anwendung bleibt `sh_isgem`/`gl_shgem` TRUE, `sh_chgrf` tut
nichts, und EmuTOS nimmt alle drei Schritte. Es **muss** sie nehmen: die
Anwendung zeichnet nur in ihre eigenen Rechtecke. Die Begruendung ist im
Quelltext jetzt beim Namen genannt (an beiden Stellen), nicht vererbt.

## 2. Was `wm_init` und `sh_draw` hier anrichten

* `tos_api::aes::window::wm_init` = `wm_new` + Desktop-Fenster-Reset
  (`gemwmlib.c:1247-1310`): **jedes** Fensterhandle wird geschlossen und
  geloescht, `gl_newdesk` auf NULL, Handle 0 wieder ganzer Schirm. Die
  `WNODE`s des Desktops halten danach tote Handles — die AES-Tabelle traegt
  **nichts** von dem, was der zurueckkehrende Desktop erwartet.
* `sh_draw(D.s_cmd, TRUE)` malt nur: der ROOT des eingebauten DESKTOP-Baums
  bekommt kurz `CLEAR_SCREEN` und wird ueber den ganzen Schirm gezeichnet
  (`gemshlib.c:64,269-289`). Kein Zustand.

**Also springen `cnx_put`/`cnx_get` ein — und sie sind hier keine Alternative,
sondern die Voraussetzung.** Genau dieselbe Lage hat EmuTOS, und genau deshalb
oeffnet sein neugestartetes `deskmain` die Fenster aus dem Kontextspeicher
wieder (`cnx_get`, `deskmain.c:2055`), den der Epilog mit `cnx_put()`
(`:2154`) gefuellt hat. Beides gibt es seit DESK09; `desktop_relinquish` rief
`cnx_put()` schon, `desktop_resume` ruft jetzt `cnx_get()`. Luecke G2 ist zu.

## 3. Was gebaut wurde

`kernel/src/shell_loop.rs` — zwei Funktionen, ein Durchgang von `sh_main`:

* `sh_gem_enter()` = `if (gl_shgem) { wm_init(); ratinit(); sh_draw(…,TRUE); }`
  (`:715-720`);
* `sh_gem_leave()` = `if (gl_shgem) wm_new();` (`:668-669,676-677`).

`kernel/src/desk/main.rs`:

* `desktop_relinquish`: `cnx_put()` → `menu_bar(…,0)` →
  `wind_set(WF_NEWDESK, NULL)` → **`win_free_all()`** → **`sh_gem_enter()`** →
  `appl_exit()`;
* `desktop_resume`: **`sh_gem_leave()`** → `appl_init()` →
  **`desk_verify(DESKWH)`** → `wind_set(WF_NEWDESK, g_screen)` →
  **`cnx_get()`** → **`men_update()`** → `redraw_desktop()`;
* `win_free_all()` — die `WNODE`s freigeben, jede durch `win_free`, damit die
  Element-`OBJECT`s ueber `obj_wfree` (`deskwin.c:148`) in die Freikette
  zurueckgehen.

Kein Stub. Der Konsolenpfad (`.NCP`, `sh_alpha_enter`/`sh_alpha_leave`,
`sh_run_pending_console`) ist **unveraendert** — richtig so, siehe §1.

## 4. Abgrenzung eingehalten

Kein Desktop-Neustartpfad. `crate::desk::run` kehrt weiterhin nicht zurueck,
`desk::global::init` bleibt einmalig, der Desktop bleibt ein In-Kernel-Aufruf,
DESK14 §4 bleibt zurueckgestellt. Gebaut sind nur die zwei Schritte **vor**
dem Programmlauf, ihr Gegenstueck danach und der Teil des
`deskmain`-Prologs, den `cnx_get` braucht.

## 5. Benannte Abweichungen

1. **Reihenfolge: der Block laeuft *vor* `appl_exit()`, nicht danach.** EmuTOS
   hat Epilog (`deskmain.c:2166`) und dann `gemshlib.c:717`, weil der Desktop
   dort ein beendeter Prozess ist. Hier teilen sich Desktop und Kind einen
   AES-Deskriptor, und `wm_new`s `wm_close` verteilt `WM_REDRAW` an die
   Besitzer der darunter noch offenen Fenster (`gemwmlib.c:1216-1222,1453`) —
   das ist dieser Desktop. `ap_exit`s Queue-Leerung (`gemaplib.c:251-261`)
   haelt das vom Kind fern (A28) und kann das nur als letzter Schritt. Gleiche
   Schritte, gleicher Durchgang, Leerung zuletzt.
2. **`win_free_all()` hat kein EmuTOS-Gegenstueck.** Dort gehoert das
   `WNODE`-Feld dem Desktop*prozess*, der endet, und der neugestartete legt
   mit `win_start()` (`deskmain.c:2028`) ein frisches an. Unser Desktop nimmt
   in-place wieder auf, also wird das Feld von Hand geleert — ohne AES-Aufruf:
   geschlossen und geloescht werden die Fenster von `wm_init`, wie in EmuTOS.
3. **`sh_gem_leave` ist das schlichte `wm_new`**, nicht mehr
   `wm_new_keep_desktop`: nach `sh_gem_enter`s `wm_init` gibt es keine
   Desktop-Fenster mehr zu behalten.
4. `wind_update(BEG/END_UPDATE)` um `cnx_get` (`deskmain.c:2054,2056`) wird
   nicht genommen — der Desktop ist in diesem Moment der einzige lebende
   AES-Klient, und der anschliessende Ganzschirm-`redraw_desktop()` deckt ab,
   was die einzelnen `wind_open` gezeichnet haben.

## 6. Gemessen

QEMU `-M raspi4b`, 640×480, `-display none`, QMP-Screendumps. Zwei Fenster auf
`C:\` offen, dann gestartet.

| Bild | HEAD | jetzt |
|---|---|---|
| waehrend `GEMWIN.NAP` (`d35-*-D-app.png`) | beide Desktop-Fenster, alle Desktop-Symbole und der Papierkorb stehen; das Programmfenster ist **zwischen** sie komponiert | **nur** das Programmfenster auf leerem Schirm |
| waehrend QED (`d35q-*-E-qed.png`) | QEDs Menueleiste ueber dem eingefrorenen Desktop — genau das Pi-Bild des Tickets | QED besitzt den Schirm, kein Fensterrest |
| danach (`d35-*-E-back.png`) | Desktop mit beiden Fenstern (sie waren nie weg), Auswahl noch invertiert | Desktop mit **beiden** Fenstern an ihren alten Stellen und in alter Reihenfolge — aus `cnx_get`; ohne Auswahl, wie beim neugestarteten EmuTOS-Desktop |

0 `[EXC]`/`[PANIC]` in allen vier Laeufen; `fsck_msdos -n` sauber auf allen
Kartenabbildern (`build/fsck.txt`).

Die Messung braucht ein GEM-Programm, das nur sein eigenes Fenster malt
**und** sich selbst beendet. `DUMPVDI.NAP` faellt aus (jeder Fall beginnt mit
`v_clrwk`, das den ganzen Schirm raeumt — es haette die Reste auch im
HEAD-Stand weggewischt), `QED.NAP` faellt fuer die zweite Haelfte aus (es
endet ueber sein eigenes Menue). Also
`measurement-2026-09-24-desk34-desk35/method/gemwin.c`, 6,9 KB, im Messordner
und nicht im Produkt.

## 7. Was offen bleibt

* **Reale Pi-Gegenprobe:** dort wurde der Defekt gefunden. Pruefpunkt 60.
* **TeraDesk** ist nicht gefahren — `DESKTOP.NAP` beendet sich nicht von
  selbst, und ein Skript durch seine Menues gehoert nicht in diesen Lauf.
  Steht im Pruefpunkt.
* `tos_api::aes::window::wm_new_keep_desktop` hat damit **keinen**
  Produktivaufrufer mehr (nur noch `tos-api/tests/aes_a96_zombie_windows.rs`).
  Nicht angefasst — eigener Befund, siehe Bericht.
