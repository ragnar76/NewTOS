# GUI04 — STATUS: **DONE** (Hardwareabnahme mit echter Radmaus offen)

Datum: 2026-09-26 · Branch `desk-rebuild`, Basis HEAD `714c5bb` ·
Änderung **uncommitted** im gemeinsamen Baum · Belege:
[`measurements/measurement-2026-09-26-gui04/`](../measurements/measurement-2026-09-26-gui04/)

---

## 1. Was gebaut wurde — die Kette, Stück für Stück

| Stück | Datei / Funktion | Vorbild |
|---|---|---|
| USB-Bericht Byte 3 → Scancode `0x59`/`0x5a` | `kernel/src/drivers/usb/hid.rs` — `mouse_report`, `wheel` | FreeMiNT `sys/usb/src.km/udd/hid/mouse-boot/usb_mouse.c:239-261,288-311` |
| Berichtlänge aus dem Transfer Event | `kernel/src/drivers/usb/xhci/mod.rs` — `poll_interrupt` gibt jetzt `(di, report, len)` | xHCI 1.2 §4.11.5.1 / Tab. 6-39 (Residual) |
| IKBD-Erweiterung: die sieben Scancodes gehen nicht in die Tastenschlange | `kernel/src/ikbd.rs` — `is_extended_mouse`, Zweig in `decode_key` | `references/emutos/bios/ikbd.c:746-760` (`kbd_int`) |
| `mousexvec` | `tos-api/src/bios/vectors.rs` — `set_mousexvec`, `mousexvec` | `bios/aciavecs.S:127,149-151`, `include/biosext.h:78` |
| `vdi_mousex_handler` | `tos-api/src/vdi/mouse.rs` — `mousex_handler` | `vdi/vdi_mouse.c:525-553` |
| `call_user_wheel` | `tos-api/src/vdi/mouse.rs` — `call_user_wheel` | `vdi/vdi_asm.S:546-552` |
| `MOUSE_BT` behält Bits 2..4 über ein normales Paket | `tos-api/src/vdi/mouse.rs` — `feed_relative` | `vdi/vdi_asm.S:270-277` („keep additional mouse button states") |
| AES trägt sich ein (`vex_wheelv(aes_wheel, &old)`) | `tos-api/src/aes/init.rs` — `install_wheel_vector`; `tos-api/src/vdi/mouse.rs` — `install_aes_wheel` | `aes/gemgsxif.c:288-292` (`gsx_init`) |
| `aes_wheel` | `tos-api/src/aes/mod.rs` — `aes_wheel` | `aes/gemdosif.S:352-374` |
| `wheel_change` | `tos-api/src/aes/input.rs` — `wheel_change` | `aes/geminput.c:394-420` |
| `wm_find` / `D.w_win[wh].w_owner` | `tos-api/src/aes/window.rs` — `wm_find`, `owner_of_window` | `aes/gemwmlib.c:1767-1770` |
| `vex_wheelv` klassifiziert den eingetragenen Vektor | `tos-api/src/vdi/mouse.rs` — `vex_wheelv` | `vdi/vdi_mouse.c:424-442` + NewTOS-Privilegienschnitt |

Die Radquelle ist USB, die **Form** ist IKBD: `hid.rs` erzeugt dieselben
Scancodes, die eine echte ST-Tastatur mit Radmaus-Erweiterung senden würde, und
von dort an ist jede Station EmuTOS' eigene. Eine spätere echte Radquelle läuft
denselben Weg, und `vex_wheelv` (Opcode 134, seit Langem vorhanden) wird endlich
bedient.

---

## 2. Auftrag 2 — gilt EmuTOS' Einschränkung bei uns? **Nein.** (geprüft, nicht vermutet)

EmuTOS nennt den Grund selbst: `wheel_change` läuft aus `forker()`, und
`forker()` parkt den laufenden Prozess für die Dauer des Fork-Ring-Abarbeitens
auf einem Sentinel — `oldrl = rlr; rlr = (AESPD *) -1;`
(`aes/gemdisp.c:101-106`). `ap_sendmsg` dereferenziert genau diesen Zeiger
(`ap_msg[1] = rlr->p_pid`, `aes/gemwmlib.c:855`) und geht anschließend in
`ap_rdwr`/`iasync`, das umplant. Beides ist gegen `rlr == -1` unzulässig; deshalb
sind die drei Zeilen auskommentiert.

**Bei uns existiert keine dieser Voraussetzungen.** Belege:

1. **Kein Fork-Ring.** `grep -rn "forker\|fpdx" tos-api/src kernel/src` findet
   nichts. Ein Raddetent läuft `Host::pump_input` → `usb::poll` → `ikbd::feed`
   → `mousexvec` → `call_user_wheel` → `wheel_change`, und `pump_input` wird
   ausschließlich **synchron** aus EL1-Code gerufen, der für einen eindeutigen
   Aufrufer arbeitet: aus dem AES-/GEMDOS-Trap des laufenden EL0-Prozesses
   (`tos-api/src/aes/event.rs:313`, `menu.rs:718`, `form.rs:416`, …) oder aus
   der Schleife des kernelseitigen Desktops. Es gibt keine Stelle, die einen
   „kein laufender Prozess"-Sentinel setzt. Genau dieselbe Stelle rufen
   `b_click` und `mchange` schon heute auf, die EmuTOS ebenfalls forkt.
2. **`rlr->p_pid` gibt es nicht.** `pd::post_msg(owner, msg)` bekommt `msg[1]`
   als Argument. `wheel_change` ist Control-Manager-Code (EmuTOS' `aes_wheel`
   wird wie `far_mcha`/`mchange` geforkt), also stempelt es
   `appl::SCRENMGR_PID` — derselbe Stempel, den `window::intercept` für jede
   `WM_*`-Nachricht setzt (Spec § „AES process identity and pid", Kontrakt (b)).
3. **Kein `iasync`.** `post_msg` reiht in die FIFO ein und weckt über
   `sched::wake` einen schlafenden Leser; es dispatcht nicht und kann den
   Aufrufer nicht wiederbetreten.

Es war also **keine andere Zustellform zu entwerfen**: die gewöhnliche
`ap_sendmsg` → `aqueue` → `doq`-Strecke, die `send_redraw` und die
Rahmen-Interception schon benutzen, trägt sie — einschließlich `doq`s
`WM_ARROWED`-Koaleszenz (`aes/gemqueue.c:63-67`), so dass ein Schwung Detents,
den die Anwendung noch nicht gelesen hat, auf den neuesten zusammenfällt (wie
ein Schwung Pfeilklicks).

**Auftrag 3 (die Abweichung benennen)** steht als Block-Doc an
`wheel_change` in `tos-api/src/aes/input.rs`: was EmuTOS hier tut, warum wir
weitergehen (Anwenderentscheidung 2026-09-25; EmuTOS schreibt selbst, dass eine
Lösung *fehlt*), und wie — mit beiden Hindernissen einzeln widerlegt.

---

## 3. Der zweite Verbraucher: eine EL0-Anwendung am `vex_wheelv`

Das Ticket verlangt ein Programm, das sich einträgt und die Ereignisse
**sieht**. Dafür muss der Kern in EL0-Code zurückrufen. Das ist kein Neubau:
der generische EL0-Upcall (Baustein 1 des kooperativen Kerns,
`docs/analysis/coop-kernel/10-upcall-signal-design.md`) ist fertig und
verdrahtet (`post_upcall`/`deliver_upcall`/`Psigreturn`, Konsumenten `etv_term`
und `G_USERDEF`), und **vex ist dort als Konsument Nr. 4 vorgesehen** (§1).

`call_user_wheel` unterscheidet drei Fälle über `USER_WHEEL_KIND`:

* `just_rts` — nichts (EmuTOS' Vorgabe, `vdi_mouse.c:588`);
* die AES-Marke — direkter EL1-Aufruf von `aes::aes_wheel`;
* eine EL0-Adresse — `post_upcall` an den **eigentümernden** Slot, neue
  Maskenbit `UPCALL_MASK_VEX_WHEEL`.

Zwei Eigenschaften sind bewusst, nicht übersehen, und stehen im Quelltext:

* Der Vektor wird nur gerufen, während **sein** Prozess läuft. Eine EL0-VA
  benennt in einem fremden Adressraum etwas anderes; dieselbe Regel wendet
  `ob_user` auf `ub_code` eines `G_USERDEF` an (A97). Stirbt der Prozess, gibt
  `free_slot` → `release_wheel_vector` den Vektor an den AES zurück — EmuTOS
  hat dafür kein Gegenstück (keine Adressräume); der nächste Anker ist
  `vdimouse_exit`s `user_wheel = just_rts` (`vdi_mouse.c:628`).
* Zugestellt wird an der **Syscall-Grenze**, nicht im Interrupt. §7.3 des
  Designs nennt für vex die Lower-EL-IRQ-Route als offene Voraussetzung; die
  gibt es nicht. Ein Detent kommt also spätestens beim nächsten
  Syscall-Rücksprung an (im Testprogramm ≤ 50 ms), und ein zweites Detent vor
  der Zustellung des ersten ersetzt es (`post_upcall`, last writer wins).

**Kettenaufruf über die Privilegiengrenze geht nicht** und kann nicht gehen:
gemlib dokumentiert „call the old interrupt routine", aber `old` ist bei uns
eine EL1-Adresse, die EL0 nicht anspringen kann. Eine Anwendung, die `old`
sichert und beim Beenden zurückschreibt, bekommt die AES-Behandlung zurück —
das ist der Weg, den das Testprogramm geht und der im Log steht.

---

## 4. Nebenbefund, mitrepariert, weil er genau auf diesem Weg liegt

`vex_wheelv(handle, new, &old)` gab `old` an der **falschen** CONTRL-Stelle
zurück. gemlib packt VDI-Zeiger `n` bei Wort `7 + n * N_PTRINTS` mit
`N_PTRINTS = sizeof(void *) / sizeof(short)`
(`sdk/gemlib/gemlib-0.44.0/gem_vdiP.h:177,248`) — 2 Worte auf m68k (daher
EmuTOS' festes `CONTRL[9]`), **4** für eine LP64-Anwendung. `swap_vector` las
`new` richtig bei Wort 7 und schrieb `old` bei Wort 9, also zwei Worte vor der
Stelle, an der eine LP64-App es liest. Jede Anwendung hätte Müll als „alten
Vektor" bekommen — der Weg, den dieses Ticket gerade erst benutzbar macht.
Behoben über den neuen `Vdipb::set_contrl_ptr` (Spiegel des vorhandenen
`contrl_ptr`); betrifft gleichermaßen `vex_butv`/`motv`/`curv`/`timv`, die
denselben Helfer benutzen. Der kernel-eigene VDI-Client packte Slot 1 schon
immer LP64-richtig (`kernel/src/vdi_client.rs:226`). Test:
`tos-api/tests/vdi_vex_wheelv.rs`.

Die dadurch unbenutzt gewordenen `Vdipb::contrl_u64`/`set_contrl_u64` sind
entfernt (sonst +1 Warnung).

---

## 5. Belege

| Was | Wo | Zahl / Bild |
|---|---|---|
| Kette bis in die EL0-App | `measurements/measurement-2026-09-26-gui04/serial/run-a-wheelt.log` | `up=3 down=4 left=0 right=0 total=7` bei 8 Berichten — der eine mit `len == 3` erzeugt **nichts** |
| Der AES-Vektor war eingetragen | dieselbe Datei | `vex_wheelv installed, mine=0x13e4 old=0xffff000000115970` (Kernel-High-Half) |
| LP64-Zeigerslot | dieselbe Datei | `got back=0x13e4` nach dem Zurückgeben |
| Desktop scrollt | `screendumps/b2-window.png` → `b3-scrolled.png` | 6 Detents → 6 Zeilen; `pixdiff` 9885 von 184250 Pixeln |
| Menü aktiv ⇒ ignoriert | `screendumps/c1-window.png` → `c3-after-wheel.png` | **0** unterschiedliche Pixel unterhalb des Klappmenüs |
| **Echte** emulierte USB-Maus (qemu-virt, `-device usb-mouse`, Rad per QMP) | `serial/run-d-virt-real-usb-mouse.log` | `report len=4 byte3=0x01 → scancode 0x59` (3×), `byte3=0xff → 0x5a` (4×) |
| Smoke-Boot beider Boards (gemeinsamer Baum) | `serial/smoke-boot-rpi4.log`, `serial/smoke-boot-qemu-virt.log` | `[desk] desktop up` |
| Hosttests | `build/tests.txt` | 955 / 0 / 1 (Basis 950 / 0 / 1) |
| Bau | `build/build.txt` | kernel 94 Warnungen auf **beiden** Boards (frisch je Board erzwungen), tos-api 64 — alles Basiswerte |
| clippy | `build/clippy.txt` | tos-api 80, kernel 125, beide Feature-Sätze — Basiswerte |

**In A/B/C ist das Rad synthetisiert.** QEMU raspi4b hat keinen USB-Host
(`[pcie] RC not present … USB disabled`); die Berichte gehen direkt in
`hid::mouse_report`, die Funktion, die der echte xHCI-Poll ruft. Alles darunter
ist unverändertes Produktcode.

**Lauf D ist es nicht.** `-M virt` bringt einen echten emulierten `usb-mouse`
hinter einem echten xHCI mit (`HID iface 0 protocol 2 → int-IN ep 0x81 max 4`),
dessen Rad QMP `input-send-event` dreht. Der Bericht kommt aus einem echten
Interrupt-Transfer; eine Sonde auf den zwei Sprüngen
(`method/usb_probe_patch.py`, nur Scratch-Kopie) zeigt `len=4`, `byte3=0x01`
für „hoch" und `0xff` für „runter", also genau die Richtung, die FreeMiNT und
die HID Usage Tables angeben — und die Scancodes `0x59`/`0x5a` samt Break bei
`mousexvec`. Damit ist auch der xHCI-Sprung gemessen; offen bleibt nur, ob eine
**physische** Maus hinter dem VL805 sich genauso verhält.

---

## 6. Was die Pi noch entscheidet

1. Physische USB-Radmaus hinter dem VL805: Bericht mit Byte 3, Länge 4 → im
   Desktopfenster scrollen. Für QEMUs emulierte Maus beantwortet Lauf D das
   schon.
2. Haptik (Detents pro Zeile).
3. Waagerechtes Rad: `0x5c`/`0x5d` sind implementiert, aber **nichts erzeugt
   sie** — ein fünftes Berichtbyte ist üblicherweise AC Pan, das steht aber
   nirgends im Boot-Protokoll und FreeMiNTs Boot-Maustreiber liest es nicht.
   Das zu wissen bräuchte den HID-Report-Descriptor, den das Boot-Protokoll
   gerade vermeidet. Bewusst nicht geraten; in
   `docs/api/usb/hid-mouse.md` festgehalten.
4. Maustasten 3–5: die IKBD-/VDI-Seite ist vollständig
   (`mousex_handler`, `MOUSE_BT` Bits 2..4, Test vorhanden), aber der
   USB-Treiber übersetzt Byte-0-Bits 2..4 **nicht** — Bit 2 (Mitte) steht im
   Boot-Protokoll, Bits 3/4 nicht, und der Auftrag hat es nicht verlangt.

---

## 7. Geänderte Dateien

```
kernel/src/drivers/usb/hid.rs          mouse_report(+len), wheel()
kernel/src/drivers/usb/mod.rs          poll(): Länge weiterreichen
kernel/src/drivers/usb/xhci/mod.rs     poll_interrupt(): Länge aus dem Residual
kernel/src/ikbd.rs                     is_extended_mouse + Zweig in decode_key
tos-api/src/bios/vectors.rs            mousexvec / set_mousexvec
tos-api/src/vdi/mouse.rs               mousex_handler, call_user_wheel,
                                       install_aes_wheel, release_wheel_vector,
                                       wheel_init, vex_wheelv-Klassifikation,
                                       MOUSE_BT-Merge, swap_vector-ABI
tos-api/src/vdi/mod.rs                 set_contrl_ptr (+ contrl_u64 entfernt)
tos-api/src/aes/input.rs               wheel_change (+ die benannte Abweichung)
tos-api/src/aes/mod.rs                 aes_wheel
tos-api/src/aes/init.rs                install_wheel_vector in init/reinit
tos-api/src/aes/window.rs              wm_find, owner_of_window
tos-api/src/bdos/process.rs            UPCALL_MASK_VEX_WHEEL,
                                       free_slot → release_wheel_vector
docs/api/vdi/input/_mouse.md           NewTOS-Notizen (Slot-Breite, Rad)
docs/api/usb/hid-mouse.md              „Wheel: drop for now" aufgelöst
tos-api/tests/vdi_vex_wheelv.rs        NEU (3 Fälle)
tos-api/tests/aes_wheel_change.rs      NEU (2 Fälle)
```

Nicht angefasst: `docs/STATUS.md`, `TASK/GUI04.md` (Lead).
Nichts geflasht, nichts ausgeworfen, kein echter Pi-Lauf, nichts committet.
