# T04 — `LONG`s und Zeiger in `int_in`/`int_out`: der Kern-Vertrag, gemessen (2026-09-21)

Auditor-Bericht zu `TASK/T04.md`, Schritt 1 („Kern-Vertrag zuerst"). Kein
Produktcode geändert; ein grüner Vertragstest liegt unter
`tos-api/tests/aes_t04_intin_long_contract.rs`. Host-Sonden (pristine gemlib
gegen Fake-`vdi()`/`aes()`, ASan, Ziel-Disassembly) liegen im Scratchpad der
Sitzung; ihre Ausgaben sind hier zitiert.

Stand des Baums: `desk-rebuild`, HEAD `ee359d3`.

---

## 1. Ergebnis in einem Satz

Der Kern kennt **eine** Regel, nicht zwei, und jeder AES-Leser/-Schreiber
folgt ihr:

> Ein Wert, der breiter als ein WORD ist, liegt im `int_in`/`int_out`-Array
> **nativ in place** an `&int_x[n]` — little-endian, wortausgerichtet,
> für seine eigene Breite unausgerichtet gelesen/geschrieben. Ein **`LONG`**
> (EmuTOS-Typ `LONG`/`ULONG` an der Zugriffsstelle) ist **32 Bit über zwei
> Worte**: `int_x[n]` = Bits 0–15, `int_x[n+1]` = Bits 16–31. Ein
> **Zeiger** (EmuTOS-Typ `char **`/`OBJECT **`) hat die **Zeigerbreite des
> Aufrufers**: 4 Worte LP64, 2 Worte ILP32 (A81). Die deklarierte Wortzahl
> (`control[IN_LEN]`, `contrl[3]`) zählt diese Worte mit; im AES klammert
> `Aespb::from_raw` darauf (`tos-api/src/aes/mod.rs:159-165`, A19).

Warum A81 („Zeiger in Aufruferbreite") und H03 („Wortpaar `[lo, hi]`")
**keine zwei Konventionen** sind: EmuTOS liest `evnt_timer`/`evnt_multi`
explizit als `MAKE_ULONG(int_in[1], int_in[0])` — `int_in[0]` ist das
**Low**-Wort (`references/emutos/aes/gemsuper.c:143,150`,
`include/aesdefs.h:190-191,220-221`, `include/portab.h:139`). Auf einer
Little-Endian-Maschine ist genau das die Bytefolge eines nativen 32-Bit-Stores
an `&int_in[0]`. Das „Wortpaar" von H03 **ist** die native In-Place-Form auf
LE; A81 ist dieselbe Form für die Breite „Zeiger". Auf dem m68k fielen beide
Breiten zusammen (32 Bit), nur deshalb hatte EmuTOS dort zwei Schreibweisen
(`*(LONG *)&int_out[1]` nativ, `MAKE_ULONG(...)` explizit), die auf BE
**verschiedene** Bytefolgen ergeben; auf LE ergeben sie dieselbe.

Der H03-Defekt war folglich **nicht** „Punning statt Arithmetik", sondern
das explizite **Vertauschen** der beiden Worte in `a_evnt_timer.c:30-31`
(`aes_intin[0] = i[1]; aes_intin[1] = i[0];`), das auf dem m68k BE-nativ
`[hi, lo]` nach `[lo, hi]` drehte und auf LE `[lo, hi]` nach `[hi, lo]`.
Kein anderes gemlib-Binding vertauscht; die vier „Klasse-1"-Stellen des
Tickets übergeben das native Layout **unverändert** und sind damit auf LP64
byte-richtig (§4, gemessen).

---

## 2. Die Kern-Leser und -Schreiber (vollständig)

Gesucht mit `/usr/bin/grep` über `tos-api/src/aes/` und `tos-api/src/vdi/`
nach jeder Stelle, die mehr als ein Wort aus `int_in`/`intin` zusammensetzt
oder in `int_out`/`intout` schreibt.

| Funktion (Opcode) | Slot | Kernform (Datei:Zeile) | EmuTOS-Form (Datei:Zeile) | Regel eingehalten |
|---|---|---|---|---|
| `evnt_timer` (24) | `int_in[0..1]` | `lo = int_in[0]`, `hi = int_in[1]`, `(hi << 16) \| lo` — `tos-api/src/aes/event.rs:830-834` | `MAKE_ULONG(T_HICOUNT, T_LOCOUNT)`, `T_LOCOUNT = int_in[0]`, `T_HICOUNT = int_in[1]` — `aes/gemsuper.c:143`, `include/aesdefs.h:190-191` | ja (LONG, `[lo, hi]`) — **getrieben**, Test 1 |
| `evnt_multi` (25) | `int_in[14..15]` | `lo = int_in[14]`, `hi = int_in[15]` — `event.rs:492-494` | `MAKE_ULONG(MT_HICOUNT, MT_LOCOUNT)`, `aesdefs.h:220-221`, `gemsuper.c:150` | ja — **getrieben**, Test 2 |
| `wind_set` (105) `WF_NAME`/`WF_INFO`/`WF_NEWDESK`, gepackte Form (`AIN_LEN == 0`) | `int_in[2..]` | `Aespb::intin_ptr(2)`: `read_unaligned` als `u64` (LP64) / `u32` (ILP32) — `mod.rs:327-346`, `window.rs:3405`; Wortzahl `ptr_words()` 4/2 — `mod.rs:303-308` | `*(char **)pinwds`, `pinwds = &int_in[2]` — `aes/gemwmlib.c:1623,1661,1669,1687` (nativ, m68k 2 Worte) | ja (Zeiger, Aufruferbreite) — gepinnt durch `aes_window_name_pointer.rs`, `aes_newdesk_root_word.rs` (A81) |
| `wind_set` `WF_NEWDESK`-Wurzel | `int_in[2 + ptr_words]` | `window.rs:1297` | `pinwds[2]` = `int_in[4]` — `gemwmlib.c:1688` | ja (ILP32 bit-exakt; LP64 `int_in[6]`) — A81-Test |
| `wind_set` native Form (`AIN_LEN >= 1`) | `addr_in[0]` | `window.rs:3405` (`wf_str_ptr`) | — (NewTOS-Zusatz, A81) | ja |
| `wind_get` (104) `WF_SCREEN` | `int_out[1..2]`, `int_out[3..4]` | `Aespb::set_intout_long`: `write_unaligned::<u32>` an `&int_out[word]` — `mod.rs:367-378`, `window.rs:3885-3886` | `gsx_mret((LONG *)poutwds, (LONG *)(poutwds+2))` → `*(LONG *)&int_out[1]`, `*(LONG *)&int_out[3]` — `gemwmlib.c:1564`, `gemgsxif.c:125-129` | ja (LONG, 32 Bit, A57) — **getrieben**, Test 3 |
| VDI: alle Opcodes | `intin`/`intout` | **kein** Leser/Schreiber eines LONG oder Zeigers in `intin`/`intout` (`tos-api/src/vdi/*.rs`; `attributes.rs:202` ist eine Verhältnisrechnung, kein Slot) | EmuTOS ebenso: kein `*(LONG *)&INTIN`, kein `MAKE_ULONG` in `vdi/*.c` | — (kein Fall) |
| VDI: MFDB-Zeiger in `CONTRL[7+n·N_PTRINTS]` (109/110/121) | `contrl` | `Vdipb::contrl_ptr`: Aufruferbreite, Index skaliert — `vdi/mod.rs:158-171` | `CONTRL[7..8]`, `[9..10]` (m68k nativ) | ja (Zeiger, Aufruferbreite) — angrenzend, nicht `intin` |
| VDI: `vex_butv/motv/curv/timv/wheelv` (125/126/127/118/134) | `CONTRL[7..]` ein, `CONTRL[9..]` aus | `contrl_u64(7)` **fest 8 Byte**, `set_contrl_u64(9)` **fest 8 Byte an Index 9** — `vdi/mouse.rs:693-695`, `vdi/mod.rs:136-139` | `ULONG_AT(&CONTRL[9]) = alt; neu = ULONG_AT(&CONTRL[7])` — `vdi/vdi_mouse.c:372-373` | **nein** — s. §6 (außerhalb `intin`, Lead-Entscheidung ob T04) |

Weitere AES-Handler lesen nur Worte oder `addr_in[]`/`addr_out[]` (nativ in
Aufruferbreite, `Abi::read_ptr`/`write_ptr`): `appl_tplay`/`appl_trecord`,
`menu_*`, `objc_*`, `form_*`, `shel_*`, `rsrc_gaddr`, `graf_mouse`,
`fsel_*`. Kein weiterer zusammengesetzter Wert existiert.

**Kern-seitige Verstöße gegen die Regel in `int_in`/`int_out`: 0.**
(Angrenzend in `CONTRL`: 1, die `vex_*`-Familie, §6.)

Dokumentationsfehler kern-seitig: der Doc-Kommentar von `set_intout_long`
(`tos-api/src/aes/mod.rs:355-360`) behauptet, cflibs
`*((long *)&aes_intout[3])` „get the value back". Für LP64 stimmt das nicht
(8-Byte-Lesung, obere Hälfte aus `int_out[5..6]`; §5, Test 3); das
A57-Design sagt korrekt „bei ILP32"
(`docs/analysis/a57-aes-screen-buffer-design-2026-09-20.md:277-286`), der
Kommentar hat die Einschränkung verloren.

---

## 3. Was die Sonden gemessen haben (Host, LP64/LE = Zielbreite und -bytefolge)

Pristine `gemlib-0.44.0`-Quellen aus dem Tarball gegen die Overlay-Header
(`-D__NEWTOS__`, `VDI_TRAP*` → `vdi()`, `AES_TRAP` → `aes()`) mit einem
Fake-Trap, der den Parameterblock einfängt:

```
vq_ctab(ctab_length=0x00010002)          op=206/0   nintin=2  intin[0..3] = 0002 0001 0000 0000
vq_dflt_ctab(ctab_length=0x00010002)     op=206/7   nintin=2  intin[0..3] = 0002 0001 0000 0000
vst_setsize32(point=0x00123456)          op=252/0   nintin=2  intin[0..3] = 3456 0012 0000 0000
H03-Kontrast, pristine a_evnt_timer:                intin[0]=i[1]=0012 intin[1]=i[0]=3456  -> [hi,lo] VERTAUSCHT
v_delete_ctab(ptr=0x600011223344)        op=206/9   nintin=2  intin[0..3] = 3344 1122 6000 0000  (8-Byte-Zeiger byte-richtig; nintin 2 statt N_PTRINTS 4)
v_delete_itab(ptr=0x600011223344)        op=208/1   nintin=2  intin[0..3] = 3344 1122 6000 0000
vqt_fontheader(buffer=&buf)              op=232/0   nintin=2  8-Byte-Zeiger == &buf; nintin 2
vs_hilite_color(cs=0x00010002, entry)    op=207/0   nintin=6  intin[0..5] = 0002 0001 0001 0002 0000 0000  (LONG [lo,hi]; Spill durch das nachfolgende Entry überschrieben)
vs_hilite_color(cs=-1, entry)            op=207/0   nintin=6  intin[0..5] = ffff ffff 0001 0002 0000 0000
mt_x_malloc(size=0x12345678)             op=0x7186  nintin=2  intin[0..3] = 5678 1234 0000 0000  (LONG [lo,hi]; Spill 0 in [2..3])
mt_x_malloc(size=-2)                                          intin[0..3] = fffe ffff ffff ffff  (negatives long: Spill 0xffff in [2..3])
mt_fnts_get_style(id=0x00010002, idx=5)  op=184     nintin=4  intin[0..4] = 0001 0002 0001 0005 0000  (index nach dem long geschrieben: bleibt)
```

ASan (`-O0`, `halt_on_error=0`) auf den exakt dimensionierten lokalen Arrays:

```
WRITE of size 8  n_vst_arbpt32.c:39   vst_arbpt32   short vdi_intout[2]; vdi_intout_long(0) = 0;
READ  of size 8  n_vst_arbpt32.c:61   vst_arbpt32   return vdi_intout_long(0);
WRITE of size 8  n_v_value2color.c:27 v_value2color short vdi_intin[2];  vdi_intin_long(0) = value;
READ  of size 8  n_vq_ctab_id.c:26    vq_ctab_id    short vdi_intout[2]; return vdi_intout_long(0);
READ  of size 8  n_vst_setsize32.c:54 vst_setsize32 short vdi_intout[2]; return vdi_intout_long(0);
```
(`v_create_ctab` `short vdi_intin[4]` + `vdi_intin_long(2)` und
`vst_track_offset` dito überlaufen um 4 Byte — arithmetisch sicher
(`sizeof`), ASan meldet unausgerichtete, teilweise überlappende 8-Byte-Zugriffe
bekanntermaßen nicht.)

Ziel-Disassembly (`clang -target aarch64-unknown-none-elf -O2`, exakt die
Flags aus `sdk/gemlib/build-gemlib.sh`):

* `vst_arbpt32`: `stur w8,[x29,#-0x28]` schreibt `control[0] = 246`; danach
  `stur xzr,[x29,#-0x2c]` (der 8-Byte-Nullstore in `vdi_intout[2]`)
  überschreibt `-0x2c..-0x24` und damit `control[0..1]` → **der VDI erhält
  Opcode 0**, nicht 246.
* `v_create_ctab`: `stur x2,[x29,#-0x24]` (`vdi_intin_long(2) = px_format`)
  überschreibt `control[0..1]` (Opcode 206 → obere Hälfte von `px_format`,
  also 0); der Store von `color_space` (`vdi_intin_long(0)`) ist **ganz
  entfallen** (UB-Folge) → Opcode 0, `intin[0..1]` uninitialisiert.
* `vst_track_offset`: dito, Opcode 237 → 0; die Stores `vdi_intin[0] = 255`,
  `vdi_intin[1] = pairmode` fehlen im Objekt.
* `v_value2color`: `stur x1,[x29,#0x1c]` schreibt 8 Byte an `fp+0x1c..+0x24`
  bei Rahmengröße 0x80 → **4 Byte in den Rahmen des Aufrufers**.

Die gemlib.a des SDK enthält diese Symbole (`llvm-nm`: `vst_arbpt32`,
`v_value2color`, `vq_ctab` vorhanden); kein Port (QED, TeraDesk, cflib,
gemtest, apitest) ruft eines der 200er-VDI- oder `x_*`/`fnts_*`-AES-Bindings
auf (Quellsweep §5, „lebendig").

---

## 4. gemlib × Kern, Stelle für Stelle

Spalten: gemlib-Form (pristine Zeile) · Kern-Leser · EmuTOS · stimmt unter
der Regel. „Kern: unimpl." = der Opcode erreicht in NewTOS keinen Handler
(VDI `vdi/mod.rs:410` `_ => {}` bzw. Escape > 19
`vdi/escape.rs:288`; AES `mod.rs:679` → `RET_CODE -1`) — **wie EmuTOS**
(VDI: nur 1–39 und 100–134, `vdi/vdi_main.c:150-157`; Escapes 0–19,
`vdi_esc.c:295-314`; AES: `crysbind` `default:` → −1). Für solche Opcodes
ist die Drahtform heute unbeobachtbar; die Bewertung gilt der Regel.

### 4.1 Klasse 1 — `(short *)&long` als `intin`-Array (4 Stellen)

| Datei:Zeile | Ausdruck | Opcode | Draht auf LP64 (gemessen) | Kern | Regel |
|---|---|---|---|---|---|
| `n_vq_ctab.c:23,25` | `VDI_PARAMS(…, (short*)&ctab_length, …)`, nintin 2 | 206/0 | `[lo, hi]` = 32-Bit-LONG richtig | unimpl. | **stimmt** |
| `n_vq_dflt_ctab.c:23,25` | dito | 206/7 | `[lo, hi]` richtig | unimpl. | **stimmt** |
| `n_vst_setsize32.c:33,35` | `VDI_PARAMS(…, (short*)&point, …)`, nintin 2 | 252 | `[lo, hi]` richtig; Rückgabe `vdi_intout_long(0)` liest 8 Byte aus `vdi_intout[2]` (READ-Überlauf, obere Hälfte Stack) | unimpl. | Eingang stimmt; Ausgang = Klasse 3b |
| `n_vst_arbpt32.c:38-39,41,61` | dito + `vdi_intout_long(0) = 0` vor dem Trap | 246 | Eingang wäre `[lo, hi]`; **im Zielobjekt wird `control[0]` durch den 8-Byte-Nullstore auf 0 gesetzt** (§3) | unimpl. (Opcode 0 ebenfalls) | Eingang stimmt, **Binding kaputt durch Klasse 3b** |

Ticket-Aussage „Klasse 1 — dieselbe Form wie H03 … unabhängig davon falsch"
ist **widerlegt**: das Punning ohne Vertauschen liefert auf LE genau die
`[lo, hi]`-Form, die EmuTOS für `evnt_timer` fordert und die der Kern liest.
EmuTOS hat keine dieser vier Funktionen (kein `arbpt`, `ctab`, `setsize` in
`vdi/`); die Referenz für ihre Drahtform ist die m68k-gemlib selbst
(nativ in place), nicht `gemsuper.c:142`.

### 4.2 Klasse 2 — `(short *)&ptr` als `intin`-Array (5 Stellen)

| Datei:Zeile | Ausdruck | Opcode | Draht auf LP64 (gemessen) | Kern | Regel |
|---|---|---|---|---|---|
| `n_v_delete_ctab.c:23,25` | `(short*)&ctab`, nintin **2** | 206/9 | 8-Byte-Zeiger nativ an `&intin[0]` (byte-richtig); **nintin 2 statt `N_PTRINTS` 4** | unimpl. | Bytes stimmen; **Wortzahl falsch** (VDI-Kern klammert nicht auf `contrl[3]`, also heute folgenlos) |
| `n_v_delete_itab.c:23,25` | `(short*)&itab`, nintin 2 | 208/1 | dito | unimpl. | dito |
| `n_vqt_fontheader.c:52,54` | `(short*)&buffer`, nintin 2 | 232 | dito | unimpl. | dito |
| `u_vqt_fontheader.c:12,14` | dito (`_VDIParBlk`) | 232 | dito | unimpl. | dito |
| `a_wind_set_str.c:25-26` (pristine) | `*(const char**)(aes_intin+2) = str; *(const char**)(aes_intin+4) = 0;` | 105 | **durch A81-Overlay ersetzt**: `sdk/gemlib/overlay/a_wind_set_str.c` → `AES_PARAMS(105,6,1,1,0)`, `aes_addrin[0] = str`; ebenso `a_wind_set_ptr.c`, `a_wind_set_ptr_int.c` (Wurzel in `aes_intin[2]`) | `wf_str_ptr` native Form (`window.rs:3405`) | **stimmt** (erledigt, Hosttest `sdk/gemlib/tests/wind_set_addrin.c`) |

### 4.3 Klasse 3 — die Accessoren (33 `*_intin_long`, 36 `*_intout_long`)

Definitionen: `gem_aesP.h:89-99` (`*((long *)(aes_intin + n))`),
`overlay/gem_vdiP.h:182-208` (`*((long *)(array + n))`, `N_PTRINTS =
sizeof(void*)/sizeof(short)`). Auf LP64: **8-Byte**-Store/-Load an einem
nur wortausgerichteten Slot.

**3a — Schreiber `*_intin_long(n) = v`.** Drahtform in `int_in[n..n+1]`:
`[lo, hi]` = 32-Bit-LONG nach Regel — **richtig**, solange `v` in 32 Bit
passt. Zusätzlich „Spill" der oberen 32 Bit nach `int_in[n+2..n+3]`
(0 bei nichtnegativen, 0xFFFF·2 bei negativen Werten). Ob der Spill etwas
zerstört, hängt von der **Store-Reihenfolge** und der **Arraygröße** ab:

| Datei:Zeile | Ausdruck | Opcode | lokales Array | Reihenfolge / Überlauf | Kern | Regel |
|---|---|---|---|---|---|---|
| `x_fnts_do.c:38-40` | `aes_intin_long(1)=id_in; (3)=pt_in; (5)=ratio_in` | 187 | `aes_intin[16]` | aufsteigend, Spill überschrieben/außerhalb IN_LEN 7 | −1 | stimmt (Draht) |
| `x_fnts_get_info.c:26` | `aes_intin_long(1) = id` | 184 | `[16]` | ok | −1 | stimmt |
| `x_fnts_get_name.c:26` | `aes_intin_long(1) = id` | 184 | `[16]` | ok | −1 | stimmt |
| `x_fnts_get_no_styles.c:24` | `aes_intin_long(1) = id` | 184 | `[16]` | ok | −1 | stimmt |
| `x_fnts_get_style.c:23-24` | `aes_intin_long(1)=id; aes_intin[3]=index` | 184 | `[16]` | `index` nach dem long → bleibt (gemessen) | −1 | stimmt |
| `x_fnts_open.c:41-43` | `long(3)=id; long(5)=pt; long(7)=ratio` | 182 | `[16]` | aufsteigend | −1 | stimmt |
| `x_fnts_update.c:44-46` | `long(2)=id; long(4)=pt; long(6)=ratio` | 185 | `[16]` | aufsteigend | −1 | stimmt |
| `x_form_xerr.c:34` | `*__aes_intin_long(0, aes_intin) = errcode` | 136 | `[16]` | Spill in [2..3] > IN_LEN 2 | −1 | stimmt |
| `x_graf_rast2rez.c:23-25` | `long(0)=plane_len; [2]=old_planes; [3]=devspef` | 0x7132 | `[16]` | Worte nach dem long | −1 | stimmt |
| `x_malloc.c:19` | `aes_intin_long(0) = size` | 0x7186 | `[16]` | Spill > IN_LEN 2 (gemessen) | −1 | stimmt |
| `x_mshrink.c:21` | `aes_intin_long(0) = newsize` | 0x7188 | `[16]` | dito | −1 | stimmt |
| `x_realloc.c:21` | `aes_intin_long(0) = size` | 0x7189 | `[16]` | dito | −1 | stimmt |
| `n_v_color2nearest.c:30` | `vdi_intin_long(0) = color_space` | 204/2 | `vdi_intin[6]` | Entry nach dem long | unimpl. | stimmt |
| `n_v_color2value.c:27-28` | `long(0)=color_space; *(COLOR_ENTRY*)&vdi_intin[2]=*color` | 204/0 | `[6]` | ok | unimpl. | stimmt |
| `n_v_create_ctab.c:27-28` | `long(0)=color_space; long(2)=px_format` | 206/8 | **`[4]`** | **Überlauf 4 Byte; im Zielobjekt Opcode → 0, `color_space`-Store entfallen** (§3) | unimpl. | **Binding kaputt** |
| `n_v_resize_bm.c:30-31` | `long(2)=byte_width; vdi_intin_ptr(4,…)=addr` | 100/2 | `[4+N_PTRINTS]` | Zeiger nach dem long | → `v_opnvwk` (wie EmuTOS, `contrl[5]` ignoriert) | stimmt (Draht) |
| `n_v_value2color.c:27` | `vdi_intin_long(0) = value` | 204/1 | **`[2]`** | **Überlauf 4 Byte in den Aufruferrahmen** (§3, ASan) | unimpl. | **Binding kaputt** |
| `n_vs_ctab_entry.c:30-31` | `long(1)=color_space; ent=&vdi_intin[3]` | 205/1 | `[7]` | Entry nach dem long | unimpl. | stimmt |
| `n_vs_hilite_color.c:27` | `vdi_intin_long(0) = color_space` | 207/0 | `[6]` | gemessen | unimpl. | stimmt |
| `n_vs_max_color.c:27` / `n_vs_min_color.c:27` / `n_vs_weight_color.c:27` | dito | 207/2,1,3 | `[6]` | ok | unimpl. | stimmt |
| `n_vsf_bg_color.c:27` / `n_vsf_fg_color.c:27` / `n_vsl_bg_color.c:27` / `n_vsl_fg_color.c:27` / `n_vsm_bg_color.c:27` / `n_vsm_fg_color.c:27` / `n_vsr_bg_color.c:27` / `n_vsr_fg_color.c:27` / `n_vst_bg_color.c:27` / `n_vst_fg_color.c:27` | `vdi_intin_long(0) = color_space` | 200–201/0–4 | `[6]` | ok | unimpl. | stimmt |
| `n_vst_track_offset.c:28-30` | `[0]=255; [1]=pairmode; long(2)=offset` | 237 | **`[4]`** | **Überlauf; im Zielobjekt Opcode → 0, Wortstores entfallen** (§3) | unimpl. | **Binding kaputt** |

**3b — Leser `*_intout_long(n)`.** Der Kern schreibt (wo er schreibt) 32 Bit;
die Bindung liest 8. Obere Hälfte = `int_out[n+2..n+3]`: vorgenullt
(`pdlg_get_setsize.c:20`, `x_fnts_get_style.c:26`, `x_edit_evnt.c:47`,
`n_vst_arbpt32.c:39`) oder Stack. **Unter der Regel falsch** (gemlib-Seite),
in jeder der 36 Stellen; heute unbeobachtbar, weil kein Kern-Schreiber
existiert:

| Datei | Ausdruck | Opcode | lokales `intout` | 8-Byte-Lesung überläuft? | Kern |
|---|---|---|---|---|---|
| `pdlg_get_setsize.c:20,24` | `aes_intout_long(0)` | 204 | `aes_intout[16]` | nein (vorgenullt) | −1 |
| `x_edit_evnt.c:47,54` | `aes_intout_long(1)` | 215 | `[16]` | nein | −1 |
| `x_fnts_do.c:53,57,61` | `long(2)`,`(4)`,`(6)` | 187 | `[16]`, OUT_LEN 8 > `O_SIZE` 7 → `int_out[7]` nie geschrieben | nein | −1 |
| `x_fnts_evnt.c:58,62,66` | `long(3)`,`(5)`,`(7)` | 186 | `[16]`, OUT_LEN 9 | nein | −1 |
| `x_fnts_get_style.c:26,30` | `aes_intout_long(0)` | 184 | `[16]` | nein | −1 |
| `n_v_color2nearest.c` · `n_v_value2color.c` · `n_vq_ctab_entry.c` · `n_vq_max_color.c` · `n_vq_min_color.c` · `n_vq_weight_color.c` · `n_vqf_bg/fg_color.c` · `n_vql_bg/fg_color.c` · `n_vqm_bg/fg_color.c` · `n_vqr_bg/fg_color.c` · `n_vqt_bg/fg_color.c` | `vdi_intout_long(0)` | 204/2, 204/1, 206/1, 209/2,1,3, 202–203/0–4 | `vdi_intout[6]` | nein | unimpl. |
| `n_v_color2value.c` · `n_v_ctab_idx2value.c` · `n_v_get_ctab_id.c` · `n_vq_ctab_id.c` · `n_vq_prn_scaling.c` · `n_vst_setsize32.c` · `n_vst_arbpt32.c` | `vdi_intout_long(0)` | 204, 206/5, 206/6, 206/2, 5/39, 252, 246 | **`vdi_intout[2]`** | **ja, 4 Byte** (ASan: `n_vq_ctab_id.c:26`, `n_vst_setsize32.c:54`, `n_vst_arbpt32.c:61`) | unimpl. |
| `n_vq_px_format.c` · `n_vqt_pairkern.c` · `n_vqt_trackkern.c` | `long(0)`, `long(2)` | 204/3, 235, 234 | **`vdi_intout[4]`** | **ja** (`long(2)`) | unimpl. |
| `n_v_getbitmap_info.c` | `long(2)`,`(4)`,`(6)`,`(8)`, `ptr(10)` | 239 | `[10+N_PTRINTS]` | nein | unimpl. |
| `n_vq_hilite_color.c` · `u_vq_hilite_color.c` · `vq_page_name.c` (`long(1)`,`(3)`) · `vqt_cachesize.c` | `vdi_intout_long(…)` | 209, 5/38, 255 | `[VDI_INTOUTMAX]` | nein | unimpl. |

**3c — Zeiger-Accessoren** (`*_intin_ptr`/`*_intout_ptr`, nicht in den 33+36
gezählt, aber dieselbe Frage): nativ in Aufruferbreite = Regel.

| Datei:Zeile | Ausdruck | Opcode | Wortzahl | Kern | Regel |
|---|---|---|---|---|---|
| `a_wind_get.c:344,346` | `aes_intin_ptr(2, short*) = W1` für `WF_NAME`/`WF_INFO`, `control[1] = 4` | 104 | **4 statt 2+N_PTRINTS = 6**: `from_raw` kopiert nur `int_in[0..3]`, obere Zeigerhälfte → 0 | `wind_get` `WF_NAME`/`WF_INFO` → `default` FALSE (`window.rs`, A42; EmuTOS `gemwmlib.c:1591`) — Slot nie gelesen | Wortzahl falsch, folgenlos |
| `a_wind_sget.c:21,25` | `AES_PARAMS(104,4,…)`, `aes_intin_ptr(2, char*) = str` | 104 | dito | dito | dito |
| `a_wind_get_ptr.c:51` | `*v = aes_intout_ptr(1, void*)` — **8 Byte aus `int_out[1..4]`** | 104 | — | `WF_SCREEN`: zwei 32-Bit-LONGs → Lesung = `addr \| len << 32` (**getrieben**, Test 3) | **falsch** (gemlib-Seite) |
| `n_v_create_itab.c` (`ptr(0)`, `3+N_PTRINTS`), `n_vqt_xfntinfo.c` (`ptr(3)`, `3+N_PTRINTS`), `n_v_get_outline.c`/`n_v_getoutline.c` (`ptr(2)`, `…+2·N_PTRINTS`), `n_v_opnprn.c` (`ptr(12)`), `n_v_read/write_default_settings.c`, `n_v_delete_driver_info.c`, `n_vq_ext_devinfo.c`, `n_vs_load/save_disp_list.c`, `vq_page_name.c`, `vq_tray_names.c`, `vs_calibrate.c`, `vst_error.c`/`u_vst_error.c`, `v_set_app_buff.c` | `vdi_intin_ptr(n, T) = p` | 208, 229, 243, 1, 182, 181, 248, 5/2106-7, 5/38, 5/36, 5/76, 245, −1/6 | `N_PTRINTS` korrekt deklariert | unimpl. (`n_v_opnprn` → `v_opnwk`, der `intin[11..]` nicht liest) | stimmt |
| `n_v_create_ctab.c`, `n_v_create_driver_info.c`, `n_v_create_itab.c`, `n_v_getbitmap_info.c`, `vqt_get_table.c` | `vdi_intout_ptr(n, T)` | 206/8, 180, 208, 239, 254 | — | unimpl. | stimmt (Regel), heute unbeobachtbar |
| `n_vq_image_type.c` | `vdi_ptsin_ptr(0, BIT_IMAGE*)` | 5/2105 | — | Escape > 19 → no-op | stimmt |
| `vex_butv.c:24,29` u. Geschwister, `vro_cpyfm.c`, `vrt_cpyfm.c`, `vr_trnfm.c`, `n_v_opnbm.c`, `n_v_open_bm.c`, `n_vr_transfer_bits.c` | `vdi_control_ptr(n, T)` = `control + 7 + n·N_PTRINTS` | 125–127/118/134, 109/121/110, 100/1, 100/3, 170 | `N_PTRINTS`-skaliert | Raster: `contrl_ptr` stimmt; **`vex_*`: nein** (§6) | s. §6 |

**Nicht Klasse 3** (Typprüfung bestätigt): `n_v_ftext16n.c:20` (`PXY` =
zwei `short`), die `COLOR_ENTRY`-Kopien (Union aus 4 × `unsigned short`,
reihenfolgefrei).

---

## 5. Lebendige Leser der `int_out`-LONGs (die beobachtbare Seite)

Der einzige heute implementierte Schreiber ist `wind_get(WF_SCREEN)`. Seine
Leser in den Ports:

| Leser | Form | Ergebnis unter dem Vertrag | Status |
|---|---|---|---|
| TeraDesk `resource.c:233-241,307,315` (`_MENUDEL 1`, `library/xdialog/library.h:177` → kompiliert) | `wind_get(0, WF_SCREEN, &d, &d, &buffer.words.high, &buffer.words.low)`; `union { long size; struct { _WORD high, low; } words; }`; `if (mnsize >= buffer.size)` | `words.high` (Offset 0) ← `int_out[3]` = Bits 0–15, `words.low` (Offset 2) ← `int_out[4]` = Bits 16–31 → **untere 32 Bit von `size` richtig** (doppelte Inversion); **obere 32 Bit von `long size` (8 Byte) uninitialisiert** — negativ → `mn_del` löscht 13 Menüeinträge, positiv-groß → nie | **PLAUSIBLE**, app-seitig; nicht getrieben (Stackinhalt) |
| cflib `purec.c:157,165-171` (`wind_get`, `WF_SCREEN`) | `*((void **)W) = *((void **)&aes_intout[1]); *((long *)W) = *((long *)&aes_intout[3])` nach `aes_intout[1..4] = 0` | Zeiger = `addr \| len << 32` (**falsch**, Test 3 belegt die Bytefolge); Länge = `len \| int_out[5..6] << 32` (uninitialisiert) | **CONFIRMED** (Draht) — kein cflib-Nutzer im SDK ruft `WF_SCREEN` (QED nicht) → latent |
| cflib `purec.c:172-181` (`WF_NEWDESK`, `WF_TOOLBAR`, `WF_USER_POINTER`, `WF_WIND_ATTACH`) | `*((void **)&aes_intout[1])` | Kern: `wind_get` `default` → FALSE, nichts geschrieben → Lesung = 8 Byte Nullen (vorgenullt) | folgenlos |
| gemlib `a_wind_get_ptr.c:51` | `aes_intout_ptr(1, void*)` | wie cflib-Zeiger | **CONFIRMED** (Draht), kein Nutzer im SDK |

Quellsweep (`/usr/bin/grep -rlw`) über `sdk/apps/qed/src`,
`sdk/apps/teradesk`, `sdk/cflib/cflib-21`, `sdk/apps/gemtest`, `apitest`:
**kein** Aufruf eines 200er-VDI-Bindings, keines `x_*`/`fnts_*`/`pdlg_*`/
`edit_evnt`-AES-Bindings, kein `wind_get_ptr`/`wind_sget`, kein `vex_*`.
Der einzige lebendige Fall ist TeraDesks `WF_SCREEN`.

---

## 6. Angrenzend, außerhalb `intin`/`intout`: `vex_*` in `CONTRL`

`vdi/mouse.rs:693-695` liest den neuen Vektor **fest 8 Byte** an `CONTRL[7]`
(`contrl_u64`, `vdi/mod.rs:136-139`, Kommentar: „only reachable from LP64
callers") und schreibt den alten **fest 8 Byte an `CONTRL[9]`**. gemlib
(beide Breiten) legt den neuen an `7 + 0·N_PTRINTS` und liest den alten an
`7 + 1·N_PTRINTS` (`vex_butv.c:24,29`): LP64 → `CONTRL[11..14]` (Kern
schreibt `[9..12]`: obere Hälfte des alten Zeigers in `[11..12]`, `[13..14]`
Aufruferstack), ILP32 → `CONTRL[9..10]` richtig, aber der Eingang liest 8
Byte über `[7..10]` (obere Hälfte aus dem uninitialisierten `[9..10]`).
EmuTOS: `CONTRL[7..8]` ein, `CONTRL[9..10]` aus, je 32 Bit
(`vdi/vdi_mouse.c:372-373`). Verstoß gegen die Zeigerregel, PLAUSIBLE
(nicht getrieben); praktisch unerreichbar, weil ein EL0-Vektor vom Kern nicht
aufrufbar wäre — Lead-Entscheidung: T04 oder eigenes Ticket.

---

## 7. Tests

* **`tos-api/tests/aes_t04_intin_long_contract.rs`** (neu, **grün** — es gibt
  keinen kern-seitigen Verstoß, dessen roter Test zu schreiben wäre):
  1. `evnt_timer([300, 0])` wartet ≈ 300 Takte, `evnt_timer([0, 1])` 65 536:
     `int_in[0]` = Low-Wort, `int_in[1]` = High-Wort.
  2. `evnt_multi(MU_TIMER, …, [200, 0])` ≈ 200 Takte, `MT_LOCOUNT = int_in[14]`.
  3. `wind_get(WF_SCREEN)`: `int_out[1..2]`/`[3..4]` als zwei 32-Bit-LONGs
     `[lo, hi]`; die 8-Byte-Lesung an `&int_out[1]` ergibt `addr | len << 32`
     ≠ `addr` (die gemlib-/cflib-seitige Fehlform, festgehalten).
* Bestehend, A81: `aes_newdesk_root_word.rs`, `aes_window_name_pointer.rs`,
  `window::newdesk_root_word_tests::ilp32_packed_root_is_int_in_4` (Zeiger in
  Aufruferbreite, beide ABIs). A57: `aes_window_screen_buffer.rs`.
* gemlib-seitige Rot-vor/Grün-nach-Tests sind Schritt 3 (Developer), Muster
  `sdk/gemlib/tests/evnt_timer_words.c`; die Sonden aus §3 sind die Vorlage
  (Fake-`vdi()` plus Ziel-Disassembly für die Rahmenkollisionen).

---

## 8. Zählung

| Seite | Verstöße gegen die Regel | davon heute beobachtbar |
|---|---|---|
| Kern, `int_in`/`int_out` | **0** von 6 Lesern/Schreibern | — |
| Kern, angrenzend (`CONTRL`, `vex_*`) | 1 (PLAUSIBLE) | 0 |
| Kern-Doku | 1 (`mod.rs:355-360`, cflib-Behauptung) | — |
| gemlib Klasse 1 | 0 von 4 (Draht) — 1 Binding (`vst_arbpt32`) durch 3b zerstört | 0 |
| gemlib Klasse 2 | 4 von 4 mit falscher Wortzahl (Bytes richtig); `a_wind_set_str.c` erledigt | 0 |
| gemlib Klasse 3a (`*_intin_long`, 33) | Draht 33/33 richtig; **3 Bindings zerstört** (`v_create_ctab`, `vst_track_offset`, `v_value2color`) + `vst_arbpt32` über 3b; Spill-Hazard in allen 33 | 0 |
| gemlib Klasse 3b (`*_intout_long`, 36) | **36/36 falsch** (8 statt 4 Byte); 10 davon mit Lese-Überlauf | 0 |
| gemlib Klasse 3c (`aes_intout_ptr`, `a_wind_get_ptr.c`) | 1 | 1 (`WF_SCREEN`, kein Nutzer) |
| gemlib Klasse 3c (`aes_intin_ptr`, Wortzahl) | 2 (`a_wind_get.c`, `a_wind_sget.c`) | 0 |
| cflib `purec.c` `WF_SCREEN` | 2 Lesungen | 1 latent |
| TeraDesk `resource.c` union | obere 32 Bit | PLAUSIBLE |

---

## 9. Was der Lead entscheiden muss

1. **Die Regel aus §1 als Vertrag festschreiben** (Ticket-Schritt 2,
   `docs/COMPATIBILITY.md` Abschnitt AES/VDI-Parameterblöcke; Querverweis in
   `docs/api/aes/_dispatcher.md` und `docs/api/vdi/_dispatcher.md`, die heute
   keine Aussage enthalten; `docs/sdk/05-abi-pitfalls.md` B16 „unresolved"
   auflösen). Typkriterium: EmuTOS-Typ an der Zugriffsstelle (`LONG` → 32 Bit,
   Zeiger → Aufruferbreite); für NVDI-Funktionen ohne EmuTOS-Quelle der
   gemlib-Accessor (`*_long` → 32 Bit, `*_ptr` → Aufruferbreite).
2. **`WF_SCREEN` bleibt LONG-Klasse (32 Bit)** — A57, EmuTOS `LONG *`,
   EL0-VAs < 4 GiB. Folge: gemlib `a_wind_get_ptr.c` und cflib `purec.c`
   müssen dort 32 Bit lesen (Overlay), nicht der Kern 8 schreiben (das
   verschöbe die Länge aus `int_out[3..4]` und bräche `mt_wind_get`/TeraDesk).
3. **Reparaturform für Klasse 3** (Developer, Schritt 3): die Accessoren in
   den Overlay-Headern `gem_aesP.h`/`gem_vdiP.h` auf **32 Bit** umstellen
   (unausgerichtete `int32_t`/`uint32_t`-Stores/Loads statt `long`) — das
   behebt mit einer Header-Änderung alle 33+36 Stellen samt Spill,
   Array-Überläufen, den drei/vier zerstörten Bindings und den 8-Byte-Lesungen,
   ohne die Tarball-Dateien anzufassen. Klasse 2: vier Dateien auf
   `N_PTRINTS` (Overlay). Klasse 1: **keine** Byteorder-Reparatur
   (die Ticket-Annahme war falsch); `vst_arbpt32` heilt durch 3.
4. **TeraDesk `resource.c` union** (obere 32 Bit): App-Overlay oder
   hinnehmen — Memory: Ported-App-Quelle bleibt unangetastet, Fixes in
   Overlays. Vorher einmal auf der Pi beobachten (Menüeinträge nach Start
   vollständig?).
5. **`vex_*`-Vektorschreiber in `CONTRL`** (§6): T04 oder eigenes Ticket.
6. **Ticket-Text korrigieren**: Klasse 1 ist nicht die H03-Form; Klasse 2 ist
   byte-richtig mit falscher Wortzahl; der Kern-Vertrag ist der aus §1;
   `set_intout_long`-Kommentar (`mod.rs:355-360`) um „bei ILP32" ergänzen.
