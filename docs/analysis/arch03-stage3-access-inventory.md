# ARCH03 vor Stufe 3 — Inventur aller Kernel-Zugriffe auf Programmspeicher

Stand: Zweig `arch03`, HEAD `2988255` (nach Stufe 2). Verfasser: Auditor
(Bericht inline geliefert, vom Lead 2026-10-05 hier abgelegt). Scratch-Bau
rpi4 release sauber (64/94), tos-api-Lib-Tests gruen. Bad-Pointer-Verhalten
aus den Messungen `measurement-2026-10-05-arch03-0c` (CRASHTST `c`/`p`) und
APITEST T31 belegt, nicht neu gefahren. F3/F4 brauchen einen zweiten AES-Client
und sind nicht nachgestellt.

## Mechanismen heute

1. **`uaccess` mit Fixup (Zielzustand):** `Cconws` (`gemdos/console.rs:537`);
   `handle_pexec` Pfad/Kommandozeile/Umgebung (`exceptions.rs:1737/1759`,
   EL1-Aufrufer ueber `read_kernel_cstr`).
2. **`ldtr/sttr` ohne Fixup** (EL0-Rechtepruefung richtig, schlechter Zeiger =
   Kernel-Halt): `Cconrs` (`console.rs:292/313`), `Ssystem`
   (`ssystem.rs:100/173/205`), `Setexc` (`bios/vectors.rs:110/133/154`),
   `conterm`-Faltung (`bios/chardev.rs:137/155`).
3. **`resolve_kva` / `kalias` + Regionspruefung (Zielmuster):** `Setscreen`
   (`xbios/screen.rs:181/277`); `ObjTree::new`/`app_owned`
   (`aes/object.rs:119/179/535/571`, `aes/window.rs:480`); `rsrc_load`/CICON
   (`aes/resource.rs:2346/2370`); `Getmpb`/`Getbpb` (`getmpb.rs:114`,
   `bpb.rs:300`); `WF_SCREEN` (`window.rs:3932`); Loader Env/Basepage
   (`loader.rs:1413/1496`); Seite 0 ueber `page0_kva` (`chardev.rs:76`,
   `tick.rs:107`, `keytbl.rs:218`, `kbdvecs.rs:202`, `exceptions.rs:786`).
4. **Direkter privilegierter Zugriff ueber TTBR0** — die Mehrheit, nur durch
   VA = PA und fehlendes PAN tragfaehig; schlechter Zeiger = Kernel-Abbruch.
5. **Kernel-eigen, keine Aenderung:** `desk/*` LP64-Baeume, `mousexvec`
   (nur VDI-Code), Loader-TPA ueber `kalias`, `gemdos-trace` (Feature).

## Inventar (Datei:Zeile · Objekt · Eigentuemer · heute → Stufe 3)

### A. GEMDOS
- `console.rs:537` Cconws · Aufrufer · uaccess+Fixup · OK
- `console.rs:292/313/476` Cconrs · Aufrufer · ldtr/sttr o. Fixup → uaccess+Fixup
- `fs/mod.rs:23` `cstr` (Fopen/Fcreate/Fdelete/Fattrib/Frename/Dcreate/
  Ddelete/Dsetpath/Fsfirst; `file.rs:156/212/559/625/718/722`,
  `dirops.rs:30/123/165`, `find.rs:456`) · direkt → uaccess **mit EL-Zweig (F1)**
- `file.rs:341` Fread, `:412` Fwrite, `:884` Fdatime, `:975/993` Konsole ·
  direkt → copy_to/from_user
- `find.rs:382/420/436` DTA · direkt → uaccess (`cur_dta` ueber `current_kva` OK)
- `dirops.rs:253` Dgetpath, `drive.rs:87` Dfree · direkt → uaccess
- `ssystem.rs:100/173` · ldtr/sttr o. Fixup → +Fixup
- `trace.rs` `copy_cstr` (Feature) · direkt → uaccess

### B. BIOS
- `vectors.rs:110` Setexc (Seite 0) · ldtr/sttr o. Fixup → +Fixup
- `bios/mod.rs:191` Rwabs-Puffer · direkt → kalias+Region oder uaccess
- `getmpb.rs:85/122` MPB schreiben · direkt → uaccess; `:114` expose OK
- `chardev.rs:137` shift_fold · ldtr o. Fixup → +Fixup; `:76` OK
- `tick.rs:107` `_hz_200` · OK

### C. XBIOS
- `screen.rs:277` Setscreen · resolve_kva · OK
- `screen.rs:313` Setpalette · direkt → uaccess/kalias
- `floppy.rs:138` Protobt · direkt → uaccess
- `keytbl.rs:205` Keytbl-Tabellen · direkt → uaccess/kalias; Rueckgabe OK
- `kbdvecs.rs:202` · OK

### D. AES-Parameterblock und Arrays
- `aes/mod.rs:299` `Aespb::from_raw` · direkt → kalias (Plan §3.6)
- `copy_intout_back` · direkt → kalias
- `appl.rs:155` AESGLOBAL · direkt → kalias
- addrin[0]-Ziele: `appl_find` (`appl.rs:366`), `appl_write` (`:417`),
  `appl_read` (`:485`), `evnt_mesag` (`event.rs:522`), `evnt_multi` (`:829`),
  `form_alert` (`form.rs:138`), `fsel` (`fsel.rs:546/550/551`), `graf_mouse`
  (`graf.rs:647`), `menu_bar` (`menu.rs:339`), `menu_text` (`:1066/1068`),
  `menu_register`, `rsrc_load` (`resource.rs:2199`), `scrp_*`/`shel_*`
  (`shell.rs:232/241/362/482/892/909/953/1114/1129`), `wind_set`-String
  (`window.rs:3441/3443`) · direkt → kalias bzw. uaccess

### E. OBJECT-Baeume und Unterstrukturen
- `object.rs` `ObjTree::new`/`app_owned` mit `ua`/`owner_kva` · OK
- `object.rs:142` `ObjTree::lp64` · Kernel · OK
- `object.rs:2736/2790` G_USERDEF (E10-Muster vorhanden) · OK
- **`object.rs:1937`, `:2070/2086/2132`** `objc_edit`-Schreiber auf `te_ptext`
  direkt · **F2**
- **`menu.rs:1077`** `menu_text`-Kopie direkt · **F2**
- `resource.rs:2916` `rsrc_obfix` · OK

### F. Ressourcen und AESGLOBAL
- `resource.rs:2346/2370` · OK
- `resource.rs:2885` `rsrc_saddr` · blob-intern, pruefen
- `resource.rs:744/2493` AESGLOBAL `AP_RSCMEM`/`AP_PTREE` · direkt → kalias/uaccess

### G. VDI
- `vdi/mod.rs:107` `Vdipb::from_raw` · direkt → kalias/uaccess
- `vdi/mod.rs:144/165` CONTRL-Zeiger · direkt
- `raster.rs:103` MFDB `fd_addr`-Blits · nur Groessen-, keine Abbildungspruefung → kalias+Region
- `mouse.rs:918` `vex_butv/motv/curv/timv` speichern EL0-VA, kein Aufruf · **F4**;
  `vex_wheelv` korrekt

### H. Upcalls
- `deliver_upcall`, `deliver_pending_kbdvec`, `read_userdef_ret_slot`,
  `etv_term`, `mousexvec` · OK

### I–K. Umgebung, Basepage, Seite 0, Loader, Accessories, Desktop
- Env/Basepage/Seite 0/Loader/Accessories · OK (kalias/page0_kva)
- **`desk/dosbind.rs`** ruft GEMDOS mit Kernel-VAs, `fs` liest roh · **F1**
- **`menu.rs:447/459/740`** `track`/`track_owned`/`rehilite`/Submenue mit
  `ObjTree::new` (owner = current) statt `mntree_accessor` (`:1239`) · **F3**

## Befunde

- **F1 (bestaetigt, strukturell):** `TrapArgs` traegt die Aufrufer-EL nicht;
  die Dateioberflaeche kennt keinen EL-Zweig. Auf uaccess umgestellt, wuerden
  Desktop (`desk/dosbind.rs`) und AES-Fiber mit Kernel-VAs faulten.
  **Voraussetzung Nr. 1 fuer Stufe 3.**
- **F2 (bestaetigt):** `objc_edit` und `menu_text` schreiben am `ua`-Pfad
  vorbei; ein schlechter `te_ptext` haelt heute den Kernel an.
- **F3 (plausibel):** Menue-Tracking liest den installierten Balken mit den
  Rechten des aktuellen Prozesses statt des Besitzers (MNPPD).
- **F4 (plausibel):** `vex_butv/motv/curv/timv` speichern Anwender-Handler,
  rufen sie aber nirgends; unter Stufe 3 E10/Upcall wie `vex_wheelv` oder
  dokumentierter No-op.
- **F5 (niedrig):** vier `ldtr/sttr`-Stellen ohne Fixup (Cconrs, Ssystem,
  Setexc, conterm) und die MFDB-Blits — schlechter Zeiger = Kernel-Halt.

## Bricht unter Stufe 3

1. alle `fs::cstr`-Pfade und `fs`-Puffer vom Desktop/AES-Fiber (F1)
2. `menu::track`/`rehilite`/Submenue (F3)
3. `objc_edit`/`menu_text` bei fremdem Besitzer (F2)
4. MFDB-Blits auf fremde `PROT_P`-Puffer
5. `vex_*` falls je aufgerufen (F4)

Bereits abgesichert: AES-Baeume (`ObjTree`), WF_NEWDESK, `rsrc_load`, Env/
Basepage/Seite 0, G_USERDEF, Setscreen, Getmpb/Getbpb/WF_SCREEN, Accessory-Titel.

## Empfohlene Reihenfolge

1. Aufrufer-EL in die Trap-Dispatch (`TrapArgs`, `rust_sync_handler`).
2. GEMDOS-Pfade/Puffer auf uaccess + EL1-Kernel-VA-Zweig (F1).
3. BIOS/XBIOS-Direktzugriffe; die vier F5-Stellen in die Fixup-Tabelle.
4. AES-/VDI-Parameterbloecke, MFDB-`fd_addr`, CONTRL-Zeiger: kalias+Region.
5. `objc_edit`, `menu_text`, AESGLOBAL-Schreiber ueber `ua`/kalias (F2).
6. `menu::track` auf `mntree_accessor` (F3).
7. `vex_*`: Upcall oder dokumentierter No-op (F4).

## Zaehlung

~72 Zugriffsstellen (GEMDOS ~17, BIOS ~6, XBIOS ~5, AES ~33, VDI ~4, Upcall ~6,
Loader/Env/Seite 0 ~10); ~24 schon stufe-3-tauglich, ~44 umzustellen.

## Nebenbeobachtungen

- `Setscreen` nicht-atomarer Bildwechsel (bekannt, unabhaengig).
- `tos-types/src/memory.rs:155-168` verwechselt Mxalloc- und PRGFLAGS-
  Schutznibble (Stufe-3-Thema).
- `selftest_sleepwake_switch` scheitert vorbestehend mit 5 Pruefungen.
