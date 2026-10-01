# A48 — STATUS: DONE, Tier 1 (Hostbeleg rot/gruen + QEMU-Selbstbeleg vorher/nachher) / Tier 2 = A106 / Pi-Sichtabnahme offen

`shel_write(SHW_RESCHNG, isgem, isover, …)` zeichnet die Anforderung jetzt
genau wie EmuTOS `sh_write` auf (`gl_changerez = 1 + isover`, `gl_nextrez =
isgem`, unbedingt); der Shell-Loop ignoriert sie weiterhin, bis A106 den
re-enterbaren Wrapper um `sh_main` baut. Bearbeitet 2026-09-21, Branch
`desk-rebuild`, Basis HEAD `bd9cd19`, **nicht committet**.

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemshlib.c:92-99` (`gl_changerez`: 0 = keine Aenderung, 1 = ST-Familie, 2 = Falcon-Familie; `gl_nextrez`: Wert fuer `Setscreen`), `:187-192` (`case SHW_RESCHNG`: `gl_changerez = 1 + isover; gl_nextrez = isgem; D.s_cmd[0] = '\0'; ad_stail[0] = '\0';` — unbedingt, kein `SHELL`-Member), `:195` (`return TRUE`), `:727` (`while (… && !gl_changerez)` — Verbraucher, A106), `:178-186` + `bios/bios.c:754-770` (`SHW_SHUTDOWN` zeichnet nichts auf: `can_shutdown()` FALSE), `aes/geminit.c:343-392` (`process_inf1`: der einzige validierende Erzeuger, via `check_moderez`), `:806` (`gem_main` loescht — A106), `aes/gemstart.S:86-124` (`aes_restart` — A106), `desk/deskrez.c:213-215` (der „keine Aenderung“-Filter sitzt im Aufrufer) |
| NewTOS (vorher) | `tos-api/src/aes/shell.rs` `SHW_RESCHNG`-Arm: nur `s_cmd[0] = s_tail[0] = 0`; kein `gl_changerez`/`gl_nextrez` im Modul; `kernel/src/shell_loop.rs:69-71` Kommentar „`gl_changerez` permanent FALSE“ (richtig nur fuer `process_inf1`) |
| NewTOS (nachher) | `shell.rs`: `State.gl_changerez`/`gl_nextrez` (`i16`, init 0), Arm setzt beide unbedingt und leert wie bisher, `pub fn gl_changerez()`/`gl_nextrez()`; `shell_loop.rs`: Kommentar benennt beide Erzeuger und den A106-Wrapper; Code der Schleife unveraendert |

## Was gebaut ist

* `tos-api/src/aes/shell.rs`
  * `State` erhaelt `gl_changerez: UnsafeCell<i16>` und `gl_nextrez:
    UnsafeCell<i16>` (Init 0) mit Kommentar zu Bedeutung, Erzeuger und
    dem fehlenden Verbraucher (A106).
  * `shel_write`, Arm `SHW_RESCHNG`: `*gl_changerez = 1i16.wrapping_add(isover)`
    (die C-WORD-Truncation von `1 + isover`), `*gl_nextrez = isgem`, danach
    `s_cmd[0] = 0; s_tail[0] = 0` wie bisher. Keine Validierung, kein
    Vergleich, keine `SHELL`-Member — `gemshlib.c:187-192`.
  * Accessoren `gl_changerez() -> i16`, `gl_nextrez() -> i16` neben
    `sh_doexec()`; benannt nach den EmuTOS-Globals wie das vorhandene
    `gl_shgem()`. Kein Setter/Clear (das Loeschen ist `gem_main`s,
    `geminit.c:806`, A106).
  * Doc-Kommentar des `SHW_RESCHNG`-Bullets und der Konstante angepasst.
* `kernel/src/shell_loop.rs` (nur Kommentar): die Exit-Bedingung laesst den
  `!gl_changerez`-Term bewusst weg; `process_inf1`s Erzeuger ist auf dem
  Board mit genau einem Modus legitim FALSE (ADR-017 rev 2, D-101),
  `sh_write`s Erzeuger wird aufgezeichnet und bis A106 ignoriert — der Term
  ohne Wrapper wuerde `sh_main` in `kmain`s `halt()` zurueckkehren lassen.
* `tos-api/tests/aes_shel_write_reschng.rs` (neu, A61/H05-Harness):
  ST/TT-Familie (`(5, 6, 0)` → 1 / 1 / 6, Puffer leer, `SHELL` unberuehrt),
  Falcon-Familie unvalidiert (`(5, 3, 1)` → 2/3, `(5, 0x7ABC, 1)` →
  2/0x7ABC, `(5, 2, 0)` → 1/2), `SHW_SHUTDOWN`-Guard (`(4, 0, 2)` → 1,
  Anforderung und `sh_doexec` unveraendert).

## Vorgabe des Leads, an der Quelle geprueft

* „`shel_write(SHW_RESCHNG, 3, 1)` → `changerez() == 1`“: mit `isover = 1`
  ist `gl_changerez = 2` (`gemshlib.c:188`). Die Tests pruefen `1` fuer
  `isover = 0` und `2` fuer `isover = 1` (Design § 9).
* „`s_cmd`/`s_tail` nur leeren, wenn EmuTOS das in diesem Arm tut“: tut es
  (`:190-191`) — beibehalten. `sh_doexec`/`sh_nextapp`/`sh_isgem` ruehrt der
  Arm nicht an — Test haelt das fest.
* Accessor-Namen: `gl_changerez()`/`gl_nextrez()` statt
  `changerez()`/`nextrez()` — Modulkonvention (`gl_shgem()`) und die
  A106-Skizze im Design (§ 5) verwenden den `gl_`-Namen.

## Hosttests (`measurements/measurement-2026-09-21-a48/host-tests/`)

| Datei | Ergebnis |
|---|---|
| `aes_shel_write_reschng-RED-vs-HEAD-bd9cd19-shell.rs.txt` | E0432: `gl_changerez`/`gl_nextrez` existieren in HEADs `shell.rs` nicht |
| `aes_shel_write_reschng-RED-state-present-arm-unchanged.txt` | Zustand + Accessoren da, Arm noch HEAD: 0 / 3 rot (`gl_changerez` 0 statt 1 bzw. 2) |
| `aes_shel_write_reschng-GREEN-after.txt` | 3 / 3 |
| `aes_shel_siblings-GREEN-after.txt` | `aes_shel_write_iscr` 3/3, `aes_shel_roundtrip` 2/2 |
| `xtask-test-AFTER.log` | `cargo run -p xtask -- test`: 794 / 0 / 1 → **797 / 0 / 1**, rc 0 |

## Build / Clippy (`build/`)

`cargo xtask build --board rpi4`: tos-api 64 / kernel 94 (Baseline 64/94,
Delta 0), rc 0; Baum-`kernel8.img` `a89b1bc9…`. `cargo clippy -p tos-api`:
80 (Baseline 80, Delta 0); `shell.rs` 15 → 15 Treffer (alle vorbestehend:
`Aespb`-Sichtbarkeit + vier Loop-Index-Hinweise auf unveraenderten Zeilen),
`shell_loop.rs` 0, neue Testdatei 0. `cargo clippy --package kernel
--no-default-features --features board-rpi4`: tos-api 80 / kernel 125, keine
Treffer auf neuen Zeilen. Kein qemu-virt-Build (Lead).

## QEMU raspi4b (`screendumps/`, `serial/`, `method/`)

Injektor-Scratchkerne (A87-Methode ohne `Step::Key`; vorher = `shell.rs` +
`shell_loop.rs` von HEAD via `git show`, nachher = Baum + eine
Scratch-only-Trace-Zeile `[a48] after shel_write: gl_changerez=… gl_nextrez=…`
nach jedem `op 121`). TeraDesk per `#Z` (`sd-tera.img`), Skript
`method/tera-a48.actions`, Koordinaten aus `desktop.rsc` (`method/rscdump.py`)
plus zwei Probelaeufen (`screendumps/probe/`): parkbr; A; Options hovern
(260,8); „Video options…“ klicken (300,170); C (Dialog, Wurzel (164,113));
ST-Low (247,185); D; OK (347,361); E (ARESCH-Alert); Alert-„OK“ (280,276);
F (+0,5 s); parkbr; G (+4,5 s); H (+8 s).

* Serial, **beide** Kerne: `[aes] pid=0 op 121 in=(5,2,0,0,0,0)
  out=(1,0,0,0,0,0,0)` = `shel_write(SHW_RESCHNG, ST_LOWRES + 2, 0)` → 1
  (`video.c:363-365,378-379`); dann `op 25 in=(48,…) out=(32,…)`
  (`wait_to_quit`: `evnt_multi(MU_MESAG|MU_TIMER, 2000 ms)` → Timer, kein
  `AP_TERM`), `op 24 in=(500,…)`, `op 19` (`appl_exit`), `[launch] returned
  (program Pterm'd)`, `[shell] sh_ldapp: … exited with rc=0`, `[shell]
  sh_ldapp: starting NEWDESK, sh_nextapp=3, sh_isgem=1`, `[desk] desktop up`.
* **Nachher zusaetzlich:** `[a48] after shel_write: gl_changerez=1
  gl_nextrez=2 sh_doexec=1` — die einzige Zeile im Serial-Diff der
  `[aes]/[a48]/[shell]/[launch]/[desk]`-Zeilen.
* Screendumps A, C, D, E, F, G, H: 7/7 Paare **0 px** Unterschied
  vorher/nachher; G == H = der Kernel-Desktop (eigene Leiste
  Desk/File/View/Options, `C:\`-Fenster) 4,5 s nach dem Alert-Klick; F =
  TeraDesks eigener Abbau (Leiste aus, `rsrc_free`) waehrend
  `wait_to_quit`. 0 `EXC`/`PANIC` in allen vier Serial-Logs.
* Tier 1 ist reiner Zustand: derselbe Ausgang wie vorher, wie geplant. Der
  Loop laeuft den naechsten Eintrag (Desktop) wie nach jedem Programmende.

## Nicht angefasst (Nebenbefunde)

* Design § 3 („`voptions()` aktiviert alle Radiobuttons“): TeraDesks
  `static long vdo` (`video.c:60`) bleibt ohne `_VDO`-Cookie 0 →
  `vdohi = ST_VIDEO` → `vd_enable_rez(VSTMED)` (`:494-507`): nur
  ST-Low/ST-Medium aktiv, TT-Medium vorselektiert-disabled. Fuer den Lauf
  unerheblich (ST-Low ist aktiv); der Kommentar „0xffffffff without cookie“
  in `video.c` ist veraltet, nicht der Initialwert.
* `docs/api/aes/scrap-shell.md` / `shell-loop.md` (uncommitted
  Researcher-Stand im Baum) sagen weiter „not yet implemented“ / „records
  no restart intent at all“ — nach Tier 1 nachzuziehen (nicht im
  Aenderungsumfang dieses Auftrags).
* Die `while`-Bedingung steht im gepinnten Baum bei `gemshlib.c:727`
  (Design/Spec zitieren `:730`).

## Offen

Tier 2 ([A106](A106.md)); Pi-Sichtabnahme (Tester/Lead; nichts geflasht).
