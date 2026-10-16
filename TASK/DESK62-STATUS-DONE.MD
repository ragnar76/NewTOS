# DESK62 — STATUS: DONE (2026-10-16) — Drop-Start verlaesst den Desktop wieder

**Auftrag:** Nach dem Drop einer Datei auf eine Desktop-Verknuepfung von `QED.NAP` bleibt die Sanduhr; QED startet nicht
(Anwender, echte Pi 4, 2026-10-16). Ursache mit Beleg, dann Fix (`TASK/DESK62.md`).

## Ursache (`c5018c19`)

| | EmuTOS | NewTOS vorher |
|---|---|---|
| Sanduhr an, `shel_write`, nur bei Fehler aus | `deskpro.c:35-45` | `pro.rs` `pro_exec` — gleich |
| Pfeil beim Desktop-Start | `deskmain.c:2074` | `lib.rs` — gleich |
| Datei auf Programmverknuepfung | `deskfun.c:1034` `exit_desktop = do_aopen(...)` | `dir.rs` `fun_file2desk`: Antwort verworfen |
| Desktop-Symbol auf Programm im Fenster | `deskfun.c:1249` | `dir.rs` `fun_desk2win`: Antwort verworfen |
| `fun_drag` | `:1304` loeschen, `:1338` `return exit_desktop` | `false` ausser `fun_win2win` |

Folge: Desktop bleibt in `evnt_multi`, die AES haelt den Start nur vorgemerkt, das `desk_busy_off` des naechsten Desktop-Starts kommt nie.
Fehler im Desktop, nicht in der AES. Analyse: `docs/analysis/desk62-hourglass-after-drop-on-shortcut-2026-10-16.md`.

## Aenderung (`a8b03c81`)

* `programs/desktop/src/dir.rs`: `static EXIT_DESKTOP: AtomicBool` (`deskfun.c:61`); `fun_drag` loescht/gibt zurueck (`:1304,1338`);
  `fun_win2win` (jetzt ohne Rueckgabe, wie `static void`), `fun_file2desk` `AT_ISFILE` und `fun_desk2win` setzen ihn aus
  `do_aopen_drop` (`:967,1034,1249`); `fun_file2desk` gibt fuer den Start weiter `false` (`:1080-1083`). Doc-Kommentare nachgezogen.
* `docs/api/desktop/deskfun.md`: veraltete Abweichung „gibt `false` zurueck" gestrichen.

## Belege (`measurements/measurement-2026-10-16-desk62-fix/RUNINFO.TXT`)

* Drop auf Verknuepfung: HEAD rot (Sanduhr, kein `appl_exit`), Fix gruen (`appl_exit` → QED mit `C:\README.TXT` → nach Ctrl-Q Pfeil).
* Desktop-Verknuepfung auf `QED.NAP` im Fenster (`fun_desk2win`): HEAD rot, Fix gruen.
* Datei auf `QED.NAP` im Fenster (Kontrolle): unveraendert gruen.
* Basislinien: Hosttests 1374/0/1, SDK 20/0/0, Bau 64/94 beide Boards, Clippy 76/125, Programme-Clippy 0, Driftprobe 0.

## Offen

* Abnahme auf echter Pi 4.
* `fun_desk2desk` auf eine Programmverknuepfung (EmuTOS ueber `fun_file2any`, `deskfun.c:1297`) fehlt — eigenes Ticket (Lead).
