# H03 — STATUS: DONE (Ursache festgestellt, kein Fix)

TeraDesk hängt nicht *nach* dem Beenden, sondern **vor** dem Beenden: im
`wait(150)` des Menüpunkts `Quit/Shutdown`, das durch die gemlib-Bindung
`mt_evnt_timer` mit **vertauschten 16-Bit-Hälften** bei der AES ankommt und
deshalb **2 h 44 min** statt 150 ms wartet. Der Quit-Dialog erscheint nie, `quit`
wird nie gesetzt, kein `appl_exit`, kein `Pterm`, kein Rückweg zu `desk/`. Die
Maus bewegt sich, weil `evnt_timer` in der AES-Warteschleife (`sched::sleep` →
`pump_input`) sitzt. Bearbeitet 2026-09-17, Branch `desk-rebuild`, Basis HEAD
`49b2c2c`. Messlauf: [`measurement-2026-09-17-h03/`](../measurement-2026-09-17-h03/RUNINFO.TXT).
**Kein Produktcode geändert.**

## 1. Welchen Beendigungsweg TeraDesk nimmt

Single-TOS-Bau (`-D_TOS_ONLY` → `_MINT_ = 0`, `sdk/apps/build-teradesk.sh`), also
ohne alle `#if _MINT_`-Zweige.

| Schritt | `sdk/apps/teradesk/…` |
|---|---|
| Menüklick → `hndlmenu(title, MQUIT, kstate)` | `main.c:1317` |
| `bell()` = `Bconout(2, 7)` | `main.c:1320`, `library/utility/other.c:49-52` |
| **`wait(150)` = `evnt_timer(150)`** | `main.c:1322`, `events.c:100-103` |
| `bell()` | `main.c:1323` |
| `chk_xd_dialog(quitopt, ROOT)` — Dialog *Shutdown / Reboot / Quit / Cancel* | `main.c:1324` |
| `QUITQUIT` → `quit = TRUE` | `main.c:1340-1342` |
| `menu_tnormal(menu, title, 1)` | `main.c:1384` |
| `evntloop()` endet (`while (!quit)`) | `main.c:1592` |
| Epilog: `va_checkclient`, `va_delall`, ggf. `save_options` (hier `save=0x0000` → nein), `wd_del_all` (`wind_close`), `menu_bar(menu, 0)`, `free_icons`, `regen_desktop(NULL)` = `wind_set(0, WF_NEWDESK, NULL)` + `form_dial(FMD_START/FINISH)`, `Mfree(global_memory)`, `vq_gdos`, `exit_xdialog` (`wind_delete`, `v_clsvwk`), `rsrc_free` | `main.c:1943-1992`, `icon.c:2509-2518`, `icon.c:242-252`, `library/xdialog/xdialog.c:2498-2526` |
| `appl_exit()` | `main.c:2124` |
| `return 0` → crt0 → `Pterm(0)` | `sdk/libc/crt0.S:59-63` |

**Kein `shel_write` auf diesem Weg.** `shel_write(SHW_INFRECGN)` (`main.c:1874`)
läuft nur bei `aes_version >= 0x399`; unsere `_AESversion` ist 832 = 0x340, also
auch das nicht. `shel_write(SHW_SHUTDOWN)` gehört allein zum `QUITSHUT`-Zweig
(`main.c:2048`), der hier ausgeschlossen ist.

## 2. Was NewTOS auf diesem Weg tut — bis zu dem Punkt, wo nichts mehr übernimmt

1. `evnt_timer(150)` → gemlib `mt_evnt_timer(unsigned long interval = 150)`,
   `sdk/gemlib/gemlib-0.44.0/a_evnt_timer.c:24-34`:
   ```c
   unsigned short *i = (unsigned short *)&interval;
   aes_intin[0] = i[1];
   aes_intin[1] = i[0];
   ```
   Auf m68k (`long` 32 Bit, Big-Endian) ist `i[0]` das **High**-Wort und `i[1]` das
   **Low**-Wort → `int_in[0] = lo, int_in[1] = hi`, korrekt. Auf AArch64 LP64
   (`long` 64 Bit, Little-Endian) ist `i[0]` das **Low**-Wort und `i[1]` Bits 16–31
   → `int_in[0] = 0, int_in[1] = 150`. Belegt am ausgelieferten Binärcode der
   Karte (`/Volumes/BOOT/Apps/TERADESK/DESKTOP.NAP`, sha256 `d957d765…` =
   `sdk/apps/teradesk/build/dist/DESKTOP.NAP`): `lsr x9, x0, #16` →
   `sturh w9, [x29,#-0x20]` (High-Wort nach `aes_intin[0]`), `sturh w0, [x29,#-0x1e]`
   (Low-Wort nach `aes_intin[1]`).
2. `svc #2` → `tos_api::aes::dispatch` (`tos-api/src/aes/mod.rs:376`) → `crysbind`
   → `event::evnt_timer` (`tos-api/src/aes/event.rs:671-685`):
   `ms = (int_in[1] << 16) | int_in[0]` = `150 << 16` = **9 830 400 ms** →
   `wait_ms(ms)` → `sched::sleep(wait_timer)` (`bdos/process/sched.rs:299-330`)
   → Ausgang (a): `pump_input()` + `idle_wake_pass()` in Schleife bis zur Deadline.
3. Damit übernimmt nichts mehr: `main.c:1323-1324` (zweites `bell`, Dialog) werden
   nicht erreicht, `menu_tnormal` nicht (der Titel *File* bleibt invertiert —
   sichtbar im Screendump), `quit` bleibt `FALSE`, kein Epilog, kein `appl_exit`,
   kein `Pterm`. `launch::run_program_with_tail_rc` (`kernel/src/launch.rs:126-134`)
   kehrt nicht zurück, `sh_ldapp` (`kernel/src/shell_loop.rs:293`) nicht,
   `desk::run()` (`kernel/src/desk/mod.rs:637`) wird nie betreten.
4. Warum sich die Maus trotzdem bewegt: auf der Pi kommt Eingabe **nur** über
   `Host::pump_input` → `usb::poll` (`kernel/src/host_env.rs:84-119`); die
   AES-Warteschleife ruft es bei jedem Durchlauf, und `ikbd::feed` zieht den
   Zeiger. Ein hängender Kernel würde die Maus einfrieren; eine hängende AES-Wartung
   nicht — genau das Bild aus dem Ticket.

## 3. Was EmuTOS tut, und wo die Abweichung liegt

| | Datei:Zeile |
|---|---|
| EmuTOS `EVNT_TIMER` | `references/emutos/aes/gemsuper.c:142-143` `ev_timer(MAKE_ULONG(T_HICOUNT, T_LOCOUNT))` |
| `T_LOCOUNT = int_in[0]`, `T_HICOUNT = int_in[1]` | `references/emutos/include/aesdefs.h:190-191` |
| `MAKE_ULONG(hi, lo)` | `references/emutos/include/portab.h:139` |
| NewTOS `evnt_timer` | `tos-api/src/aes/event.rs:673-675` — `lo = int_in[0]`, `hi = int_in[1]`, **identisch** |
| gemlib-Bindung (Abweichung) | `sdk/gemlib/gemlib-0.44.0/a_evnt_timer.c:27-31` — Byte-Order-/Wortbreiten-Punning, nur auf m68k richtig |
| Vergleich: `mt_evnt_multi` macht es richtig | `sdk/gemlib/gemlib-0.44.0/a_evnt_multi.c:91-92` — `interval`, `interval >> 16` |

Gepinnte Quelle: `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`.

**Die Abweichung liegt nicht im Kernel.** Kernel und EmuTOS lesen die Worte
gleich. Abweichend ist die **portierte Anwendungsbibliothek**: `a_evnt_timer.c`
wird aus `gemlib-0.44.0.tar.xz` unverändert (kein Eintrag in
`sdk/gemlib/overlay/`) für AArch64 übersetzt. Es ist dieselbe Defektklasse wie
`065270b` („give BFOBSPEC a little-endian field order"): eine m68k-Annahme über
Byte-Reihenfolge/Wortbreite in gemlib, die auf LP64/LE still falsch wird. Der
Kernel bekommt den Fehler bloß getreu ausgeführt — eine App, die 2,7 Stunden warten
will, darf das, und die AES tut es.

## 4. Messergebnis QEMU (`raspi4b`, 640×480, Dev-Bau aus unverändertem Baum)

**Reproduziert: ja.** Weil `raspi4b` keine Eingabe hat, wurde eine
**Scratch-Kopie** des Workspace mit einem IKBD-Byte-Injektor (Maus-Skript in
`pump_input`) und einem AES-Opcode-Trace gebaut
(`measurement-2026-09-17-h03/method/scratch-kernel-injector.diff`); das
Projektverzeichnis blieb unberührt. Skript: Zeiger auf (0,0) → *File* hovern
(72,8) → *Quit/Shutdown* (150,235) klicken → (320,313) klicken → wackeln.

Ausgeliefertes `DESKTOP.NAP`, letzte AES-Aufrufe nach dem Klick
(`serial/run3-teradesk-shipped-scripted-quit-AESTRACE.log`):

```
[aes] >> op 106 150 235      wind_find
[aes] >> op 79 0 0           graf_mkstate
[aes] >> op 104 0 4          wind_get(0, WF_WORKXYWH)
[aes] >> op 44 0 0           objc_offset
[aes] >> op 104 0 10         wind_get(0, WF_TOP)
[aes] >> op 24 0 150         evnt_timer  int_in[0]=0  int_in[1]=150   <-- kehrt nie zurück
[h03] step 16 at +5000 ticks -> mouse (300, 293)
[h03] step 17 at +5100 ticks -> mouse (320, 313)
```

Screendumps: `screendumps/run2-shipped-a-file-menu-open.png` (Menü offen),
`run2-shipped-c-15s-after-click-NO-dialog.png` (Titel *File* invertiert, kein
Dialog), `run2-shipped-e-35s-still-teradesk-pointer-moved.png` (Zeiger an neuer
Position, Bild sonst unverändert) — das Ticketbild.

**Gegenprobe A — Rückweg funktioniert:** Sonde `probe/quittest.c` (Modus 1,
TeraDesks Aufruffolge inkl. `rsrc_load/free`, `v_opnvwk/v_clsvwk`, `WF_NEWDESK`,
`menu_bar`, Fenster, `form_dial`, `Malloc/Mfree`, `appl_exit`, `Pterm(0)`) per `#Z`:

```
[quittest] appl_exit -> 1
[quittest] returning 0 -> Pterm(0)
[launch] returned (program Pterm'd)
[shell] sh_ldapp: 'C:\APPS\TERADESK\QUITTST1.NAP' exited with rc=0
[shell] sh_ldapp: starting NEWDESK, sh_nextapp=3, sh_isgem=1
[desk] desktop up (1 drives)
```
(`serial/run1-probe-quitpath-mode1.log`, `screendumps/run1-probe-after-pterm-desktop-up.png`)

Die vom Auftrag verlangten Varianten derselben Sonde: Modus 2 (zusätzlich
`shel_write(SHW_SHUTDOWN, 2, 2, NULL, NULL)` vor `appl_exit`, wie TeraDesks
`QUITSHUT` ohne `lobo()`) und Modus 3 (nur `appl_init` → `evnt_multi(MU_TIMER)` →
`appl_exit` → `Pterm(0)`) kommen **beide** ebenfalls zu `[desk] desktop up`
(`serial/run7-probe-mode2-shw_shutdown.log`, `run7-probe-mode3-bare.log`);
`SHW_SHUTDOWN` wird von `shel_write` nicht aufgezeichnet (`aes/shell.rs:441`,
`_ => {}`), der Loop läuft weiter. **Keine Sondenvariante hängt** — nur das echte
TeraDesk, und dort im `wait(150)`.

**Gegenprobe B — mit korrekter Bindung beendet sich das echte TeraDesk:**
`DESKTOP.NAP` im Scratch neu gelinkt aus den unveränderten TeraDesk-Objekten plus
`probe/a_evnt_timer_fixed.c` (`interval & 0xFFFF`, `interval >> 16`), Kernel
unverändert (`serial/run6-teradesk-fixed-evnt_timer-clean.log`):

```
[aes] >> op 24 150 0         (run4, mit Trace) -> kehrt zurück, Dialog erscheint
[launch] returned (program Pterm'd)
[shell] sh_ldapp: 'C:\APPS\TERADESK\DESKTOP.NAP' exited with rc=0
[shell] sh_ldapp: starting NEWDESK, sh_nextapp=3, sh_isgem=1
[desk] desktop up (1 drives)
```
`screendumps/run6-fixed-c-quit-dialog.png` (Dialog *Quit / Shutdown Options*),
`run6-fixed-e-desktop-up.png` (eingebauter Desktop mit `C:\`-Fenster).

## 5. Regressionsfrage

**Nein, war vorher schon so.**
* `343b3c5` (desk/-Rückschnitt) berührt ausschließlich `kernel/src/desk/*`,
  `docs/`, `measurement-2026-09-14-desk/` (`git show 343b3c5 --stat`); der
  `#Z`-Rückweg (`shell_loop.rs`, `launch.rs`, `event.rs`) ist unberührt, und der
  Hänger liegt ohnehin **vor** `Pterm` in der Anwendung. Der Rückweg selbst ist auf
  dem heutigen HEAD (inkl. `343b3c5` und A65) zweimal gemessen intakt (Gegenproben A, B).
* A65 (`tos-api/src/aes/object.rs`, Zeigerklammer) hat mit `evnt_timer` nichts zu tun.
* `evnt_timer` im Kernel liest `int_in[0]` als Low- und `int_in[1]` als High-Wort
  seit seiner Einführung `78e915f` (2026-06-20) unverändert.
* Die gemlib-Bindung ist seit dem Port `371a477` (2026-07-19) so; kein Overlay für
  `a_evnt_timer.c`. `docs/HW-TEST-LOG.md` kennt keinen erfolgreichen TeraDesk-Quit
  auf Hardware — der Weg war nie gangbar, er ist am 2026-09-17 nur erstmals begangen
  worden. `wait()` wird in TeraDesk sonst nur im Shutdown-Zweig gerufen
  (`main.c:2054, 2068`), also fiel es vorher nie auf.

Die G1-Spur des Tickets (`desk::run()` kehrt nie zurück) ist für H03 **nicht die
Ursache**: bei `#Z` wird `desk/` nach dem Autorun-Programm von `sh_main` **von oben**
betreten (`shell_loop.rs:306-308, 78-101, 240`), nicht aus einem verschachtelten
`run_gem_app`. G1 bleibt ein Thema für den Doppelklick-Start aus `desk/` heraus.

## 6. Ursache in einem Satz, und die Schicht

**`sdk/gemlib/gemlib-0.44.0/a_evnt_timer.c` zerlegt das `unsigned long`-Intervall
per Zeiger-Punning in der m68k-Byte-Reihenfolge; auf AArch64 LP64 vertauscht das
Low- und High-Wort, TeraDesks `wait(150)` vor dem Quit-Dialog wird zu
`evnt_timer(9 830 400 ms)`, und die AES wartet — mit laufender Mauseingabe —
2 h 44 min.** Schicht: **Toolchain / gemlib-Port** (Reparaturort nach dem
bestehenden Muster: `sdk/gemlib/overlay/a_evnt_timer.c` mit endianness-freier
Zerlegung, so wie `a_evnt_multi.c:91-92` es bereits tut; dann
`build-gemlib.sh` + `build-teradesk.sh` + QED neu bauen). Kein Kernel-, kein
desk/-, kein AES-Defekt. Kein Fix in diesem Auftrag.

## Offen / Nebenbefunde (nicht verfolgt)

* **Hardwareabnahme** nach dem Fix: TeraDesk auf der Pi per Menü beenden → Dialog →
  *Quit* → eingebauter Desktop. Erwartung aus QEMU: ≈150 ms nach dem Klick zwei
  Piepser, dann der Dialog.
* Weitere Zeiger-Punnings in gemlib betreffen nur NVDI-Erweiterungen
  (`n_v_color2value.c:28`, `n_vq_ctab.c:23`, `n_v_ftext16n.c:20`) — gleiche
  Defektklasse, hier nicht geprüft, von TeraDesk/QED nicht benutzt (nicht verifiziert).
* `menu_register(-1, …)` liefert bei uns −1 (Sonde) — EmuTOS `mn_register` nimmt
  nur `pid >= 0` als Accessory auf und liefert sonst −1
  (`references/emutos/aes/gemmnlib.c:794, 815`), also kein Befund; nur notiert.
* Der scharfe AES-Trace (run4) schreibt auf die fbcon und scrollt den Bildschirm —
  darum ist `run4` nur seriell zu lesen; `run6` liefert die sauberen Bilder.

## Aufräumen

Projektbaum unverändert: `git status` zeigt nur das fremde, gleichzeitig
entstehende `docs/sdk/` (nicht von H03). `kernel/src/host_env.rs` sha256
`287ff741…` vor und nach der Messung; Injektor und Trace existieren nur in der
Scratch-Kopie (`measurement-2026-09-17-h03/method/scratch-kernel-injector.diff`
dokumentiert sie). Keine QEMU-Prozesse, kein gemountetes Scratch-Image, Karte nur
gelesen. Neu im Baum (unversioniert, nur Artefakte): `measurement-2026-09-17-h03/`,
diese Quittung.

## Verifikation (Tester, 2026-09-17)

Unabhaengig nachgemessen, ohne dem Developer-/Auditor-Selbstreport zu trauen.
Kein Produktcode geaendert; einzige temporaere Instrumentierung war der vom
Auftrag vorgegebene `scratch-kernel-injector.diff`, angewendet ausschliesslich
in einer Scratch-Kopie (`kernel/src/host_env.rs` im Projektbaum sha256
`287ff741...` vor und nach der Messung identisch). Artefakte:
[`measurement-2026-09-17-h03/verify/`](../measurement-2026-09-17-h03/verify/RUNINFO.TXT).

| # | Messung | Ergebnis |
|---|---|---|
| 1 | Hosttest `evnt_timer_words` (gegen `sdk/gemlib/gemlib-0.44.0/a_evnt_timer.c`, Overlay bereits angewendet) | **PASS** -- rc=0, `OK -- mt_evnt_timer hands int_in[0]=low, int_in[1]=high`; zwei vorbestehende `__nonnull`-Warnungen (host-only) |
| 2 | sha256 `DESKTOP.NAP` | **PASS** -- `db31aae1...`, entspricht dem Fix (vorher `d957d765...`) |
| 3 | Disassembly `mt_evnt_timer` (`llvm-objdump -d`) | **PASS** -- `sturh w0, [x29,#-0x20]` (Low, ohne `lsr`) an `intin[0]`, `lsr x8,x0,#16` + `sturh w8,[x29,#-0x1e]` (High) an `intin[0]+2`; AES_PARAMS-Layout (`gem_aesP.h`) bestaetigt `x29-0x20` = `intin` |
| 4 | QEMU raspi4b, Startpfad (`#Z` Autorun, unveraenderter Kernel + neue Karte) | **PASS** -- Serial `sh_ldapp: starting C:\APPS\TERADESK\DESKTOP.NAP`, Screendump zeigt TeraDesk-Oberflaeche |
| 5 | QEMU raspi4b, Quit ueber das Menue (Scratch-Kopie, IKBD-Injektor, zwei unabhaengige Laeufe) | **PASS** -- Dialog *Quit / Shutdown Options* erscheint (Screendump), Klick auf *Quit* -> `[launch] returned (program Pterm'd)` -> `sh_ldapp: starting NEWDESK` -> `[desk] desktop up (1 drives)`, in beiden Laeufen identisch |
| 6 | QED-Kontrolle | **PASS** -- sha256 `d4b147df...` unveraendert, `mt_evnt_timer` nicht im Symtab (0 Treffer) |
| 7 | Change-Set-Kontrolle (`git diff --stat`, `git status`) | **PASS** -- nur `sdk/README.md` (vorbestehend/fremd) geaendert; neu ausschliesslich `sdk/gemlib/overlay/a_evnt_timer.c` und `sdk/gemlib/tests/evnt_timer_words.c`; kein Crate im Diff |

**Abweichung vom Auftrag (woertlich):** der erwartete AES-Opcode-Trace
`>> op 24 150 0` wurde in keinem der beiden Quit-Laeufe beobachtet (0 Treffer
auf `[aes]` in beiden Serial-Logs). Ursache: im wiederverwendeten
`scratch-kernel-injector.diff` ist die Stelle, die `H03_TRACE` scharf schaltet,
tot codiert (`if i == 6 && false { ... }`, `kernel/src/host_env.rs`
H03-TEMP-Block) -- wegen `&& false` nie erreichbar. Das Diff wurde unveraendert
wie vorgegeben angewendet, nicht repariert (nicht Aufgabe des Testers). Der
funktionale Beleg -- Dialog-Screendump plus vollstaendige Rueckkehrsequenz in
zwei unabhaengigen Laeufen -- traegt den PASS von Messung 5 trotzdem; die
fehlende Trace-Zeile ist ein offener Punkt fuer, falls der Injektor spaeter
wiederverwendet wird.

**Gesamtverdikt: PASS.** Der Fix (`sdk/gemlib/overlay/a_evnt_timer.c`) behebt
den in H03 beschriebenen Haenger nachweisbar: Wortreihenfolge auf Quellebene
(Host), im ausgelieferten Maschinencode (Disassembly) und im End-zu-Ende-Verhalten
(QEMU, zwei unabhaengige Laeufe) bestaetigt. QED unveraendert und nicht betroffen.
Real-Pi4-only: interaktives Beenden per echter USB-Maus/-Tastatur (kein
IKBD-Injektor) ist nicht durch diese Messung abgedeckt -- Empfehlung: in
`docs/HW-TEST-LOG.md` / den HW-Test-Checklisten als offene Hardwareabnahme
fuehren (wie bereits in TASK/H03-STATUS-DONE.MD Abschnitt "Offen / Nebenbefunde"
vermerkt).
