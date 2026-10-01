# X03 — Quittung

**Status: IMPLEMENTIERT · in QEMU auf beiden Boards belegt · Abnahme durch den
Tester offen** · 2026-10-01 · Basis `desk-rebuild`, HEAD `2c0e63b`, Baum vorher
sauber · Belege [measurements/measurement-2026-10-01-x03/](../measurements/measurement-2026-10-01-x03/)

Die Maschinenzeile des Info-Kastens kommt jetzt aus einer Bau-Konstante im
Desktop (`MACHINE_NAME`, per `cfg(feature = "board-…")`), nicht mehr aus
`crate::initinfo::machine_name()`. Driftprobe **32 → 31**. Texte unveraendert;
beide Info-Kaesten sind pixelgleich zu HEAD.

## Was tatsaechlich getan wurde

| Datei | Stelle | was |
|---|---|---|
| `kernel/src/desk/inf.rs` | neuer Block `MACHINE_NAME` hinter `AUTHORSHIP_LINE` | `board-rpi4` → `"Raspberry Pi 4 (BCM2711)"`, `board-qemu-virt` → `"QEMU aarch64-virt"`; dazu `compile_error!`, wenn keins der beiden Features gesetzt ist. Doc-Kommentar: Herkunft (Anwenderentscheidung 2026-10-01), warum kein `_MCH`, die bewusst hingenommene Grenze (eigenstaendig gebauter Desktop kennt das Feature nicht mehr). |
| dieselbe | `do_aboutdesk` | `Line::new(crate::initinfo::machine_name())` → `Line::new(MACHINE_NAME)`; die `[STOP — DESK43 U4]`-Notiz ersetzt durch einen Verweis |
| dieselbe | Doc von `do_aboutdesk` | Tabellenzeile 3, Absatz „Two readers, two copies" (Maschinenzeile ergaenzt), Abweichung 2 (Breite) |
| `kernel/src/initinfo.rs` | `machine_name` | Doc neu: der Desktop liest sie nicht mehr, hat eine eigene textgleiche Kopie; Sichtbarkeit `pub(crate)` → privat, damit kein zweiter Leser wieder andockt. Funktion und `other => other` unveraendert (Startbildschirm, kernelseitig) |
| `docs/api/desktop/deskmain.md` | Info-Kasten, Tabelle + Absatz darunter + Abweichung 2 | Quellen der vier Zeilen auf die Desktop-Konstanten umgestellt; der Absatz behauptete noch die gemeinsame `initinfo`-Quelle (seit U4 fuer drei, jetzt fuer alle vier Zeilen falsch) und nennt jetzt Bau-Konstante, Fehlerfall und Grenze. Zeilenzitat `deskmain.c:1881` dabei auf **`:1880`** korrigiert (dort steht `objversion->ob_spec = (LONG) version;`, gegen `references/emutos` @ `1a7484ab` gelesen) |
| `docs/analysis/desktop-userspace-plan-v2.md` | §2.2-Tabelle und U4-Abschnitt | je ein „Nachtrag X03": der `_MCH`-Weg traegt nicht, umgesetzt ist die Bau-Konstante |
| `TASK/X03.md` | Statuszeile | IMPLEMENTIERT, Tester offen |

Kein gemeinsames Kernel-Symbol fuer den Desktop angelegt; `crate::BOARD_NAME`
bleibt, wie es war, ein Kernel-Symbol fuer Startausgabe und Startbildschirm.

## Fehlerfall: Board ohne Zeile → Compilefehler, kein Ersatztext

Der Startbildschirm faellt mit `other => other` auf den rohen Board-Bezeichner
zurueck. Diesen Bezeichner sieht der Desktop nicht (er duerfte dafuer ja gerade
kein Kernel-Symbol lesen); jeder Ersatztext, den er sich selbst ausdenkt, waere
genau die Zeile, „die auf dem falschen Brett die falsche Maschine behauptet",
die das Ticket ausschliesst. Also: ein neues Board-Feature muss seine Zeile
mitbringen, sonst baut der Kernel nicht. Heute faengt `kernel/build.rs` den
Fall „kein Board-Feature" schon vorher ab; der Desktop-Waechter greift, sobald
jemand ein `board-rpi5` in `Cargo.toml`/`build.rs`/`lib.rs` einzieht und
`desk/inf.rs` vergisst. Belegt mit einer Scratch-Simulation
(`method/missing_line_check.sh`): genau **ein** Fehler, unsere Meldung, Exit 101.

## Pruefung, die gelaufen ist

* **QEMU, beide Boards, Info-Kasten geoeffnet** (Scratch-IKBD-Injektor wie
  DESK50, Desk → „Desktop info..."):
  `screendumps/rpi4-c-about.png` zeigt `Machine  Raspberry Pi 4 (BCM2711)`
  (raspi4b, 640x480), `screendumps/qemuvirt-c-about.png` zeigt
  `Machine  QEMU aarch64-virt` (virt + ramfb, 1920x1080).
* **Vorher/Nachher:** derselbe Lauf mit HEAD-Fassung beider Dateien — je Board
  alle drei Bilder **0 abweichende Pixel**.
* **ELF:** das rpi4-Produkt-ELF enthaelt nur „Raspberry Pi 4 (BCM2711)", das
  qemu-virt-ELF nur „QEMU aarch64-virt" (`method/elf-strings.txt`).
* 0 `[EXC]`/`[PANIC]` in allen vier seriellen Logs.
* **Basislinien** (alle unveraendert):
  * Hosttests `cargo test -p tos-api -p tos-types -p xtask --no-fail-fast`:
    **964 / 0 / 1** (summiert).
  * Frische Release-Bauten nach `cargo clean -p kernel -p tos-api --target
    aarch64-unknown-none-softfloat --release`, je Board einzeln (beide Logs
    „Compiling tos-api" + „Compiling kernel"): rpi4 **64 / 94**, qemu-virt
    **64 / 94** (tos-api / kernel Warnungen).
  * clippy `--features board-qemu-virt` und `board-rpi4`: je **80 / 125**
    (tos-api / kernel).
  * `sh tools/desk-drift-probe.sh`: **31** (vorher 32).

## Was offen bleibt

1. **Abnahme durch den Tester.**
2. **Echte Pi 4** — nicht noetig fuer den Inhalt (Bau-Konstante, im ELF
   belegt), aber der Info-Kasten auf der Karte ist nicht gesehen.
3. **Grenze, bewusst hingenommen (Ticket):** wird der Desktop ein eigenstaendig
   gebautes Programm, kennt er das Board-Feature nicht mehr; dann ist
   Cookie/`Ssystem` neu zu entscheiden. Steht im Doc-Kommentar von
   `MACHINE_NAME` und in `deskmain.md`.

## Nebenbefunde, nicht angefasst

* `deskmain.md` Info-Kasten, Abweichung 3 („No build date … NewTOS has no
  build-date constant anywhere") ist seit DESK12 (`tos_api::build`) veraltet.
* ~~`kernel/src/desk/inf.rs` zitiert `deskmain.c:1880` … die Zeile ist `:1879`.~~
  **Korrektur Lead 2026-10-01:** `:1880` ist richtig (`objversion->ob_spec =
  (LONG) version;`; `:1879` ist der Kommentar darueber). Die Spec-Stelle in
  `deskmain.md` wurde entsprechend auf `:1880` gesetzt, nicht `:1879`.

## Nicht committet

Die Arbeit liegt unversioniert im gemeinsamen Baum (vier Dateien + diese
Quittung + `TASK/X03.md` + das Messverzeichnis). `docs/STATUS.md` nicht
angefasst.
