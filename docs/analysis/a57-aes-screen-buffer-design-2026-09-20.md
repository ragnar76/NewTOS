# A57 — Entwurf: der persistente AES-Kratzpuffer (Menü- und Alarmhintergrund, `wind_get(WF_SCREEN)`)

Datum: 2026-09-20 · Rolle: Researcher · Auftrag: [`TASK/A57.md`](../../TASK/A57.md)
(„Erst Entwurf, dann Code") · EmuTOS gepinnt auf
`1a7484aba1bfba23e2c0899e243b6d8177fe41df` (geprüft mit `git -C references/emutos log -1`).

Dieser Text ist ein **Entwurf**, kein Code und keine Spec-Änderung. Er legt
die zwei Entscheidungen des Tickets (Pool, EL0-Abbildung) mit Beleg vor,
rechnet die Größe aus dem Geräteprofil und listet den Bau mit Abnahmen.
Jede Aussage über EmuTOS trägt `Datei:Zeile`; was gefolgert und nicht
gelesen ist, steht als **[gefolgert]**.

Vorab eine Korrektur der Ticket-Prämisse und ein Fund, beide belegt in §4
und §1.5: (1) EmuTOS' Formel, an **unserem** Profil ausgewertet, ergibt
163 840 Byte, und alle vermessenen Tafeln (TeraDesk, QED de/en, zView) passen
hinein — die Tafelgröße hängt an der Fontzelle, nicht an der Auflösung, und
die ist bei 640×480 und 1920×1080 dieselbe (8×16). Was nicht passt, ist der
theoretisch größte Alarm. (2) **TeraDesk liest heute die Länge des Puffers
und löscht bei zu kleinem Wert 13 Menüeinträge** — mit unserem `FALSE` ist
die Länge 0, die Bedingung greift, die Einträge sind weg. A57 hat damit eine
sichtbare Wirkung in einer echten Anwendung.

---

## 1. EmuTOS-Modell

### 1.1 Ein Puffer, zwei Globale

`aes/gemgsxif.c:80` `static FDB gl_tmp;` und `:83` `static LONG gl_mlen;`.
Der Puffer ist `gl_tmp.fd_addr`; seine Länge ist `gl_mlen`. Es gibt genau
diesen einen.

### 1.2 Größe (`gsx_mcalc`, `:98-104`)

```c
gsx_fix_screen(&gl_tmp);            /* :100 — fd_w/fd_h/fd_wdwidth/fd_nplanes vom Schirm, fd_addr = NULL (:518-526) */
gl_mlen = (LONG)MENU_BUFFER_SIZE * gl_wchar * gl_hchar * gl_nplanes / 8;   /* :101 */
```

`MENU_BUFFER_SIZE` (`:69-73`) = `max(MAX_MENU_CHARS, MAX_ALERT_CHARS)` mit
`MAX_MENU_WIDTH 32 × MAX_MENU_HEIGHT 20 = 640` (`:62-64`) und
`MAX_ALERT_WIDTH 50 × MAX_ALERT_HEIGHT 10 = 500` (`:50-52`) — also **640
Zeichenzellen**, „somewhat arbitrary", nachträglich für die französische
AtariWorks vergrößert (`:55-61`); die Alarmzahlen sind „derived by trial"
(`:44-49`). `gl_nplanes` kommt aus `vq_extnd(1)` `intout[4]`
(`gemgraf.c:395` → `gemgsxif.c:497-501`).

Am Referenz-TT (4 Planes, 8×16): `640·8·16·4/8 = 40 960` — der gemessene
Wert aus `measurements/measurement-2026-09-12-aes-r4/` (Adresse `0x01000218`, TT-RAM).

### 1.3 Belegung und Freigabe

`gsx_malloc` (`:108-114`): `gl_tmp.fd_addr = dos_alloc_anyram(mlen)`;
`dos_alloc_anyram` ist `Mxalloc(nbytes, MX_PREFTTRAM)` (`util/gemdos.c:117-120`,
`MX_PREFTTRAM = 3`, `include/bdosdefs.h:21`): TT-RAM bevorzugt, sonst ST-RAM
(`bdos/umem.c:308-313`). Eigentümer des Blocks ist der laufende BDOS-Prozess
(`bdos/iumem.c:134` `q->m_own = run;`) — zum Zeitpunkt der Belegung die AES
selbst **[gefolgert: `run` ist bei `geminit.c:700` der AES-Prozess; nicht
im Detail nachgelesen, für den Entwurf unerheblich]**.

Gerufen wird `gsx_malloc` aus `sh_tographic` (`aes/gemshlib.c:223-239`, Aufruf
`:236`), das zweimal erreicht wird: einmalig beim AES-Start aus
`run_accs_and_desktop` (`aes/geminit.c:700`, vor `sh_main` `:739`) und bei
jedem Wechsel Alpha→Grafik in `sh_chgrf` (`gemshlib.c:513-523`, Aufruf
`:519`). Freigegeben wird in `sh_toalpha` (`:246-260`, `gsx_mfree` `:258`),
erreicht aus `sh_chgrf` (`:521`) und am Ende von `sh_main` (`:729-730`).
Zwischen zwei GEM-Programmen bleibt der Puffer also **bestehen**; nur ein
Zeichenprogramm dazwischen gibt ihn frei und lässt ihn neu belegen (die
Adresse kann sich dann ändern). Schlägt `dos_alloc_anyram` fehl, bleibt
`fd_addr == NULL` bei unverändertem `gl_mlen` — `WF_SCREEN` antwortet dann
TRUE mit Adresse 0 (kein Fehlerpfad im Code).

### 1.4 Wer ihn benutzt

| Nutzer | Stelle | Rechteck |
|---|---|---|
| Alarm (`fm_alert`, damit auch `form_error`) | `aes/gemfmalt.c:363` `bb_save(&d)`, `:390` `bb_restore(&d)` | `d` aus `ob_center` (`:355`; `gemobed.c:67-106`: Wurzel ± 3 px bei `OUTLINED` `:83-93`, DIALERT-Wurzel ist `OUTLINED`, `gem_rsc.c:489-493`), dann `rc_intersect(&gl_rscreen, &d)` (`:358`) — **auf den Schirm geklemmt** |
| Menütafel (`menu_sr`) | `aes/gemmnlib.c:208-223`; Sichern in `menu_down` `:239`, Zurückschreiben in `mn_do` `:464,492` (bzw. `:666,680` ohne Menüerweiterung) | `ob_actxywh(tree, imenu)`; `g_x -= 1; g_w += 2; g_h += 2` (`MENU_THICKNESS = 1`, `gemmnlib.h:14`); `g_y` bleibt |
| `wind_get(WF_SCREEN)` | `aes/gemwmlib.c:1563-1565` → `gsx_mret` (`gemgsxif.c:125-129`) | — |

**Nicht** über diesen Puffer laufen Popups und Untermenüs: `popup_blit`
(`aes/gemmnext.c:136-215`, `CONF_WITH_MENU_EXTENSION`) belegt je Aufruf einen
**frischen** Block `dos_alloc_stram(bufsize)` (`:155-163`) und gibt ihn beim
Zurückschreiben frei (`:212`); so für `menu_popup` (`:328,333`) und das
Untermenü (`:668,696`). Das Untermenü öffnet, **während** die Tafel offen ist —
deshalb ist die Trennung zwingend, nicht Stil. `form_dial` sichert gar nicht
(`docs/api/aes/object-form.md:638`).

### 1.5 Was `WF_SCREEN` liefert

`gsx_mret(LONG *pmaddr, LONG *pmlen)`: `*pmaddr = (LONG)gl_tmp.fd_addr;
*pmlen = gl_mlen;` (`gemgsxif.c:125-129`), gerufen mit
`(LONG *)poutwds, (LONG *)(poutwds+2)` (`gemwmlib.c:1564`), `poutwds =
&int_out[1]` (`gemsuper.c:343`). Also: **Adresse als LONG in
`int_out[1..2]`, Länge als LONG in `int_out[3..4]`**, Rückgabe TRUE
(`:1595-1598`); kein gültiger Handle nötig (`:1517-1520`).

Wer das liest: TeraDesk `sdk/apps/teradesk/resource.c:307`
(`wind_get(0, WF_SCREEN, &dummy, &dummy, &buffer.words.high,
&buffer.words.low)`, aktiv mit `_MENUDEL 1`, `library/xdialog/library.h:174-178`)
vergleicht `mnsize >= buffer.size` (`:315`) — `mnsize` ist der Speicherbedarf
seiner zwei größten Tafeln nach `xd_initmfdb` (`library/xdialog/xdutil.c:95-108`:
`((g_w+16)&~15) · g_h · planes / 8`) — und löscht bei zu kleinem Puffer **13
Einträge** (`n = 13`, `:308`; `mn_del` `:188-208`: Separatoren 1–9, `MDELETE`,
`MFCOPY`, `MSHSIZ`, `MREVS`). gemlib kopiert die vier Worte unverändert
(`sdk/gemlib/gemlib-0.44.0/a_wind_get.c:363-372`); unser Dispatcher nullt
`int_out` → `buffer.size == 0` → **heute werden die 13 Einträge gelöscht**
**[gefolgert aus Code, nicht am Bildschirm nachgemessen — Abnahme §5.7]**.
Bei 16 Planes braucht TeraDesk mindestens `208·288·2 = 119 808` Byte
(View-Tafel, siehe §4.2), sonst greift die Löschung.

### 1.6 Überlauf („droppings")

`bb_set` (`gemgsxif.c:320-380`): `sx` wird auf 16 abgerundet, `sw` auf ein
Vielfaches von 16 aufgerundet, das den alten Rand noch deckt (`:328-332`);
`size = memsize(sw/16, sh, fd_nplanes)` = `wdwidth·h·nplanes·2` Byte (`:42`).
Ist `size > gl_mlen` (`:334`), wird **die Höhe beschnitten**
`sh = gl_mlen·sh/size` (`:336`) — „this will leave droppings!" — mit `KINFO`
nur beim Sichern (`:338-342`). Der untere Teil der Tafel wird nie gesichert
und nie zurückgeschrieben. Die Kopie selbst ist `vro_cpyfm(S_ONLY, …)`
zwischen Schirm-FDB und `gl_tmp` (`:345-377`) unter `gsx_moff()`/`gsx_mon()`
(`:367,378`; A66).

---

## 2. Entscheidung 1 — aus welchem Pool

### 2.1 Was der Puffer können muss

1. Vom **Kernel** beschreib- und lesbar (der Blit `bb_save`/`bb_restore`
   läuft im Kernel, `tos-api/src/vdi/raster.rs:569-598`, über einen rohen
   Zeiger in `Mfdb::device_mem`, `:133-135`).
2. Von **EL0** beschreib- und lesbar (Zweck des Feldes, `TASK/A54.md`).
3. **Über Prozessgrenzen hinweg persistent**: ein Block, den kein `Pterm`
   abräumt.
4. **Seitenbesitzend**: eine EL0-Abbildung ist seitengranular; die
   veröffentlichten Seiten dürfen nichts anderes enthalten (ADR-023,
   `docs/MEMORY.md:1629-1633`).
5. Größe **zur Laufzeit** aus dem Geräteprofil (Auflösung ist
   `config.txt`-gesteuert, `docs/sdk/05-abi-pitfalls.md` B3).

### 2.2 Kandidaten

**(a) Ein Block aus dem ST-/TT-RAM-Pool, Eigentümer = Kernel (EmuTOS' Weg).**
`tos-api/src/bdos/mem/traps.rs:262` `mxalloc(amount, mode)`, Modus
`MX_PREFTTRAM` (`:331`) = genau `dos_alloc_anyram`. Belegt man vor dem
ersten Prozess (bei `go_graphic()`, `kernel/src/lib.rs:394`, ist `RUN`
null — `Pterm` setzt ihn zurück, `tos-api/src/bdos/process/pterm.rs:204`),
stempelt `current_owner` (`traps.rs:88-103`) den `KERNEL_PD_SENTINEL`
(`:66`), den `free_all_owned` bei jedem `Pterm` überspringt (`:108-118`,
`free_all_owned.rs:37`). Das Muster existiert schon: `rsrc_load` nimmt
`mxalloc(pages + PAGE_SIZE, MX_PREFTTRAM)`, richtet die Basis auf eine Seite
aus und erreicht den Block über den Kernel-Alias `base_pa + VA_OFFSET`
(`tos-api/src/aes/resource.rs:2243-2252`). Die Überbelegung um eine Seite
macht den seitengerundeten Bereich **exklusiv** (Bedingung 4).
Vorteile: 1:1 EmuTOS (derselbe `Mxalloc`, derselbe Pool, der Block steht wie
bei EmuTOS auf `mp_mal` und ist über `Getmpb` sichtbar), Größe zur Laufzeit,
kein Kernel-BSS, kein Linker-Budget. Nachteile: ein Pool-Block weniger frei
(0,6–4 MiB, §4), Belegung kann fehlschlagen (`ENSMEM`) → Rückfall §5.4.

**(b) Statischer Kernelbereich, per MMU EL0-RW abgebildet.** Technisch
möglich — `Physbase` veröffentlicht heute einen kernelseitig gemappten
Bereich RW (`tos-api/src/xbios/screen.rs:93-117`, `expose_attr(…,
ProtMode::G, …)`). Aber: die Größe wäre Compile-Zeit (Bedingung 5 verletzt),
und das Linker-Budget trägt sie nicht: `__kernel_image_end` liegt im
Debug-Build bei `0x312000`, `STRAM_OS_RESERVE_PHYS = 0x400000`
(`kernel/src/arch/aarch64/link-rpi4.ld:43,108-109`) — **952 KiB** Reserve.
4 MiB (1080p, §4) sind unmöglich; 176 KiB (Minimalvariante) fräßen ein
Fünftel der Reserve. Der Kernel-Heap (2 MiB aus TT-RAM, `kernel/src/lib.rs:660-663`)
scheidet aus wie im Ticket: `AP_RW_EL1` (`kernel/src/arch/aarch64/mmu.rs:82,178-179,283-284`)
und nicht seitenbesitzend. **Abgelehnt.**

**(c) TT-RAM ausdrücklich (`MX_TTRAM`).** Ist die strikte Form von (a);
EmuTOS nimmt `MX_PREFTTRAM` mit Rückfall auf ST-RAM. Beide Pools existieren
auf dem Pi (`bring_up_bdos_pools`, `lib.rs:665-693`). Strikt hieße: ohne
TT-RAM kein Puffer — ohne Gewinn. **Aufgehoben in (a).**

### 2.3 Empfehlung

**(a)**: einmalig bei `sh_tographic` ein Pool-Block `mxalloc(page_round(len)
+ PAGE_SIZE, MX_PREFTTRAM)`, Basis seitenausgerichtet, kernel-eigen, nie
freigegeben. Abweichung von EmuTOS: **nicht** in `sh_toalpha` freigeben
(EmuTOS `:258`) und nicht in `sh_chgrf` neu belegen — EmuTOS gibt keine
Adressstabilität über einen Alpha-Wechsel, ein Zeichenprogramm sähe bei
EmuTOS sogar einen hängenden Zeiger (§1.3); „behalten" ist die sichere
Obermenge und **intern** im Sinne von `docs/COMPATIBILITY.md` (kein
Programm kann den Unterschied beobachten, außer durch Benutzung eines
freigegebenen Blocks). Im Entwurf so festhalten, damit es keine stille
Abweichung ist.

---

## 3. Entscheidung 2 — EL0-Abbildung

### 3.1 Was `usermem` heute kann (ADR-021/023)

* Je Prozess-Slot ein Fenster `[0x40_0000, 0x200_0000)` (`WINDOW_BASE =
  TPA_SIZE`, `WINDOW_SIZE`, `tos-api/src/bdos/process/usermem.rs:77-81`;
  `USER_L3_COUNT = 16`, `mmu.rs:80-87`), 32 Blöcke (`:93`), **Bump-Cursor**
  (frische VA je Block → kein TLBI, QUIRK-001, `:26-31`).
* `expose_attr(slot, kernel_va, len, prot, attr)` (`:449-540`): mappt die
  physischen Seiten hinter einer High-Half-Kernel-VA in das Fenster des
  Slots; `ProtMode::G` → `PageFlags::UserRwNx` (**RW**, `:461-467`;
  `mmu.rs:176,207`), `ProtMode::Pr` → RO; `Kind::Shared` (`:98-119`), kein
  `m_start`, **idempotent je physischer Seite** (`:477-486`: gleiche
  `map_pa`, `size >= span`, gleiche `prot`/`attr` → dieselbe User-VA).
  `translate`/`free` weisen `Shared` ab (`:545-557`, `:609-629`) — ein
  `Mfree`/`Mshrink` der App auf den Puffer ist `EIMBA`.
* Die Kernel-VA muss `phys + VA_OFFSET` sein (`kva_to_pa`, `:246-254`) — für
  einen Pool-Block ist das `base_pa + VA_OFFSET`, genau der Alias, den
  `rsrc_load` schon benutzt.
* `seed_tpa` setzt die Tabelle des Slots bei jedem Laden zurück
  (`:265-282`), `init_slot_tables` die Seitentabellen (`loader.rs:546`):
  Abbildungen leben **je Prozess** und müssen je Prozess neu gemacht werden.

Also: **RW ja**, **in jedes Prozessfenster ja** — aber nicht an einer für alle
Prozesse festen VA. Der Cursor vergibt je Prozess die nächste freie
Fenster-VA. Das ist für ein konformes Programm unsichtbar (es vergleicht nur
mit dem Zeiger, den es selbst bekam; ADR-023 „Neutral", `MEMORY.md:1651-1655`).

Das `$5A0`-/Basepage-Muster (Seite 0, `loader.rs:693-800`) ist **keine**
Abbildung, sondern eine **Kopie** je Prozess in dessen eigener TPA-Seite
(Cookie-Spiegel, `Keytbl`-Bereich); für einen Puffer, aus dem der Kernel
blittet, untauglich (es gäbe je Prozess eine andere Kopie). Eine feste VA
wäre nur mit einem reservierten Bereich im Fensterlayout erreichbar (z. B.
die obersten 4 MiB unter `USER_TPA_MAX_VA`), was den Bump-Cursor um eine
Ausnahme erweitert — ohne Gewinn für ein Programm. **Nicht empfohlen.**

### 3.2 Ereignis: wann wird gemappt

Zwei Möglichkeiten:

* **Bei `seed_tpa`/Pexec** — kostet jedes Programm 0,6–4 MiB Fenster und
  einen Blockeintrag, auch wenn es `WF_SCREEN` nie ruft; und der Puffer
  existiert zu Ladezeit eines AUTO-Programms noch nicht (`go_graphic` läuft
  nach `run_auto_folder`, `kernel/src/lib.rs:387-394`).
* **Lazy beim ersten `wind_get(WF_SCREEN)` des Prozesses** — das Muster von
  `Getbpb` (`tos-api/src/bios/bpb.rs:284-304`) und `Physbase`
  (`screen.rs:93-117`): `process::slot_for(process::current())`; `None` →
  EL1-Aufrufer; `Some(slot)` → `expose(slot, kva, len, ProtMode::G)`;
  Idempotenz liefert beim zweiten Aufruf dieselbe VA.

**Empfehlung: lazy.** Nur wer fragt, zahlt Fenster; die Abbildung ist
`Kind::Shared`, überlebt bis zum `Pterm` des Prozesses (dann fällt die
Tabelle, der Pool-Block bleibt).

### 3.3 Attribute

`PROT_G` (RW, „global — any access OK", `references/freemint/sys/mint/mem.h:130-134`,
wie `Physbase`), `MemAttr::Cacheable`: der Puffer ist gewöhnliches DRAM, das
nur die CPU berührt — Kernel-Alias und EL0-Alias haben dieselben Attribute
(Normal WB, Inner Shareable), kein Mismatch, keine Cache-Pflege (anders als
das Framebuffer, `MEMORY.md:1704-1737`). **[gefolgert: gleiche Attribute auf
beiden Aliasen → kohärent; kein Datasheet-Zitat nötig, das ist die Regelseite
des ARM ARM „mismatched attributes" — hier eben *nicht* mismatched.]**

### 3.4 Was `WF_SCREEN` dann liefert

* **EL0-Aufrufer:** `int_out[1..2]` = User-VA des Puffers (< 32 MiB, passt in
  32 Bit für ILP32 und LP64), `int_out[3..4]` = Länge (§4), `int_out[0] = 1`.
  Handle unbeachtet (`gemwmlib.c:1517-1520`, heute schon so, `window.rs:3119`).
* **EL1-Aufrufer** (das In-Kernel-Desktop; ruft `WF_SCREEN` nirgends — kein
  Treffer in `kernel/src/desk/` und `references/emutos/desk/`): die 64-Bit
  Kernel-VA passt nicht in zwei Worte. ADR-021 hält für Kernel-Aufrufer
  „raw PAs" fest (`MEMORY.md:1368`) → **die physische Adresse des
  Blocks** (< 2 GiB), dazu die Länge, TRUE. Konsistent mit dem Pool-Vertrag.
* **Ohne Puffer** (Belegung fehlgeschlagen, oder Hosttest ohne Fixture):
  **FALSE, nichts geschrieben** — die A54-Antwort bleibt der Rückfall. Das ist
  eine Abweichung von EmuTOS (dort TRUE mit Adresse 0, §1.3), bereits als
  sanktioniert eingetragen (`docs/COMPATIBILITY.md:158-199`); der Eintrag
  wird auf den Rückfall verengt, nicht gestrichen.
* **Wortlage der LONGs:** EmuTOS führt `*(LONG *)poutwds = addr` aus. Der
  Entwurf empfiehlt, **dieselbe Anweisung nativ** auszuführen — ein
  unausgerichteter 32-Bit-Store an `&int_out[1]` bzw. `&int_out[3]` (auf
  AArch64 LE: `int_out[1]` = Bits 0–15, `int_out[2]` = Bits 16–31). Beleg,
  dass genau das die vorhandenen Leser richtig bedient: TeraDesks Union
  `{ _WORD high; _WORD low; }` (`resource.c:233-241`) wird als natives
  `long size` gelesen — nativ gespeichert, nativ gelesen, das Ergebnis ist
  die Länge; ebenso cflibs `*((long *)&aes_intout[3])` (`sdk/cflib/cflib-21/purec.c:169-171`)
  bei ILP32. Das ist dieselbe In-Place-Konvention wie `Aespb::intin_ptr`
  (`tos-api/src/aes/mod.rs:312-335`), nur mit fester 32-Bit-Breite, weil
  EmuTOS 32 Bit schreibt und die Adresse < 32 MiB ist. **Offen bleibt der
  allgemeine Vertrag** (`TASK/T04.md` „Kern-Vertrag zuerst"): eine LP64-Bindung,
  die 8 Byte an `&aes_intout[1]` liest (`purec.c:166`), erhält
  `addr | len << 32` — das ist ein Overlay-Thema der Werkzeugkette
  (Memory: Fixes gehören ins Overlay), nicht des Kerns; `wind_get` kann nicht
  beides bedienen, EmuTOS' Layout hat Vorrang.

---

## 4. Größe aus dem Geräteprofil

### 4.1 Vorbemerkung: die Prämisse des Tickets, nachgemessen

Die Fontzelle ist in beiden Profilen 8×16 (`crate::vdi::font::DEFAULT =
F8x16`, `tos-api/src/vdi/font.rs:250`, `max_cell_width 8`, `top 13 + bottom
2 + 1 = 16`, `:226-232`; `gl_wchar`/`gl_hchar` daraus, `aes/graf.rs:546-557`).
RSC-Koordinaten sind Zeichenraster × Fontzelle (`gemrslib.c:53-99`
`fix_chpos`/`rs_obfix`). **Eine Tafel ist bei 1920×1080 pixelgleich groß wie
bei 640×480.** EmuTOS' Formel skaliert mit `gl_wchar·gl_hchar·gl_nplanes`,
also mit dem, woran die Tafel hängt — nicht mit der Auflösung. An unserem
Profil (16 Planes, `TASK/README.md:26`: „der Lauf meldet 16 Planes"):

```
gl_mlen = 640 · 8 · 16 · 16 / 8 = 163 840 Byte
```

Vermessen (Lesewerkzeug im Scratch, klassisches RSHDR `include/rsdefs.h:16-36`,
OBJECT 24 Byte, tafelrelative Indizes; Rechteck nach `menu_sr` + `bb_set`-
Wortausrichtung, 2 Byte je Pixel):

| Resource | größte Tafel (Zeichen × Zeilen) | Pixel | EmuTOS-Rechteck (Byte) | NewTOS `sr_rect` (Byte) |
|---|---|---|---|---|
| TeraDesk `sdk/apps/teradesk/desktop.rsc` | 25 × 18 (View) | 200×288 | **129 920** | 121 472 |
| QED de `Applications/qed/src/qed.rsc` | 23 × 21 | 184×336 | **140 608** | 130 560 |
| QED en `Applications/qed/en/qed_en.rsc` | 22 × 21 | 176×336 | 129 792 | 130 560 |
| zView `Applications/z-tools/zview/zview.rsc` | 26 × 12 | 208×192 | 86 912 | 87 808 |

Alle **< 163 840**. Die Aussage „bei 1920×1080 passen unsere Dropdowns nicht
hinein" trifft für die gemessenen Anwendungen **nicht** zu. QEDs 21 Zeilen
überschreiten zwar `MAX_MENU_HEIGHT 20`, aber der Deckel ist die
Zellenzahl (640), nicht Breite und Höhe einzeln.

### 4.2 Bedarf

**Alarm, EmuTOS-Geometrie** (`fm_build`, `gemfmalt.c:188-316`): Breite
`al.g_w` = größeres von `1 + (4+1 bei Icon) + mlenmsg + 1` (`:234`, ≤ 47 bei
`MAX_LINELEN 40`, `gem_rsc.h:83`) und `allbut + 2` mit `allbut = 3·mlenbut +
4` (`:231,239`), `mlenbut ≤ MAX_BUTLEN 20 + 1` (`:173`, `gem_rsc.h:85`) →
**69 Zeichen**; Höhe `al.g_h` = 9 Zeilen (`:249-250`, 5 Zeilen, Icon ≤ 4
Zeilen). Dazu `OUTLINED` ± 3 px (`gemobed.c:83-93`) und `bb_set`s Aufrundung
auf 16 px (bis +15 px, `gemgsxif.c:328-332`):

```
Bedarf_alert_EmuTOS(wchar, hchar) = align16(69·wchar + 6 + 15) · (9·hchar + 6) · 2
                                  = 576 · 150 · 2 = 172 800 Byte   (8×16)
```

**> 163 840.** Der theoretisch größte Alarm (drei 20-Zeichen-Knöpfe) überläuft
EmuTOS' Puffer an unserem Profil um 4 %. Das ist der eigentliche Grund, die
Größe nicht abzuschreiben.

**Alarm, NewTOS-Geometrie** (`tos-api/src/aes/form.rs:200-218`, nicht 1:1
`fm_build` — s. Nebenbefund): `box_w = 74·CW`, `box_h = 8,5·CH + 6`,
gesichert wird `+6/+6` (`:293-295`):

```
Bedarf_alert_NewTOS = align16(74·wchar + 6) · (8,5·hchar + 12) · 2 = 608 · 148 · 2 = 179 968 Byte
```

**Tafel:** anwendungsdefiniert, unbeschränkt; Messwerte oben (max. 140 608).
TeraDesks eigene Schwelle: 119 808 (§1.5).

**Untermenü / Popup:** eigener Block (§1.4), zählt hier nicht.

### 4.3 Formel und Empfehlung

Beide Nutzer klemmen ihr Rechteck auf den Schirm — der Alarm per
`rc_intersect(&gl_rscreen, &d)` (`gemfmalt.c:358`; NewTOS `BbSave::capture`
`form.rs:360-372`), die Tafel per `sr_rect` (`menu.rs:609-620`). Ein Puffer
von **einem Schirm in Gerätepixeln** ist deshalb die kleinste Größe, bei der
der Überlaufpfad **strukturell unerreichbar** ist:

```
len(W, H) = stride16(W) · H · 2 Byte          stride16(W) = ((W + 15) / 16) · 16
          = bb_save_words(W, H) · 4            (raster.rs:545-547, mem_wdwidth :196-198)

  640 × 480  :   640 · 480  · 2 =   614 400 Byte  (150 Seiten)
 1920 × 1080 :  1920 · 1080 · 2 = 4 147 200 Byte  (1013 Seiten, 4 149 248 Byte belegt)
```

Die Fontzelle fällt in dieser Formel heraus, weil sie nur den *Bedarf*
bestimmt, den der Schirm ohnehin deckt. Der Bedarf (§4.2) bleibt als
Plausibilitätsgrenze dokumentiert: `len ≥ max(Bedarf_alert_NewTOS,
Bedarf_alert_EmuTOS, größte gemessene Tafel)` gilt in beiden Profilen mit
Faktor ≥ 3,4.

Kosten: Pool 0,6 / 4,0 MiB von 768 MiB; Fenster 0,6 / 4,0 MiB **nur** im
Prozess, der fragt (TeraDesk fragt einmal beim Start). Mit `Physbase`
(15,8 MiB bei 1080p, `tos-api/src/bdos/process/mmu.rs:72-79`) zusammen bleiben ~8 MiB Fenster für
`Malloc` — das ist die eine Zahl, die der Anwender abwägen soll (§6.1).

**Minimalvariante (falls das Fenster wichtiger ist):** Deckel in Zellen wie
EmuTOS, aber aus dem Bedarf: `len = page_round(max(Bedarf_alert_NewTOS, 32·24
Zellen · wchar·hchar·2))` = `page_round(max(179 968, 272·388·2 = 211 072))` =
**212 992 Byte** (52 Seiten) in beiden Profilen. Dann braucht es die
Überlaufregel aus §5.6 wirklich.

### 4.4 Als benannte Abweichung (Vorschlag für `docs/COMPATIBILITY.md`)

> **Der AES-Menü-/Alarmpuffer ist einen Schirm groß, nicht
> `MENU_BUFFER_SIZE` Zeichenzellen.** EmuTOS rechnet `gl_mlen = 640 ·
> gl_wchar · gl_hchar · gl_nplanes / 8` (`aes/gemgsxif.c:98-104`) und
> beschneidet beim Überlauf die gesicherte Höhe — „this will leave
> droppings" (`:334-343`). An NewTOS' Profil (8×16, 16 Planes) ergibt die
> Formel 163 840 Byte; der größtmögliche `form_alert` (drei 20-Zeichen-Knöpfe,
> `gemfmalt.c:173,231-239`) braucht 172 800. NewTOS belegt
> `stride16(W)·H·2` Byte — den Schirm in Gerätepixeln — und hat damit keinen
> Überlaufpfad; die von `wind_get(WF_SCREEN)` gemeldete Länge ist deshalb
> größer als auf einem TOS/EmuTOS gleicher Auflösung (614 400 statt 163 840
> bei 640×480, 16 Bit). Ein Programm, das die Länge als Obergrenze für
> eigene Sicherungen nimmt (TeraDesk `resource.c:307-320`), bekommt mehr,
> nie weniger. Die Belegung erfolgt einmal bei `sh_tographic` und wird —
> anders als `gsx_mfree` in `sh_toalpha` (`gemshlib.c:258`) — nicht wieder
> freigegeben (intern, unbeobachtbar).

---

## 5. Bauliste mit Abnahmen

Reihenfolge ist Abhängigkeitsreihenfolge. Kein Punkt ist Code aus diesem
Dokument; jeder ist gegen die genannten Zeilen zu bauen.

### 5.1 Der Puffer (`tos-api/src/aes/`, neues Modul nach `gemgsxif.c`, z. B. `gsxif.rs`)

* `gsx_malloc()`: `len = bb_save_words(W, H) · 4`; `mxalloc(page_round(len)
  + PAGE_SIZE, MX_PREFTTRAM)` (`traps.rs:262`); `base_pa = page_round_up(raw)`;
  `kva = base_pa + VA_OFFSET`; speichern `{ kva, pa: base_pa, len }`
  (Statik, einmalig). `debug_assert!(process::current().is_null())` — der
  Block muss kernel-eigen sein (`traps.rs:88-103`). Fehlschlag → Statik
  bleibt leer, Konsolenzeile (EmuTOS meldet nichts; wir dürfen).
* Aufruf aus `kernel/src/shell_loop.rs` `sh_tographic` (`:157-163`; dort
  steht heute „`gsx_malloc()` have no counterpart to perform" — Kommentar
  ändern), einmalig (Idempotenz in `gsx_malloc`), kein Gegenstück in
  `sh_toalpha` (§2.3).
* `gsx_mret() -> Option<(kva, pa, len)>`; Hosttest-Fixture
  `install_screen_buffer(ptr, fake_pa, len)` analog `install_host`, damit
  Hosttests ohne Pool (ein Host-`Vec` liegt nicht unter 2 GiB, `init.rs`
  `seed_md` prüft `Long`) einen Puffer haben.
* **Abnahme:** Boot-Log nennt Adresse und Länge; `Getmpb`-Walk (apitest)
  zeigt den Block auf `mp_mal` mit Kernel-Sentinel **[Prüfung: `is_kernel_owned`]**.

### 5.2 `bb_save`/`bb_restore` mit Überlaufwächter (`raster.rs`)

* Kein neuer Blit. Ein dünner Wrapper im Puffermodul:
  `save(rect)`/`restore(rect)`, der `bb_save_words(w,h)·4 ≤ len` prüft; bei
  Verstoß wie `bb_set` die Höhe auf `len·h/size` beschneiden (`gemgsxif.c:336`)
  und **loggen** — unter §4.3 unerreichbar, unter der Minimalvariante die
  EmuTOS-Antwort (§5.6).
* **Abnahme:** Hosttest „Rechteck > Puffer" (nur mit Fixture kleiner Länge
  erzwingbar): gesicherte Höhe = Formel, kein Schreiben jenseits `len`.

### 5.3 `form.rs` nutzt den Puffer

* `BbSave::capture` (`form.rs:356-380`) nimmt statt `alloc::vec!` den
  Pufferzeiger; `restore` unverändert; die Struktur behält `x,y,w,h`.
  Ohne Puffer (Statik leer): heutiges Verhalten (frischer `Vec`) als Rückfall
  behalten — dann antwortet `WF_SCREEN` FALSE (§3.4), konsistent.
* **Abnahme:** `aes_form_alert_cursor_bracket` (A66) bleibt grün; neuer
  Hosttest: Alarm über Truecolor-Muster öffnen/schließen → Schirm
  **bytegleich** (Muster mit allen 5-Bit-Stufen, damit die RGB565-Verlustfreiheit
  von `raster.rs:552-554` mitgeprüft wird); Screendump QEMU vorher/nachher
  gleich.

### 5.4 `menu.rs`: Tafel über den Puffer, Device-Pixel (löst A71 mit)

* `MBACK` (`menu.rs:571-593`) für die **Tafel** (`open_box` `:678-691`,
  `erase_box_inner` `:738-744`) durch `bb_save`/`bb_restore` in den
  persistenten Puffer ersetzen; `save_under`/`restore_under` (`:630-673`,
  `read_pixel`/`fill_rect` je Pixel) entfallen für diesen Pfad.
* **Untermenü** (`SMBACK`, `:511,531`) und **Popup** (`popup_track`
  `:1041-1052`, heute `MBACK`) **nicht** über den persistenten Puffer
  (§1.4: EmuTOS `popup_blit` = frischer Block; das Untermenü öffnet bei
  offener Tafel). Zwei Wege: (i) je Öffnung ein transienter Pool-Block
  `mxalloc(…, MX_STRAM)` wie `gemmnext.c:158` mit `dos_free` beim
  Schließen, Fehlschlag → D-086 (`docs/api/aes/menu.md:391`); (ii) die
  vorhandene zweite Statik, aber im Device-Format über `bb_save`/`bb_restore`
  (A71). (i) ist EmuTOS' Weg; (ii) ist kleiner. Entscheidung dem Developer,
  im Kommentar benennen.
* `sr_rect` bleibt (Rechteckform ist nicht Gegenstand von A57; Nebenbefund
  unten).
* **Abnahme:** A71-Hosttest: Tafel über Truecolor-Muster öffnen/schließen →
  bytegleich; Untermenü bei offener Tafel öffnen/schließen → beide
  Rechtecke bytegleich (Verschachtelung); Screendump TeraDesk-Menü
  auf/zu in QEMU gleich dem Ausgangsbild.

### 5.5 `wind_get(WF_SCREEN)` (`window.rs:3088-3122`)

* `match gsx_mret()`: `None` → `set_ret(0)` (heutiger Zweig, Kommentar auf
  „Rückfall ohne Puffer" kürzen); `Some` → Adresse nach §3.4 (EL0: `expose(slot,
  kva, len, ProtMode::G)`, `None` vom Fenster → FALSE; EL1: `pa`), Länge,
  `set_ret(1)`. Neuer Helfer `Aespb::set_intout_long(word, u32)` =
  unausgerichteter nativer 32-Bit-Store (§3.4; Bezug `TASK/T04.md`).
* Spec `docs/api/aes/window-graphics.md` (Zeile 350 und der Handle-Absatz
  `:394`): Absatz mit `int_out[1..2]`/`[3..4]`-Layout, EL0/EL1-Antwort,
  Rückfall FALSE, Verweis auf den COMPATIBILITY-Eintrag §4.4.
  `docs/api/vdi/raster/_raster.md:601-609` („transient per-alert buffer …
  still honestly reports a null/zero-length buffer") ist seit A54 falsch und
  wird mit diesem Bau richtig; `docs/api/aes/menu.md` § `menu_sr`
  (`:595-596`) und `object-form.md` `:682-684` bekommen den Satz „ein
  Puffer, `gsx_malloc`".
* `docs/COMPATIBILITY.md:158-199`: Eintrag auf den Rückfall verengen, neuen
  Eintrag §4.4 anfügen; `docs/STATUS.md` Recent Decisions.
* **Abnahme (Host):** `tos-api/tests/aes_window_screen_buffer.rs` umdrehen:
  mit Fixture → `int_out[0] == 1`, `int_out[1..2]` = Fixture-`pa` (EL1-Pfad),
  `int_out[3..4]` = `bb_save_words(W,H)·4` des Test-Backends; ohne Fixture →
  weiterhin FALSE (der dritte Test `the_other_two_handle_free_fields_still_answer`
  bleibt). Referenzharness: Fixture in `context()` (`harness.rs:688-700`)
  installieren, **Fall 166 aus `SANCTIONED` streichen** (`harness.rs:1472-1511`
  — ein Eintrag, dessen Antwort dem Referenzwert gleicht, ist dort ohnehin
  ein Fehler); r4-Fälle 331/332/439 gegen `EMUTOS14-AES-r4.BIN` erneut
  fahren: `int_out[0]` wieder 1, `[1..4]` bleiben Geräteparameter
  („RECORDED, not compared away").

### 5.6 Überlaufverhalten — benannt

* **Empfohlene Größe (§4.3):** unerreichbar (beide Rechtecke ⊆ Schirm ⊆
  Puffer). Der Wächter aus §5.2 bleibt als Defensive mit Logzeile.
* **Minimalvariante:** EmuTOS' Höhenbeschnitt (`gemgsxif.c:336`) **nicht**
  nachbauen — er hinterlässt Müll. Stattdessen: Sichern überspringen, Statik
  „ungültig" markieren, beim Schließen kein Zurückschreiben, sondern die
  Anwendung erhält für das Rechteck eine `WM_REDRAW`-Nachricht (das ist der
  Weg, den `form_dial(FMD_FINISH)` ohnehin geht) und der Desktop zeichnet
  seinen Teil neu. Das ist eine **beobachtbare** Abweichung (Nachricht statt
  Müll) und wäre als solche einzutragen — ein Grund mehr für §4.3.

### 5.7 EL0-Schreibprobe (QEMU, ohne Hardware)

* `sdk/examples/wfscreen.c` nach dem Muster von `physbase.c`/`apitest.c`:
  `appl_init`; `wind_get(0, WF_SCREEN, &a1,&a2,&l1,&l2)`; erwartet
  `ret == 1`, Adresse `< 32 MiB` (`el0_readable`-Bereichsprüfung), Länge
  `== stride16(W)·H·2` (W/H aus `vq_extnd`/`v_opnvwk`); dann **über die
  ganze Länge** ein Muster schreiben und zurücklesen; `Cconws` PASS/FAIL;
  `appl_exit`. Start über `#Z`-Autorun (Memory: QEMU-Dumps via `#Z`), nicht
  aus `\AUTO\` — dort existiert der Puffer noch nicht (§3.2). Kein Fault ⇒
  bestanden; Screendump als Beleg.
* **TeraDesk-Nachweis:** Screendump des View-Menüs vor/nach A57 —
  vorher fehlen die 13 Einträge aus §1.5 (Vorhersage aus dem Code, erst
  gegen den Dump prüfen — Memory: Vorhersagen erst gegen die Quelle
  prüfen), nachher sind sie da. Das ist die Abnahme, die eine echte
  Anwendung liefert.

---

## 6. Offene Entscheidungen für den Anwender (nur echte)

1. **Größe: ein Schirm (§4.3, 4 MiB bei 1080p, kein Überlaufpfad) oder
   Bedarfsdeckel (§4.3 Minimalvariante, 208 KiB, mit Überlaufregel §5.6)?**
   Der Unterschied ist Fensterplatz im fragenden Prozess (4 MiB von 28) gegen
   eine zweite beobachtbare Abweichung. Empfehlung: ein Schirm.
2. **Untermenü/Popup: transienter Pool-Block je Öffnung (EmuTOS) oder
   zweite Statik im Device-Format?** (§5.4). Empfehlung: EmuTOS' Weg, weil
   D-086 dann wörtlich gilt.

Nicht offen (im Entwurf entschieden, Widerspruch erbeten, falls gewünscht):
Pool (a) mit `MX_PREFTTRAM`; lazy `expose` mit `PROT_G`/cacheable; EL1 erhält
die PA; Rückfall FALSE ohne Puffer; Puffer wird nie freigegeben.

---

## Nebenbefunde (außerhalb A57, je eine Zeile)

* `tos-api/src/aes/form.rs:200-218` baut den Alarm nicht 1:1 nach
  `fm_build` (`gemfmalt.c:188-316`: Knopfbreite `mlenbut+1`, Abstände 2,
  Höhe 9 Zeilen; NewTOS `mlenbut+2`, eigene Padding-Regel) — eigener Task.
* `menu.rs:609-620` `sr_rect` wächst nach `popup_blit`-Form
  (`+MT+ADJ3DSTD` rechts/unten), `menu_sr` wächst nur `±MT` und lässt `g_y`
  stehen (`gemmnlib.c:215-217`) — Abweichung im Rechteck, nicht im Puffer.
* `docs/api/vdi/raster/_raster.md:606-609` behauptet noch die
  A54-Vorgängerantwort („null/zero-length buffer") — veraltet.
* `sdk/cflib/cflib-21/purec.c:165-172` liest `WF_SCREEN` als 8-Byte-Zeiger
  an `&aes_intout[1]` — auf LP64 falsch; gehört zu `TASK/T04.md`.
* `docs/api/aes/_full-function-inventory.md` Zeilen 443/878 (`WF_SCREEN`
  „ja, aber 0/0") — laut A54-Quittung A53 zugewiesen, weiterhin offen.

## Lesewerkzeug

`/private/tmp/claude-501/…/scratchpad/menusize.py` (nur lesend; RSHDR nach
`include/rsdefs.h:16-36`, `fix_chpos`-Umrechnung nach `gemrslib.c:53-83`,
tafelrelative Objektindizes). Ergebnisse in §4.1; nicht ins Repo übernommen.
