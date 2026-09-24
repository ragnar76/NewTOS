# DESK28 — STATUS: DONE (2026-09-24)

**Auftrag:** Den Verzeichnisschritt aus `desk::pro::set_default_path` an den
AUTO-Startpfad anlegen — und vorher an `bios/bios.c`s `autoexec` gegenpruefen,
welchen Pfad TOS dort setzt.

Basis: Branch `desk-rebuild`, HEAD `3f6b6ef`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset`.

Messung:
[`measurement-2026-09-24-desk22-desk23-desk28-desk29/`](../measurement-2026-09-24-desk22-desk23-desk28-desk29/)
§1-2. Spezifikation: `docs/api/emutos-boot-flow.md` §3.

---

## 1. Die Gegenprobe an der Quelle — die Antwort ist „Wurzel"

`run_auto_program` (`bios/bios.c:685-695`) baut den **vollen** Pfad
`"\AUTO\<name>"` und gibt ihn an `Pexec(PE_LOADGO, path, "", NULL)`. In ganz
`autoexec` (`:697-733`) steht kein `Dsetdrv` und kein `Dsetpath`. Das
Arbeitsverzeichnis des Kindes ist damit reines Erbe:

* `Pexec` kopiert das aktuelle Laufwerk und die ganze
  Pro-Laufwerk-Verzeichnistabelle aus der Basepage des Rufers in die des
  Kindes (`init_pd_files`, `bdos/proc.c:400-410`),
* der Rufer ist das BIOS, dessen Zustand `Dsetdrv(bootdev)` (`bios.c:827`)
  ist, und
* fuer jedes Laufwerk, das es nie `Dsetpath`t hat, haengt `ckdrv` beim ersten
  Zugriff die **Wurzel**-DND ein (`bdos/fsdrive.c:119-131`).

Ein AUTO-Programm, das `FOO.TXT` mit blossem Namen oeffnet, liest also
`<Bootlaufwerk>:\FOO.TXT`. Die Ticketpraemisse „das ist derselbe Defekt wie
DESK18 an der anderen Stelle" ist damit **zurueckgezogen**: die Wurzel zu
sehen ist TOS-Verhalten, kein Defekt. `tools/make-sd-image.sh` hatte das
uebrigens schon notiert („Never in \AUTO\: the AUTO path has its own (root)
directory").

## 2. Was uebrig bleibt — und ein Defekt ist

Was EmuTOS mit einer **Kopie** erreicht, hat NewTOS nicht: das aktuelle
Laufwerk und die Verzeichnisse sind globale Statics, der dokumentierte
Platzhalter fuer `p_curdrv`/`p_curdir` (`tos-api/src/bdos/fs/drive.rs`,
`…/fs/path.rs`). Ein `Dsetpath` des einen AUTO-Programms wird deshalb vom
naechsten und vom Desktop danach gesehen — auf EmuTOS unmoeglich, weil das
Kind nur seine eigene Kopie veraendert.

`kernel/src/launch.rs::set_auto_default_path` stellt den BIOS-Zustand wieder
her — `set_boot_drive()` (das vorhandene `Dsetdrv(bootdev)`-Aequivalent) und
Wurzel auf jedem der 16 Laufwerke — **vor jedem** Programm und einmal nach
der Schleife, damit auch der Desktop den Zustand sieht, den EmuTOS' BIOS nach
`autoexec` noch hat.

## 3. Dass es einer war — gemessen, nicht hergeleitet

Zwei AUTO-Programme in Verzeichnisreihenfolge: `AUTOCHD.NCP` (Messwerkzeug,
`method/autochdir.c`, ruft `Dsetpath("\SUB")`) und danach `CURDIR.NCP` aus
DESK18. Drei `PROOF.TXT` mit verschiedenem Inhalt in `C:\`, `C:\SUB\` und
`C:\AUTO\` machen alle drei Kandidaten unterscheidbar.

Kontrollbau (`make_scratch.sh … before`, die drei Dateien dieses Stapels auf
HEAD zurueckgesetzt), `serial/d28-before-serial.log:36-43`:

    AUTOCHD: Dsetpath("\SUB") = 0
    CURDIR: drive C:\SUB
    CURDIR: read "PROOF: C:\SUB"

Produktbau, `serial/d28-after-serial.log:36-43`:

    AUTOCHD: Dsetpath("\SUB") = 0
    CURDIR: drive C:
    CURDIR: read "PROOF: C:\ (root)"

`PROOF: C:\AUTO` erscheint in keinem der beiden Laeufe — und kann es nach §1
auch nicht. Bilder: `screendumps/d28-{before,after}-A-auto-text.png`.

## 4. Pruefung

* `cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat`
  vor **jedem** Board.
* `cargo xtask build --board rpi4` → tos-api 64 / kernel 94 (Basis 64/94).
* `cargo xtask build --board qemu-virt` → tos-api 64 / kernel 94 (Basis 64/94).
* `cargo clippy --features board-rpi4` → 80 / 125 (Basis 80/125), 0 Treffer
  `desk/`.
* `cargo clippy --features board-qemu-virt` → 80 / 125 (Basis 80/125), 0
  Treffer `desk/`.
* `cargo run -p xtask -- test` → 938 passed / 0 failed / 1 ignored (Basis
  938/0/1). Kein Hosttest hinzugefuegt: der Pfad liegt ganz im `no_std`-
  aarch64-`kernel`-Crate, das die Hostsuite nicht erreicht.
* `fsck_msdos -n` auf allen sechs beschriebenen Kartenabbildern: Phase 1/2/3
  ohne Fehler (`build/fsck.txt`).
* 0 `[EXC]`/`[PANIC]` in allen Serienprotokollen.

## 5. Offen

Hardwareabnahme auf der echten Pi 4: Punkt **55** in
`docs/api/desktop/_hw-test-2026-09-17.md`. Der Laufwerksteil von
`set_auto_default_path` (`set_boot_drive`) laeuft in QEMU, aendert dort aber
nie etwas — die QEMU-Karte hat eine Partition, die echte hat C-F.
