# AES 1:1-Audit — PAKET P6 (resource / fileselector / scrap / shell)

**Scope (ein Satz):** Abgleich von `tos-api/src/aes/resource.rs`, `fsel.rs`,
`shell.rs` (rsrc/fsel/scrp/shel) gegen
`references/emutos/aes/{gemrslib,gemfslib,gemsclib,gemshlib}.c` + Header,
Default-Prinzip 1:1 EmuTOS.

**Methode:** rein statisch, Quelle-gegen-Quelle, Zeile-für-Zeile; kein
Build/QEMU (5 parallele Auditoren — Contention). Alle `Runtime-verify: yes`-Items
sind reasoned, nicht reproduziert.

**Referenzkonfiguration:** Voll-Build → `CONF_WITH_PCGEM=1`,
`CONF_WITH_COLOUR_ICONS=1` (config.h:1266,1319), `CONF_WITH_SHUTDOWN=0` für
Nicht-Atari-HW (config.h:2028-2034). Damit sind `sc_clear`/`sh_rdef`/`sh_wdef`
reale Funktionen; `SHW_SHUTDOWN` ist wegkompiliert.

---

## Teil 1 — Coverage-Tabelle

### RSRC (gemrslib.c)
| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| rs_load | gemrslib.c:826 | resource.rs:657 rsrc_load | partial (eigene Suche statt sh_find) | nein |
| rs_free | gemrslib.c:685 | resource.rs:824 rsrc_free | impl | ~ja |
| rs_gaddr/get_addr | gemrslib.c:112,715 | resource.rs:875 rsrc_gaddr | **partial (6 von 17 R_*-Typen)** | nein |
| rs_saddr | gemrslib.c:728 | resource.rs:958 rsrc_saddr | **STUB** | nein |
| rs_obfix/fix_chpos | gemrslib.c:53,89 | resource.rs:986,1018 | impl | ja |
| rs_readit | gemrslib.c:751 | resource.rs:755 load_into_user | impl (Nativ-Format) | n/a |
| fix_trindex | gemrslib.c:591 | resource.rs:223-230 | impl | ~ja |
| fix_objects | gemrslib.c:606 | resource.rs:438,315 relocate_*_core | partial (G_CICON/G_USERDEF fehlen) | nein |
| fix_tedinfo_std | gemrslib.c:655 | resource.rs:462-470 | impl | ~ja |
| fix_nptrs (IB/BB/FRSTR/FRIMG) | gemrslib.c:640,799-805 | resource.rs:479-498 | impl | ~ja |
| fix_cicons/fixup_all_ciconblks/transform_all_cicons | gemrslib.c:549,226,403 | — | **MISSING** | nein |
| fix_long/fix_ptr | gemrslib.c:576,649 | resource.rs:538,299 reloc/reloc32 | impl | ja |

### FSEL (gemfslib.c)
| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| fs_input | gemfslib.c:609 | fsel.rs:247 run | partial | nein |
| fs_input(label) | gemfslib.c:691 | fsel.rs:243 fsel_exinput | partial (Titel ignoriert) | nein |
| fs_active (Read+Sort) | gemfslib.c:268 | fsel.rs:321 relist | partial (kein Sort) | nein |
| fs_comp / Shell-Sort | gemfslib.c:243,312 | — | **MISSING** | nein |
| fs_add ('\007'-Ordner-Präfix) | gemfslib.c:249 | — | MISSING | nein |
| '.'/'..'-Filter | gemfslib.c:298 | — | MISSING (werden angezeigt) | nein |
| Ordner unabhängig von Maske | gemfslib.c:300 | — | MISSING (Maske filtert Ordner) | nein |
| fs_newdir | gemfslib.c:487 | fsel.rs:321,347 relist/refill | impl | ~ja |
| fs_format | gemfslib.c:363 | fsel.rs:347 refill | impl | ~ja |
| fs_sel | gemfslib.c:413 | fsel.rs:652-654 | impl | ~ja |
| fs_start (3D) | gemfslib.c:59 | fsel.rs:112 set_3d_flags | partial (nur Flags, keine Geometrie) | ~ |
| FCLSBOX-Up-Nav | gemfslib.c:839 | — | MISSING | nein |

### SCRP (gemsclib.c)
| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| sc_read | gemsclib.c:40 | shell.rs:115 scrp_read | impl | ja |
| sc_write | gemsclib.c:57 | shell.rs:124 scrp_write | impl | ja |
| sc_clear | gemsclib.c:74 | shell.rs:134 scrp_clear | **STUB (ret 0)** | nein |

### SHEL (gemshlib.c)
| EmuTOS fn | EmuTOS file:line | NewTOS fn | Status | 1:1? |
|---|---|---|---|---|
| sh_read | gemshlib.c:102 | shell.rs:157 shel_read | partial (kein Producer → leer) | nein |
| sh_write | gemshlib.c:157 | shell.rs:184 shel_write | partial (Modi + Launch nie ausgeführt) | nein |
| sh_get | gemshlib.c:203 | shell.rs:219 shel_get | impl (geclampt) | ~ja |
| sh_put | gemshlib.c:213 | shell.rs:236 shel_put | impl (geclampt) | ~ja |
| sh_find/findfile | gemshlib.c:406,474 | shell.rs:274 shel_find | partial (2 von 4 Schritten) | nein |
| sh_envrn | gemshlib.c:327 | shell.rs:326 shel_envrn | impl (interne Env statt DOS-Env) | ~ |
| sh_rdef | gemshlib.c:491 | shell.rs:377 shel_rdef | impl | ja |
| sh_wdef | gemshlib.c:504 | shell.rs:392 shel_wdef | impl | ja |
| sh_path (PATH-Iterator) | gemshlib.c:356 | — | MISSING | nein |

---

## Teil 2 — Findings

### AUD-P6-001: rsrc_saddr — vollständiger STUB, ignoriert alle Argumente
- Severity: **BLOCKER**
- EmuTOS: gemrslib.c:728-742 (`rs_saddr`: `psubstruct = get_addr(rtype,rindex); if (psubstruct != -1) { *psubstruct = rsaddr; return TRUE; } return FALSE;`)
- NewTOS: resource.rs:958-960 (`pub fn rsrc_saddr(pb){ pb.set_ret(0); }`)
- Divergenz: `rsrc_saddr` (Opcode 113) tut nichts, schreibt keinen Zeiger, liefert immer 0/FALSE. Ein Programm, das z. B. einen Free-String oder ein TEDINFO-`te_ptext` per `rsrc_saddr(R_STRING/R_TEPTEXT, idx, ptr)` auf einen eigenen Puffer umbiegt, hat keinerlei Wirkung und bekommt Fehlschlag gemeldet. Verstoß gegen die No-Stub-Regel.
- 1:1-Fix: `get_addr(rtype,rindex)`-Äquivalent implementieren (Slot-Adresse für R_TREE/R_OBJECT/R_STRING/R_IMAGEDATA/R_FRSTR/R_FRIMG/… bestimmen), den übergebenen Zeiger `addr_in[0]` (caller-ABI-Breite) in den Slot schreiben, TRUE/FALSE nach Gültigkeit des **Typs** zurückgeben.
- Runtime-verify: yes

### AUD-P6-002: rsrc_gaddr — 11 von 17 R_*-Typen fehlen, liefern still 0/not-found
- Severity: **BLOCKER**
- EmuTOS: gemrslib.c:112-176 (`get_addr` behandelt R_TREE(0)…R_FRIMG(16)), aesdefs.h:578-594 (Typtabelle)
- NewTOS: resource.rs:66-75, 875-931 (nur R_TREE 0, R_OBJECT 1, R_STRING 5, R_IMAGEDATA 6, R_FRSTR 15, R_FRIMG 16; `_ => 0` bei 926)
- Divergenz: `rsrc_gaddr` löst **R_TEDINFO(2), R_ICONBLK(3), R_BITBLK(4), R_OBSPEC(7), R_TEPTEXT(8), R_TEPTMPLT(9), R_TEPVALID(10), R_IBPMASK(11), R_IBPDATA(12), R_IBPTEXT(13), R_BIPDATA(14)** nicht auf und gibt `addr_out[0]=0`, `int_out[0]=0` zurück. EmuTOS liefert die echte Struct-/Sub-Zeiger-Adresse. Ein konformes Programm, das `rsrc_gaddr(R_TEDINFO, n, &ted)` macht und danach `ted` dereferenziert, arbeitet auf einem Null-Zeiger → falsches Ergebnis/Crash. Das Nativ-Format rechtfertigt das nicht: die TEDINFO/ICONBLK/BITBLK-Array-Basen könnten wie frstr/frimg mitgeführt werden.
- 1:1-Fix: `Loaded` um die Array-Basen (tedinfo/iconblk/bitblk) + Zähler erweitern (mkrsc.py muss die Offsets im NRSC-Header ablegen), und die fehlenden `get_addr`-Zweige samt Sub-Zeiger-Aliasing (R_TEPTEXT=&ted.te_ptext usw., R_OBSPEC=&obj.ob_spec) implementieren.
- Runtime-verify: yes

### AUD-P6-003: Fileselector sortiert nicht (Ordner-zuerst + alphabetisch fehlt)
- Severity: **HIGH**
- EmuTOS: gemfslib.c:249-258 (`fs_add`: `\007`-Präfix für Ordner, `' '` für Dateien), :312-326 (Shell-Sort per `fs_comp`=strcmp) → Ordner sortieren wegen 0x07 < 0x20 immer nach oben, danach alphabetisch
- NewTOS: fsel.rs:321-343 (`relist` übernimmt `list_dir`-Reihenfolge), find.rs:210-232 (`list_dir` scannt in roher FAT-Slot-Reihenfolge, kein Sort)
- Divergenz: Die Dateiliste erscheint in physischer Verzeichnis-Reihenfolge statt sortiert; Ordner sind nicht gruppiert. Sichtbar bei jedem Öffnen des Selektors mit mehr als einem Eintrag.
- 1:1-Fix: In `relist` nach dem Einlesen mit dem 0x07/0x20-Präfix-Schlüssel (Ordner zuerst) + `strcmp` sortieren, wie `fs_active`.
- Runtime-verify: yes (Screendump: Eintragsreihenfolge)

### AUD-P6-004: Fileselector versteckt Ordner bei Maske ≠ `*.*`
- Severity: **HIGH**
- EmuTOS: gemfslib.c:298-304 — liest per `dos_sfirst(dir\*.*, FA_SUBDIR)` **alle** Einträge und behält `if ((d_attrib & FA_SUBDIR) || wildcmp(pspec, d_fname))`: Ordner **immer**, Dateien nur bei Maskentreffer
- NewTOS: fsel.rs:326-330 (`list_dir(dpath)` mit `dpath` inkl. Maske), find.rs:69,175,210 (`fcb_pattern(comp)` + `pattern_match` filtert **auch** Verzeichnisnamen gegen die Maske)
- Divergenz: Bei einer Maske wie `*.C`/`*.TXT` matcht kein Ordnername die Erweiterung → Ordner verschwinden komplett aus der Liste, der Nutzer kann nicht mehr in Unterverzeichnisse navigieren. EmuTOS zeigt Ordner unabhängig von der Maske.
- 1:1-Fix: In `relist` mit `*.*` listen und die Maske nur auf Nicht-Verzeichnis-Einträge anwenden (Ordner immer aufnehmen).
- Runtime-verify: yes

### AUD-P6-005: shel_write(SHW_EXEC) — Launch wird aufgezeichnet, aber nie per Pexec ausgeführt
- Severity: **HIGH**
- EmuTOS: gemshlib.c:168-176 (SHW_EXEC setzt `D.s_cmd`), :592-689 (`sh_ldapp`) + :725 → nach Terminierung des aktuellen Programms läuft die `dos_exec(PE_LOADGO, D.s_cmd, …)`-Schleife
- NewTOS: shell.rs:184-215 (`shel_write` speichert `next_cmd/next_tail/next_doex`), shell.rs:419 (`pending_launch`) — kein Konsument: grep findet keinen Aufrufer von `pending_launch()` außerhalb der Definition
- Divergenz: `shel_write` liefert korrekt 1, das aufgezeichnete Programm wird jedoch nie gestartet. GEM-Apps, die per `shel_write(SHW_EXEC)` + `Pterm` ein Folgeprogramm ketten (Installer, Launcher, Konvertierer), führen dieses Programm nie aus. Bekannter Architektur-Aufschub (Shell-Loop-Wiring), aber durch die API beobachtbar.
- 1:1-Fix: Shell-Loop/Launcher an `pending_launch()` anbinden: bei `SHW_EXEC` nach `Pterm` des aktuellen Prozesses den aufgezeichneten Befehl via `sh_find` + `Pexec` starten.
- Runtime-verify: yes (echte HW: App-Ketten)

### AUD-P6-006: shel_read liefert immer den Leer-String (kein Producer)
- Severity: **HIGH**
- EmuTOS: gemshlib.c:102-106 (`sh_read` kopiert `D.s_cmd`/`ad_stail`), gesetzt vom Loader vor `dos_exec` (:604-605,645)
- NewTOS: shell.rs:157-164 (`shel_read` liest `launch_cmd`/`launch_tail`); grep: **kein Schreiber** von `S.launch_cmd`/`launch_tail`. Der Launcher (kernel/src/launch.rs:85) setzt nur `resource::set_app_dir`, nicht die Shell-Puffer.
- Divergenz: Jedes Programm, das per `shel_read` seinen Startbefehl + Command-Tail abfragt (z. B. um Argumente/Startpfad zu ermitteln), erhält zwei leere Strings. Observable Fehlfunktion.
- 1:1-Fix: Beim Launch `launch_cmd` = voller Programmpfad, `launch_tail` = 128-Byte-Command-Tail füllen (Producer im Loader), analog EmuTOS `D.s_cmd`/`ad_stail`.
- Runtime-verify: yes

### AUD-P6-007: Fileselector zeigt `.` und `..` als Einträge (EmuTOS filtert führenden Punkt)
- Severity: **MEDIUM**
- EmuTOS: gemfslib.c:298 (`if (D.g_dta.d_fname[0] != '.')` — überspringt `.`, `..` und alle Dotfiles; Up-Navigation nur via FCLSBOX)
- NewTOS: fsel.rs:329-341 (`relist`: keine Punkt-Filterung), find.rs:142-178 (`scan` gibt `.`/`..` zurück, da `pattern_match(11×'?')` sie trifft); fsel.rs:415-461 (`nav_into_folder` behandelt sie bewusst)
- Divergenz: In Unterverzeichnissen erscheinen zwei zusätzliche Einträge (`.` als No-Op-Ordner, `..`), die EmuTOS nie anzeigt. Sichtbarer Listenunterschied.
- 1:1-Fix: In `relist` Einträge mit `name[0]=='.'` überspringen (und stattdessen den FCLSBOX-Weg für „nach oben" liefern, siehe AUD-P6-013).
- Runtime-verify: yes

### AUD-P6-008: fsel_exinput ignoriert den benutzerdefinierten Titel (addr_in[2])
- Severity: **MEDIUM**
- EmuTOS: gemfslib.c:691-693 (`obj->ob_spec = pilabel ? pilabel : rs_str(ITEMSLCT); centre_title(...)`), Dispatcher gemsuper.c:326 (`FS_TITLE = addr_in[2]`)
- NewTOS: fsel.rs:243-245 (`fsel_exinput` ruft nur `run(pb)`), run() liest `addr_in[2]` nie und setzt FSTITLE nicht
- Divergenz: Der Selektor zeigt statt des App-Titels (z. B. „Bild laden") immer die GEM.RSC-Default-Beschriftung. `fsel_exinput` degeneriert zu `fsel_input` und ignoriert ein deklariertes Argument.
- 1:1-Fix: In `run` ein optionales `label = addr_in[2]` durchreichen; wenn nicht-null FSTITLE-`ob_spec` darauf setzen und zentrieren, sonst „Item Selector".
- Runtime-verify: yes

### AUD-P6-009: shel_find — nur 2 von 4 Suchschritten, falsche Präzedenz
- Severity: **MEDIUM**
- EmuTOS: gemshlib.c:406-472 (`findfile`: (1) App-Dir `rlr->p_appdir` **zuerst**, (2) aktuelles Verzeichnis, (3) Laufwerkswurzel, (4) AES-`PATH=`)
- NewTOS: shell.rs:274-311 (nur Schritt (2) Pfad-wie-gegeben + (3) Laufwerkswurzel; App-Dir + PATH fehlen)
- Divergenz: Eine Ressource/Datei, die im App-Verzeichnis liegt, wird von EmuTOS bevorzugt gefunden (Schritt 1 vor 2); NewTOS sucht App-Dir gar nicht (obwohl `resource::APP_DIR` existiert) und durchsucht die AES-`PATH=` nicht. Andere Fund-Präzedenz und Fehlschlag für PATH-erreichbare Programme.
- 1:1-Fix: App-Dir als Schritt (1) voranstellen, `PATH=`-Iteration (`sh_path`) als Schritt (4) ergänzen; auf `Fsfirst`-Existenzprobe statt `read_file` umstellen (Wildcards/Dir-Treffer).
- Runtime-verify: yes

### AUD-P6-010: shel_envrn liest interne hartcodierte Env statt der DOS-Umgebung
- Severity: **MEDIUM**
- EmuTOS: gemshlib.c:327-348 (`sh_envrn` scannt `ad_envrn`, die reale AES/DOS-Env)
- NewTOS: shell.rs:21 (`DEFAULT_ENV = "PATH=C:\\;\0\0"`), 326-370 (scannt internen `env`-Puffer, defaultet auf DEFAULT_ENV)
- Divergenz: `shel_envrn("PATH=")` liefert stets `C:\;` statt der tatsächlichen Umgebung. ABI-Breiten-Handling des Ergebniszeigers (`abi.write_ptr`) ist korrekt und matcht die Slot-Semantik `addr_in[0]`=`char**`/`addr_in[1]`=Suchstring (Dispatcher gemsuper.c:396) — der Slot-Swap ist also richtig; nur die **Quelle** der Env divergiert.
- 1:1-Fix: An eine echte, vom Loader befüllte Prozess-Umgebung (`ad_envrn`-Äquivalent) anbinden; bis dahin ist die Default-Env eine dokumentierte Näherung, keine 1:1-Treue.
- Runtime-verify: yes

### AUD-P6-011: rsrc_load nutzt eigene 3-Pfad-Suche statt sh_find
- Severity: **MEDIUM**
- EmuTOS: gemrslib.c:835-836 (`strcpy(tmprsfname,rsfname); if (!sh_find(tmprsfname)) return FALSE;`) — Lokalisierung über die volle Shell-Suchkette
- NewTOS: resource.rs:702-737 (Schleife über `[app_dir, "C:\\GEMSYS\\RSC\\", "C:\\"]`)
- Divergenz: NewTOS durchsucht App-Dir, `C:\GEMSYS\RSC\`, `C:\` — nicht das aktuelle Arbeitsverzeichnis und nicht die AES-`PATH=`. Eine RSC im CWD (aber nicht im App-Dir) wird von EmuTOS gefunden (sh_find Schritt 2), von NewTOS ggf. nicht.
- 1:1-Fix: `rsrc_load` auf denselben `shel_find`-Pfad umstellen (nach Behebung von AUD-P6-009), statt einer parallelen Ad-hoc-Suchliste.
- Runtime-verify: yes

### AUD-P6-012: Relocation behandelt G_CICON und G_USERDEF nicht
- Severity: **MEDIUM**
- EmuTOS: gemrslib.c:606-637 (`fix_objects` relociert per **Ausschluss**: G_BOX/G_IBOX/G_BOXCHAR nicht, G_CICON via `ob_spec = ciconblkptr[ob_spec]` :623-625, **alles andere** via `fix_long` :633) + `fix_cicons` :549
- NewTOS: resource.rs:460-490 / 333-369 (relociert per **Einschluss**: nur G_STRING/G_BUTTON/G_TITLE, G_TEXT/G_BOXTEXT/G_FTEXT/G_FBOXTEXT, G_IMAGE, G_ICON; `_ => {}` bei :489 lässt G_USERDEF(24) **und** G_CICON(33) unrelociert), keine CICON-Kette
- Divergenz: Ein Objekt vom Typ G_USERDEF mit nicht-null `ob_spec`-Offset behält einen rohen Datei-Offset (falscher USERBLK-Zeiger). G_CICON wird weder als Index→Zeiger aufgelöst noch die Farbicon-Kette (`fixup_all_ciconblks`/`transform_all_cicons`) ausgeführt — Farbicons in einer Nativ-RSC würden fehlerhaft gezeichnet. Praktische Relevanz hängt davon ab, ob mkrsc.py aktuell CICONs/USERDEFs erzeugt (derzeit vermutlich nicht → PLAUSIBLE).
- 1:1-Fix: Relocation auf Ausschluss-Logik umstellen (nur BOX-Typen nicht relocieren; G_CICON gesondert; alles andere `ob_spec += tb`), CICON-Fixup ergänzen falls Farbicons unterstützt werden sollen.
- Runtime-verify: yes (Nur mit CICON-/USERDEF-haltiger RSC)

### AUD-P6-013: FCLSBOX-Close-Box (eine Ebene hoch) fehlt im Fileselector
- Severity: **MEDIUM**
- EmuTOS: gemfslib.c:839-848 (FCLSBOX: `fs_back` + `fs_pspec` → eine Verzeichnisebene zurück, außer an der Laufwerkswurzel)
- NewTOS: fsel.rs (kein FCLSBOX-Handling; Doc-Zeilen 15-17 bestätigen die Auslassung; Up-Nav nur via `..` oder Pfadeditieren)
- Divergenz: Der Close-Box-Gadget als Up-Navigation existiert nicht. Da NewTOS `..` anzeigt (siehe AUD-P6-007), gibt es zwar einen Ersatz-Weg, aber die EmuTOS-Interaktion (`..` unsichtbar, FCLSBOX = hoch) ist nicht 1:1.
- 1:1-Fix: FCLSBOX-Index (gem.rsc FCLSBOX) im Modal-Loop hit-testen und `fs_back`-Logik anwenden.
- Runtime-verify: yes (echte HW: Klick FCLSBOX)

### AUD-P6-014: Fileselector-Kapazität hart auf 128 Einträge (EmuTOS: RAM-dynamisch)
- Severity: **MEDIUM**
- EmuTOS: gemfslib.c:655-657 (`nm_files = (dos_avail_anyram()-LEN_FSWORK)/(LEN_FSNAME+sizeof(char*))` — i. d. R. tausende)
- NewTOS: fsel.rs:51 (`MAX_LIST = 128`), relist/list_dir cappen bei 128
- Divergenz: Verzeichnisse mit >128 sichtbaren Einträgen werden bei 128 abgeschnitten; Einträge dahinter fehlen. EmuTOS schneidet ebenfalls ab, aber bei einem weit höheren, speicherabhängigen Limit. Bei großen Verzeichnissen beobachtbar.
- 1:1-Fix: Kapazität dynamisch aus verfügbarem RAM bemessen (oder deutlich anheben) statt fester 128.
- Runtime-verify: yes (Verzeichnis mit >128 Einträgen)

### AUD-P6-015: scrp_clear — STUB liefert 0 statt zu löschen (PCGEM=1)
- Severity: **MEDIUM**
- EmuTOS: gemsclib.c:74-107 (`sc_clear`: löscht `SCRAP.*` im `D.g_scrap`-Verzeichnis via Fsfirst/Fdelete-Schleife, `return TRUE`; `FALSE` nur wenn `g_scrap` leer) — unter `CONF_WITH_PCGEM` (Default 1)
- NewTOS: shell.rs:134-136 (`scrp_clear` → `set_ret(0)`)
- Divergenz: `scrp_clear` (82) löscht die Zwischenablage-Dateien nicht und meldet Fehlschlag. Eine App, die vor dem Schreiben neuer Clipboard-Daten `scrp_clear` aufruft, bekommt 0 und die alten `SCRAP.*` bleiben liegen. Verstoß gegen No-Stub-Regel (hartcodierter Return).
- 1:1-Fix: `SCRAP.*` im Scrap-Verzeichnis enumerieren und löschen (`Fsfirst(FA_SUBDIR)`+`Fdelete`-Schleife), TRUE zurückgeben; FALSE nur bei leerem Scrap-Pfad.
- Runtime-verify: yes

### AUD-P6-016: rsrc_gaddr — Index-Bounds-Check + addr_out=0 weichen von EmuTOS ab
- Severity: **LOW**
- EmuTOS: gemrslib.c:715-721 (`rs_gaddr` bounds-checkt den Index **nicht**; R_TREE liefert `ap_ptree[rsindex]` roh), gemsuper.c:446-447 (Dispatcher kopiert `ad_rso` **immer** nach `addr_out[0]`, also bei „not found" den `-1`-Wert aus `get_addr` default)
- NewTOS: resource.rs:882-931 (bounds-checkt R_TREE/R_OBJECT/frstr/frimg gegen die Zähler; bei not-found `addr_out[0]=0`)
- Divergenz: Bei gültigem Index identisch. Bei out-of-range Index liefert EmuTOS einen OOB-Zeiger + TRUE (bzw. `-1` in addr_out bei ungültigem **Typ**), NewTOS `0` + FALSE. Für konforme Programme unbeobachtbar (Index-OOB = App-Bug); der Bounds-Check ist sicherer. D-054 entschied jedoch explizit „no index check" → dokumentierte, bewusst sicherere Abweichung.
- 1:1-Fix: Streng 1:1 wäre kein Index-Check und `addr_out=−1` bei ungültigem Typ; die sichere Variante ist als bewusste Abweichung akzeptabel (unbeobachtbar für konforme Apps).
- Runtime-verify: no

### AUD-P6-017: fsel-Button-Return kann nie −1 werden
- Severity: **LOW**
- EmuTOS: gemfslib.c:922 (`*pbutton = inf_what(tree, FSOK)`), optimize.c:157-172 (`inf_what` → 1=OK, 0=Cancel, **−1**=keiner selektiert)
- NewTOS: fsel.rs:307,315 (`modal` liefert nur 1 oder 0 → `int_out[1]` ∈ {0,1})
- Divergenz: Der von D-054 dokumentierte Wert `−1` (kein Button selektiert) ist in NewTOS nicht erzeugbar. In der Praxis verlässt die Modalschleife nur über OK/Cancel/Doppelklick (→ 1/0), der −1-Pfad ist real kaum erreichbar.
- 1:1-Fix: `inf_what`-äquivalente Auswertung (−1 wenn weder FSOK noch FSCANCEL selektiert) statt eines binären Rückgabewerts.
- Runtime-verify: no

### AUD-P6-018: shel_write — Modus-Details (NOEXEC/SHUTDOWN) weichen ab
- Severity: **LOW**
- EmuTOS: gemshlib.c:162-192 (SHW_NOEXEC: `strcpy(D.s_cmd, DEF_DESKTOP)` — Befehl wird auf "EMUDESK" gesetzt, **nicht** geleert; SHW_EXEC: zusätzlich CONSOLE-Erkennung + `sh_curdrvdir(sh_apdir)`; SHW_RESCHNG leert cmd/tail; SHW_SHUTDOWN unter `CONF_WITH_SHUTDOWN=0` **wegkompiliert** → tut nichts)
- NewTOS: shell.rs:196-210 (SHW_NOEXEC leert `next_cmd`; SHW_SHUTDOWN|SHW_RESCHNG leeren beide und setzen `next_doex=-1`; keine Console-Erkennung/curdir-Save)
- Divergenz: (a) SHW_NOEXEC setzt in EmuTOS den Desktop-Namen statt zu leeren; (b) NewTOS „räumt" bei SHW_SHUTDOWN auf, wo EmuTOS im Referenzbuild nichts tut. Beide Rückgaben sind 1. Rez-Wechsel/Shutdown sind HW-Belange (D-101, akzeptable HW-Absence). Der NOEXEC-Befehlsinhalt ist interner Zustand ohne beobachtbaren Read-Pfad in NewTOS (`shel_read` liest separaten Puffer).
- 1:1-Fix: SHW_NOEXEC den Desktop-Befehl setzen; SHW_SHUTDOWN im HW-freien Fall als Def-No-Op behandeln (nicht cmd leeren).
- Runtime-verify: no

### AUD-P6-019: Command-Tail NUL-terminiert statt fester 128-Byte-memcpy
- Severity: **LOW**
- EmuTOS: gemshlib.c:105,170 (`memcpy(ptail, ad_stail, CMDTAILSIZE=128)` — feste 128 Byte, gemlib.h:97)
- NewTOS: shell.rs:157-164,190-192 (Tail als NUL-terminierter C-String round-tripped)
- Divergenz: Byte-exakt für konforme Text-Tails; ein Tail mit eingebettetem NUL (seltener Binär-Tail) wird abgeschnitten. Bewusst gewählt, um einen unbedingten 128-Byte-Overread eines EL0-Zeigers zu vermeiden.
- 1:1-Fix: Feste 128-Byte-Kopie (mit validierter Zeiger-Länge), wenn Binär-Tails unterstützt werden sollen.
- Runtime-verify: no

---

## Was ich getrieben habe vs. offen

**Getrieben (statisch, Quelle-gegen-Quelle):**
- R_*-Typtabelle 0-16 exakt gegen `get_addr`/`rsrc_gaddr` abgeglichen → 11 fehlende Typen (AUD-P6-002).
- `rsrc_saddr`-Präsenz → STUB (AUD-P6-001).
- `rs_obfix`/`fix_chpos` Zeile-für-Zeile: signed-High-Byte-Offset (`>128 → −256`), char×cell, width-80→gl_width, x/w=8, y/h=16 → **1:1 korrekt** (kein Finding).
- Fileselector-Datenpfad: `list_dir`/`scan`/`pattern_match`/`attr_ok` gelesen → Sort fehlt (003), Ordner-Maskenfilter (004), `.`/`..`-Filter (007).
- `scrp_read`/`write` gegen `sc_read`/`sc_write`: always-1/no-validate → **1:1 korrekt**; `sc_clear` → STUB (015).
- `shel_write` vier Modi + isgem/isover + Pexec-Schleife: Dispatcher-Slots gemsuper.c:383-396 + aesdefs.h verifiziert → Launch unkonsumiert (005), Modus-Details (018).
- `shel_envrn` Slot-Semantik (addr_in[0]=char**, addr_in[1]=Suchstring) → **Slot-Swap korrekt**; nur Env-Quelle divergiert (010).
- Buffer-Größen: SHELLBUF=4192=SIZE_SHELBUF ✓, PATHBUF=128≥LEN_ZPATH(114) ✓, CMDTAILSIZE=128 (019).
- CONF_WITH_PCGEM/COLOUR_ICONS/SHUTDOWN-Gating in config.h geprüft.
- Producer-Suche `launch_cmd`/`launch_tail` + Konsument-Suche `pending_launch` per grep → beide fehlen (005/006).

**Nicht verifiziert (bewusst statisch, keine Runtime):**
- Kein Build/QEMU/Screendump (5 parallele Auditoren — Contention; per Auftrag rein statisch). Alle „Runtime-verify: yes"-Items sind reasoned, nicht reproduziert.
- Pixel-genaue 3D-Geometrie des Selektors (fs_start-Positionsanpassungen fsel.rs:112 nur Flags, nicht die Offset-Arithmetik) — als partial markiert, nicht gemessen.
- Ob mkrsc.py aktuell G_CICON/G_USERDEF erzeugt (AUD-P6-012 daher PLAUSIBLE).

## Out-of-scope-Beobachtungen (nicht verfolgt)
- `find.rs:63` `fgetdta` truncatet High-Half-DTA auf i32 (Kommentar bestätigt) — GEMDOS-DTA-Scope, nicht P6.
- `Aespb::from_raw` (mod.rs:68) liest den Parameterblock ABI-breit — ADR-020-Pfad, außerhalb rsrc/fsel/scrp/shel.

## Cleanup-Bestätigung
Kein Produkt-Code geschrieben oder editiert, keine temporäre Test-Verdrahtung
angelegt, kein Build/QEMU ausgeführt — reine Lesezugriffe. Einziges geschriebenes
Artefakt ist diese Katalogdatei. Arbeitsbaum sonst unverändert (nur
vorbestehendes `?? pictures/`).

---

## Zusammenfassung

**Counts:** BLOCKER 2 · HIGH 4 · MEDIUM 9 · LOW 4 (19 Findings; ~30 Funktionen
auditiert).

**Bug-Dichte (echte Bugs / auditierte Fns):** 15 echte Bugs (2 BLOCKER + 4 HIGH +
9 MEDIUM) / 30 ≈ **0,50**; 4 LOW sind teils bewusste, unbeobachtbare Abweichungen.

**Top-3 Blocker/HIGH:**
1. **AUD-P6-001** `rsrc_saddr` ist ein vollständiger STUB (ret 0, ignoriert
   Argumente) — No-Stub-Verstoß, Apps können keine RSC-Zeiger setzen.
2. **AUD-P6-002** `rsrc_gaddr` liefert für 11 der 17 dokumentierten R_*-Typen
   (R_TEDINFO/R_ICONBLK/R_BITBLK/R_OBSPEC/R_TEP*/R_IBP*/R_BIPDATA) still 0 →
   Null-Deref/falsches Ergebnis im aufrufenden Programm.
3. **AUD-P6-003/004** Fileselector: keine Sortierung (Ordner-zuerst/alphabetisch
   fehlt) und Ordner werden bei jeder Maske ≠ `*.*` ausgeblendet → sichtbar
   falsche, teils unnavigierbare Dateiliste.
