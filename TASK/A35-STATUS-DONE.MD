# A35 — Quittung

**Status: ERLEDIGT (Implementierung + Hosttests + QEMU-Gegenmessung)** ·
2026-09-14 · Hardwareabnahme durch den Tester offen.

## Was implementiert ist

`rsrc_load` meldet die Ladung jetzt durch den `AESGLOBAL` des Aufrufers zurück,
wie `rs_readit` (`aes/gemrslib.c:787-788`) und `fix_trindex` (`:595-597`) es
tun. Dafür musste zuerst das entstehen, worauf `ap_ptree` überhaupt zeigen
kann.

### 1. Der NRSC-Baumindex ist ein relokiertes Zeigerfeld (v7/v8)

`tools/mkrsc.py::convert` packt den Baumindex jetzt mit `PTR_FMT` und schreibt
**Byteversätze der Wurzel-OBJECTs** statt Objektindizes — dieselbe Gestalt, die
`frstr` und `frimg` zwanzig Zeilen darüber schon hatten. Formatversion 7 (LP64)
/ 8 (ILP32), Kopf unverändert 68 Byte.

`tos-api/src/aes/resource.rs`:

* neues Gatter `NRSC_VER_TRINDEX_PTR = 7` plus `trindex_stride()`, in der Form
  der beiden vorhandenen (`NRSC_VER_TABLES`, `NRSC_VER_SUBSTRUCTS`);
* `header_is_sound` misst den Index mit der versionsrichtigen Schrittweite;
* `relocate_blob_core` / `relocate_blob_core_ilp32` relozieren die Slots mit
  denselben `fix_nptrs64`/`fix_nptrs32` wie die zwei Nachbartabellen;
* `relocate_and_register_core` liest die Wurzeln ab v7 direkt aus dem
  relozierten Slot — damit ist `rsrc_gaddr(R_TREE, i)` **wörtlich**
  `ap_ptree[i]`, EmuTOS' Bindung in `get_addr` (`gemrslib.c:124`);
* `tree_root()` desgleichen;
* v1–v6 laufen unverändert weiter über die Indexform.

### 2. `rsrc_load` belegt die drei Felder

Neue Funktion `report_load(global: *mut i16)` in `resource.rs`, aufgerufen aus
`rsrc_load` nur bei Erfolg. Sie schreibt `ap_ptree` (u32 @10), `ap_rscmem`
(u32 @14), `ap_rsclen` (u16 @18) aus drei neuen Zellen des `Loaded`-Deskriptors
(`blob_target`, `trindex_target`, `blob_len`), die
`relocate_and_register_core` setzt.

**Nicht übernommen** (wie beauftragt): `rs_sglobe`s Rücklesen
(`gemrslib.c:675-679`). Die Kernel-Statics bleiben die Wahrheit; die drei Worte
sind eine Meldung. Beobachtbare Folge, gemeldet statt geändert: eine Anwendung,
die `ap_rscmem` überschreibt und danach erwartet, dass `rsrc_gaddr` ihr folgt,
bekommt bei uns weiter die eigene Ladung. Kein bekannter Aufrufer tut das;
TeraDesks eigener `xrsrc_*`-Lader hält seine Ressource ganz außerhalb der
kernelseitigen `rsrc_*`.

## Die zwei Punkte, die selbst zu klären waren

### Slotbreite gegen Feldbreite — belegt

Die **Slots** der Tabelle sind aufruferbreit: 8 Byte bei LP64, 4 bei ILP32.
Beleg ist `sdk/gemlib/overlay/gem.h`:

```c
#define _AESrscfile   ((OBJECT **)(unsigned long)*_AESrscfile_p())
```

— das **Feld** wird als 4 Byte gelesen und zu `OBJECT **` verbreitert, also ist
`_AESrscfile[i]` ein natives `OBJECT *`. Damit `ap_ptree[i]` und
`rsrc_gaddr(R_TREE, i)` dasselbe liefern (`get_addr`, `gemrslib.c:124`), muss
der Slot genau diese Breite haben.

Das **Feld** `ap_ptree` (Byte 10) und `ap_rscmem` (Byte 14) bleiben 4 Byte auf
jeder ABI — Entscheidung vom 2026-09-13, weil `AESGLOBAL` doppelt adressiert
wird.

Ja, die Slotbreite hängt an der **Aufrufer-ABI**, nicht an der Dateiversion —
der Loader reloziert ohnehin nach `abi`, wie bei frstr/frimg.

### Der EL1-Pfad — ehrliche 0, belegt

`ap_ptree`/`ap_rscmem` sind vier Byte. Sie können nur einen Blob benennen, der
im Adressraum des Aufrufers unter 4 GiB liegt.

* **EL0:** gilt bauartbedingt (Prozessfenster unter `USER_TPA_MAX_VA` = 32 MiB;
  TeraDesks 68 rsrc-Aufrufe gemessen bei `0x4000ac … 0x409381`).
* **EL1:** gilt nicht. `RSC_BUF` ist eine Kernel-High-Half-Statik; die unteren
  32 Bit einer solchen VA sind die physikalische Adresse, und die erreicht
  unter EL1 nichts — nach dem High-Half-Umschalten bildet `TTBR0_EL1` ohne
  Prozess nichts ab (`kernel/src/arch/aarch64/mmu.rs`, `install_stage2_ttbr1`
  setzt `EMPTY_L0`). Eine abgeschnittene Adresse wäre also nicht ungenau,
  sondern falsch.

Also: der EL1-Pfad schreibt **keins** der drei Felder; der Aufrufer behält
seine Nullen, EmuTOS' Lesart „nichts geladen" (`gemrslib.c:685`). Die Ladung
gelingt weiter, `rsrc_gaddr` antwortet in voller Breite. Das steht im Kommentar
an `report_load`, im Modulkopf und in der Spec.

## Was am Auftrag falsch war

**Die drei `#[ignore]`-Tests konnten unter der eigenen Randbedingung des
Auftrags nicht grün werden.** Der Auftrag verbietet die abgeschnittene
Kernel-Adresse *und* verlangt die drei Tests grün — beides zusammen ist auf
diesem Host unerfüllbar:

* Die Schablone fährt den EL1-Pfad (kein Prozess ⇒ `RSC_BUF`).
* Auf macOS/arm64 liegt **keine** Adresse unter 4 GiB: `__PAGEZERO` belegt die
  ersten 4 GiB. Gemessen: `nm` auf die Testbinärdatei ergibt `RSC_BUF` bei
  `0x1_0010_7ef8`.
* `rsrc_load_reports_ap_rscmem` verlangte `ap_rscmem <= ted < ap_rscmem + len`
  mit `ted` aus `rsrc_gaddr` — also die **volle** 64-Bit-Adresse in einem
  4-Byte-Feld. Auch eine Trunkierung hätte den Test nicht grün gemacht.
* `rsrc_load_reports_ap_ptree` dereferenzierte `ap_ptree` als Hostzeiger.

Aufgelöst so: der **darstellbare** Zweig wird dort geprüft, wo er erreichbar
ist — in `tos-api/src/aes/resource.rs` (`mod tests`), über genau die
Access/Target-Teilung, die `load_into_user` benutzt (`access_base` = eigener
Puffer, `target_base` = die Benutzer-VA). Die Integrationsdatei prüft den
Verweigerungszweig **und** die Bindung `ap_ptree[i] == rsrc_gaddr(R_TREE, i)`
durch den echten Dispatcher. Die `#[ignore]` sind weg; die drei Tests heißen
jetzt nach dem, was sie belegen.

**Die Sollzahl 504/0/4 ist veraltet.** Dieser Baum stand vor meiner Änderung
bei **530 grün / 0 rot / 4 ignoriert** (M08 hat inzwischen 26 Referenzfälle
verdrahtet). Nachher: **538 / 0 / 1**.

## Nebenwirkung, mitgefixt

Der 68-Byte-Kopf liegt bei 4 mod 8. Ein LP64-Baumindex aus 8-Byte-Slots hätte
das OBJECT-Feld auf 4 mod 8 gelegt — und der Loader dereferenziert es als
`repr(C)`-`Object` (Rusts Ausrichtungsprüfung hat es im ersten Testlauf sofort
gemeldet). `mkrsc.py` polstert den Index deshalb auf die Satzausrichtung. Die
Fehlausrichtung **bestand vorher schon** bei jeder Ressource mit einer
Baumzahl, die 8 nicht teilt: `desktop.rsc` off_obj 172, `qed.rsc` 196. Für alle
drei ausgelieferten Blobs ist sie damit behoben.

## Mitgezogen

* `tools/xtask/src/rsc_align_tests.rs` bestimmte die NRSC-Zeigerbreite mit
  `version == 5` statt nach Paritäts­regel und wurde durch v7 rot (3 Fälle).
  Jetzt `nrsc_ptr_width()`.
* `kernel/src/selftest_objc.rs::run_rsrc` baut einen **v1**-Blob mit
  `u32`-Index — **unverändert lauffähig**, aktiv nachgeprüft und zusätzlich als
  Modultest (`a_pre_v7_blob_still_loads_and_publishes_nothing`, v1 **und** v5)
  abgesichert. `run_rsrc_file`/`run_rsrc_gem` laden von Karte, also die neuen
  Blobs — sie haben weiterhin keinen Aufrufer. `selftest_aes.rs` prüft die
  Worte 5..9 auf „unberührt" nach `appl_init` und fasst `rsrc_load` nicht an,
  bleibt also gültig.
* Kein Referenzfall der Dumper-Tabellen stellt Opcode 110 (geprüft), die
  AES-Referenzsuite ist unberührt.

## Neu gebaute Ressourcen

| Datei | vorher | nachher |
|---|---|---|
| `tos-api/src/aes/gem_fsel.nrsc` (eingecheckt) | v5, 4975 B | v7, 4991 B |
| `sdk/apps/qed/build/qed.rsc` | v5, 41190 B | v7, 41322 B |
| `sdk/apps/teradesk/build/dist/desktop.rsc` | v5, 39114 B | v7, 39222 B |

`icons.rsc` ist native-RSHDR (anderes Format, eigener Baumindex, schon
zeigerbasiert) und unverändert. Ein byteweiser Suchlauf über den Baum fand
keinen weiteren NRSC-Blob.

## Belege

* **Rot vorher:** `cargo test -p tos-api --test aes_rsrc_global_fields --
  --ignored` → `0 passed; 3 failed` (`ap_ptree ... is still 0`, `ap_rscmem ...
  is still 0`, `ap_rsclen ... left: 0 right: 4975`).
* **Rot vorher, neuer positiver Zweig:** `report_load` testweise von Hand
  stillgelegt → `an_el0_load_reports_all_three_fields` und
  `ap_ptree_slots_are_what_rsrc_gaddr_returns` rot (`left: 0, right: 4194372`);
  von Hand zurückgebaut, danach grün.
* **`cargo run -p xtask -- test`: 538 grün / 0 rot / 1 ignoriert.**
* Beide Boards `--release` frisch gebaut (`Compiling tos-api` + `Compiling
  kernel` in beiden Protokollen). Warnungen tos-api 64 / kernel 94, Clippy
  tos-api 81 / kernel 125 — Delta 0.
* **QEMU `raspi4b`, headless, 640×480:** QED und TeraDesk gestartet, beide
  Schirme **bytegleich** zur Referenz vom 2026-09-13.
  `measurement-2026-09-14-a35/`.

## Offen

* **Maschinenbeleg für die drei Felder selbst.** Ein Screendump zeigt nicht,
  was in `global[5]/[7]/[9]` steht. Der darstellbare Zweig ist hostseitig
  bewiesen und auf der Maschine nur *indirekt* belegt (QED und TeraDesk laufen
  weiter). Ein direkter Beleg bräuchte eine Diagnose, die `rsrc_load` fährt und
  die drei Worte druckt — `sdk/apps/apitest/apitest.c` wäre der Ort; **nicht**
  hinzugefügt, weil außerhalb des Auftrags.
* **Interaktiv auf echter Pi 4.** TeraDesk ist der Leser dieser Felder
  (`icon.c:2692,2707-2708`); Ikonen anklicken, Fenster öffnen, Dateidialoge.
  QEMU `raspi4b` gibt Eingabe oder C:-Speicher, nicht beides.
* Nicht committet (unversionierte Fremdarbeit M08 in denselben Verzeichnissen).
