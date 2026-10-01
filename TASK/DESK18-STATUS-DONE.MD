# DESK18 — STATUS: DONE (2026-09-23)

**Auftrag:** `do_aopen`s Pfadwahl mit `set_default_path` nachbauen — NewTOS
setzt vor einem Programmstart **gar kein** Arbeitsverzeichnis —, danach die
Radiogruppe *Default dir* in `inf_conf` nachziehen.

Basis: Branch `desk-rebuild`, HEAD `e2437af`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset`.

Messung: [`measurements/measurement-2026-09-23-desk18/`](../measurements/measurement-2026-09-23-desk18/).
Spezifikation: `docs/api/desktop/deskpro.md` (§ *The current-directory step*),
`docs/api/desktop/deskinf.md` (§ *Default dir* ist zurueck),
`docs/api/desktop/deskmain.md` (§ `CSAVE`).

---

## 1. Der Defekt, und dass er einer war

`kernel/src/desk/main.rs::launch_native` ging direkt in
`crate::launch::run_program`. Auf dem ganzen Weg steht kein `Dsetdrv` und
kein `Dsetpath`; die per-Laufwerk-Verzeichnisse in
`tos-api/src/bdos/fs/path.rs` bleiben also auf ihrem Einschaltwert
(Wurzel, Laufwerk C). Ein Programm, das `Fopen("FOO.TXT")` ruft, bekam die
Datei aus `C:\`, egal aus welchem Verzeichnis es gestartet wurde.

Das ist gemessen, nicht hergeleitet. Ein **Kontrollbau**
(`method/desk18_before_patch.py`, nimmt nur den neuen Aufruf wieder heraus)
liest auf derselben Karte mit derselben Klickfolge:

    CURDIR: drive C:
    CURDIR: read "PROOF: C:\ (root)"

der Produktbau:

    CURDIR: drive C:\SUB
    CURDIR: read "PROOF: C:\SUB"

Das Testprogramm ist neu — es gab keins, das einen blossen relativen Namen
oeffnet: `sdk/examples/curdir.c`, von `tools/make-sd-image.sh` als
`C:\SUB\CURDIR.NCP` installiert, mit zwei gleichnamigen Dateien
`C:\PROOF.TXT` und `C:\SUB\PROOF.TXT`, die sich in ihrem Inhalt nennen.

## 2. Was gebaut ist

| EmuTOS | NewTOS |
|---|---|
| `set_default_path` (`desksupp.c:1785-1801`) | `desk::pro::set_default_path` |
| `dos_chdir` = `Dsetpath` (`gemdos.h:95-98`) | `desk::dosbind::dos_chdir` (neu) |
| `do_aopen`s Pfadwahl (`desksupp.c:893-916`) | `desk::main::set_launch_dir` |
| `STDEFDIR` (`desk_rsc.c:2558-2559`) | `dir::fun_alert(1, …)` in `set_launch_dir` |
| `G.g_appdir` (`deskbind.h:180`) | `global::DeskState::g_appdir` |
| `cs_appdir` (`deskapp.h:153`) | `global::CSave::cs_appdir` |
| `cnx_put`/`cnx_get` (`deskmain.c:1391`, `:1453`) | `main::cnx_put`/`cnx_get_prefs` |
| `INF_E5_APPDIR` (`deskapp.c:110`, `:942`, `:1164`) | `inf::INF_E5_APPDIR` in `env_to_state`/`state_to_env` |
| `ADDESKCF` 2..5 (`desk_rsc.c:1615-1637`) | die *Default dir*-Gruppe in `inf::inf_conf` |
| `pa->a_flags & AF_APPDIR` | `inf::Association::app_dir` (`INF_AT_APPDIR`) |

`do_aopen` liegt in NewTOS in zwei Haelften — `open_item`s
`OpenTarget::App` (Programm direkt gewaehlt, Schalter `G.g_appdir`) und
`open_document` (Datei mit installierter Anwendung, Schalter `AF_APPDIR`) —,
also nimmt der gemeinsame Rumpf den Schalter als Parameter. Das ist genau
EmuTOS' `if (is_installed(pa)) … else …`.

## 3. Benannte Abweichungen

1. **`own_path` ist der Programmpfad, nicht die Fenstermaske.** EmuTOS
   uebergibt fuer den nicht-installierten Zweig `pathname` = `p_spec` des
   Fensters (`C:\SUB\*.*`), NewTOS den vollen Programmpfad
   (`C:\SUB\PROG.NCP`). `filename_start` (`util/optimize.c:226-235`)
   schneidet beide auf dasselbe `C:\SUB\` zurueck, und mehr macht die
   Funktion mit der Zeichenkette nicht.
2. **Objektnummern sind NewTOS'.** Durch die weiter fehlende
   *Parameter*-Gruppe sind die `ADDESKCF`-Indizes ab *Function keys* um vier
   kleiner als in `desk_rsc.h:191-211`; die **Namen** sind EmuTOS'. Wie
   `inf_pref` es seit DESK02 macht. Objekte 0..5 sind unveraendert die der
   Quelle.
3. **Kastenhoehe 312x320.** `ADDESKCF` ist 39x22 Zeichen; ohne die zwei
   Zeichenzeilen der *Parameter*-Reihe sind es 39x20. *Default dir* behaelt
   Zeile 3, alles darunter rueckt zwei Zeilen hoch statt DESK05s vier.
4. **`set_default_path` laesst bei einem Fehlschlag das aktuelle Laufwerk auf
   dem nicht erreichbaren stehen** — `dos_sdrv` kommt vor `dos_chdir` und
   prueft nichts (`desksupp.c:1794`, unser `dsetdrv` ignoriert nur
   ausserhalb von `BLKDEVNUM`). Das ist EmuTOS' Reihenfolge, nicht unsere
   Zutat; der naechste erfolgreiche Start korrigiert es.

## 4. Der ehrliche Befund zu *Default dir*

**Beide Stellungen waehlen auf dem heutigen Desktop dasselbe Verzeichnis.**
Nicht weil der Schalter nicht gelesen wird — `set_launch_dir` liest ihn bei
jedem nicht-installierten Start —, sondern weil die AES einen Klick auf ein
Fenster, das nicht das oberste ist, dem Besitzer als `WM_TOPPED` gibt und den
Klick verbraucht (EmuTOS `hctl_window`, `aes/gemctrl.c:172-179`; unsere AES
ebenso — im Protokoll `[aes] … op 25 … out=(16,…)` = `MU_MESAG`). Das Fenster
des doppelgeklickten Programms ist danach immer das oberste, also sind
„Anwendungsverzeichnis" und „oberstes Fenster" dasselbe.

EmuTOS bekommt die zwei Stellungen nur auf Wegen auseinander, die NewTOS
nicht hat:

* Desktop-Verknuepfung (`CONF_WITH_DESKTOP_SHORTCUTS`, `desksupp.c:854-872`),
  wo `pathname` der gespeicherte Pfad der Verknuepfung ist;
* Datei auf ein Programmsymbol abgelegt (`fun_win2win`, `deskfun.c:958-970`
  → `do_aopen`), den Zweig hat `fun_win2win` hier nicht (DESK05 §1).

**Derselbe Programmzweig** (`if !app_dir && pw != DESKWH`) ist trotzdem
beidseitig gemessen, ueber die Zuordnungsseite: `C:\SUB\CURDIR.NCP` als
Handler fuer `*.TXT`, einmal mit und einmal ohne `INF_AT_APPDIR`, gestartet
durch einen Doppelklick auf `C:\HELLO.TXT` im (obersten) `C:\`-Fenster:

    Flagge gesetzt  → CURDIR: drive C:\SUB   read "PROOF: C:\SUB"
    Flagge geloescht→ CURDIR: drive C:       read "PROOF: C:\ (root)"

Wenn einer der zwei fehlenden Wege landet, faengt die Gruppe ohne weitere
Aenderung hier an, die Stellungen zu unterscheiden.

## 5. Was gemessen wurde

QEMU `-M raspi4b`, 640x480, `-display none`, QMP-Screendumps, Kratzkopie des
IKBD-Injektors (`method/make_scratch_desk18.sh`). Zehn Laeufe, **0
`[EXC]`/`[PANIC]`** in allen zehn Protokollen, `fsck_msdos -n` auf den
FAT16-Partitionen aller zehn Abbilder ohne Fehler.

* **P/Q** — Sondierungen: Fenster- und Dialoggeometrie. Jeder Klickpunkt
  unten stammt aus diesen zwei Auszuegen, keiner ist gerechnet.
* **B0 vs. A** — der Kontrollbau gegen den Produktbau (§1).
* **I1 vs. I0** — die Zuordnungsseite, beide Flaggenstellungen (§4).
* **A vs. W** — beide Stellungen von *Default dir* aus der Datei
  (`g_appdir=true`/`false` im Protokoll), beide lesen `C:\SUB` (§4).
* **D** — Dialog: *Application* vorgewaehlt, ein Klick waehlt *Window* und
  *Application* geht aus (`G_IBOX`-Elternknoten), `OK`, `Save desktop...` →
  `#E 1A E0 00 00 20` (Byte 5 `0x60` → `0x20`, `INF_E5_ISFULL` unberuehrt).
* **C** — Kaltstart aus genau dieser Karte: `g_appdir=false`, Dialog mit
  *Window* vorgewaehlt.
* **E** — `STDEFDIR`: Zuordnung auf `D:\NOSUCH\APP.NCP`, Kasten
  `Failed to set default directory.`, **kein** Start dahinter.

## 6. Bau

`cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat` vor
**jedem** Board.

    cargo xtask build --board rpi4       tos-api 64 / kernel 94    (Basis 64/94)
    cargo xtask build --board qemu-virt  tos-api 64 / kernel 94    (Basis 64/94)
    cargo clippy --features board-rpi4        tos-api 80 / kernel 125 (Basis 80/125)
    cargo clippy --features board-qemu-virt   tos-api 80 / kernel 125 (Basis 80/125)
    grep -c "desk/" auf beiden Clippy-Laeufen:  0
    cargo run -p xtask -- test   938 passed / 0 failed / 1 ignored (Basis 938/0/1)

Protokolle in `measurements/measurement-2026-09-23-desk18/build/`.

**Kein neuer Hosttest.** Die Aenderung liegt vollstaendig in
`kernel/src/desk/` (`no_std`, aarch64) und ist vom Hostlauf nicht erreichbar;
`tos-api` hat kein Stueck davon. Der `#E`-Byte-5-Rundlauf hat das neue Bit
bekommen: `inf::selftest_inf_roundtrip` fuehrt jetzt `appdir = false` (den
Nicht-Vorgabewert) mit — das ist allerdings ein Brett-Selbsttest, der in
keinem Bootpfad haengt (Erinnerung `unwired-selftests-rot-silently`).

## 7. Offen / Folgetickets

* **Pi-Sichtabnahme** — HW-Punkt 43 in
  `docs/api/desktop/_hw-test-2026-09-17.md`. Nur dort zu pruefen: die
  1920x1080-Optik des groesseren Kastens und — das kann QEMU gar nicht — das
  **Laufwerk**, weil `set_default_path` auch `Dsetdrv` ruft und die echte
  Karte `C:`–`F:` hat.
* **Der `\AUTO\`-Ordner setzt weiterhin kein Verzeichnis.**
  `crate::launch::run_auto_folder` ruft den Lader direkt; EmuTOS startet
  `\AUTO\`-Programme aus dem BIOS, also gibt es dort auch kein `do_aopen`,
  aber ein `.NCP`, das relativ oeffnet, sieht die Wurzel. Nicht angefasst —
  eigener Befund, eigenes Ticket.
* **`G.g_fullpath` / *Parameter*** — haengt weiter an `fun_win2win`s
  Ablegen-auf-Programm-Zweig (`deskfun.c:958-970`) und
  `build_selected_path` (`:903-931`). Unveraendert offen seit DESK05.
* **Der Klick auf ein nicht oberstes Fenster** kostet einen Klick (AES
  `WM_TOPPED`), was EmuTOS-treu ist — aber `top_msg` ruft `desk_clear`, also
  ueberlebt keine Auswahl einen Fensterwechsel. Fuer `File → Open` auf eine
  Auswahl in einem anderen Fenster (EmuTOS' `G.g_cwin`-Weg) waere das zu
  pruefen. Nur beobachtet, nicht angefasst.
