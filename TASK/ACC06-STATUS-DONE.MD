# ACC06 — STATUS: DONE (2026-09-24) — Teil 1 gebaut, Teil 2 als Quelltext

**Auftrag:** Ein Accessory, das Taktfrequenz und CPU-Temperatur zeigt. Zwei
Teile: (1) die zwei Werte muessen aus dem Kernel heraus und aus EL0 lesbar
sein, mit einem Konsolenprogramm als Beleg; (2) das Accessory selbst — nur
Quelltext, denn ohne ACC02/ACC03 kann es nicht laufen.

Basis: Branch `desk-rebuild`, HEAD `63c78f0`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen,
keine Pi gefahren, nichts committet; kein
`git checkout/restore/clean/stash/reset`.

Messung:
[`measurements/measurement-2026-09-24-acc06/`](../measurements/measurement-2026-09-24-acc06/).
Spezifikation: `docs/api/gemdos/154-ssystem.md`, Abschnitt *NewTOS-own modes
(2000+)*. Hardwarereferenz: `docs/hardware/mailbox-framebuffer.md` §3d.
Hardwarepunkt **64** in `docs/api/desktop/_hw-test-2026-09-17.md`.

---

## 1. Die Nummernwahl — was in der Quelle wirklich steht

Das Ticket nannte FreeMiNTs belegte Nummern als „0–29, 100, 900–913,
1000–1100 und `0x54f8`". Nachgesehen in
`references/freemint/sys/mint/ssystem.h` (@ `f906416`) ist das zu eng
gefasst: der Header reserviert nicht Nummern, sondern **Bloecke**, und zwar
per Kommentar

* *„additional informations about the kernel / **reserved 900 - 999**"*
* *„debug section / **reserved 1000 - 1999**"*

Dazu kommt eine Nummer, die im Header gar nicht steht, aber vom Verteiler
beantwortet wird: `case 3000:` (`sys/ssystem.c:631`) — ein
`register_trap2`-Haken hinter `#ifdef MINT_STATUS_CVS`, im Quelltext selbst
als *„XXX only for testing"* bezeichnet. Hoechste regulaere Nummer bleibt
`S_TIOCMGET = 0x54f8` (21752).

**Fazit unveraendert, Begruendung schaerfer:** frei ist die Luecke
**2000…2999**. Festgeschrieben:

| Konstante | Wert | Einheit |
|---|---:|---|
| `S_NEWTOS_CPUCLOCK` | 2000 | Hz |
| `S_NEWTOS_CPUTEMP` | 2001 | 1/1000 Grad Celsius |

Vermerkt ist zugleich: ein spaeterer NewTOS-Block darf nicht ueber 2999
hinauswachsen, ohne 3000 erneut zu pruefen.

## 2. Der Weg vom Trap zur Mailbox

```
Ssystem(0x154, mode, arg1, arg2)
  tos-api/src/gemdos/ssystem.rs   newtos_scalar()
    -> Host::cpu_clock_hz() / Host::cpu_temperature_mdeg()
         tos-api/src/host.rs      (Trait-Standard: None)
    -> kernel/src/host_env.rs     (nur #[cfg(feature = "board-rpi4")])
    -> kernel/src/drivers/bcm2711/mailbox.rs
         get_clock_rate(CLOCK_ID_ARM=3)  Tag 0x0003_0002
         get_temperature(TEMPERATURE_ID=0) Tag 0x0003_0006
         beide ueber query_id_value()
```

**Argumente** (in der Spezifikation festgehalten):

* `arg1` — Zeiger auf ein 32-Bit-LONG, das den Wert aufnimmt, oder **0** fuer
  den Wert im Rueckgabe-LONG. Die Zeigerform folgt FreeMiNTs eigener
  Konvention fuer „Antwort in den Puffer des Aufrufers" (`S_GETBOOTLOG` 29,
  `S_KNAME` 900) und ist die eindeutige: ein Rueckgabe-LONG kann `0xFFFF_FFFF`
  nicht von `ERROR` unterscheiden.
* `arg2` — **reserviert**, muss 0 sein. Ein anderer Wert wird
  **zurueckgewiesen** (`EINVAL`), nicht ignoriert, und zwar **bevor** der
  Sensor gelesen wird: ein Programm, das gegen eine spaetere, breitere Fassung
  geschrieben wurde, soll erfahren, dass es mit einem aelteren Kern spricht,
  statt die falsche Groesse zu bekommen.

**Keine erfundene Zahl, an keiner Stelle.** `board-qemu-virt` und jede
hostlose Konfiguration behalten den Trait-Standard `None`; daraus macht der
Trap `ERROR` (-1) und schreibt **nichts** in den Zeiger.

Zwei Filterregeln, beide belegt statt geraten:

* Takt **0** heisst „keine Antwort", nicht „0 Hz" — so behandelt es die
  Referenzimplementierung vor ihrem Ausweichpfad
  (`references/circle/lib/machineinfo.cpp:459-471`).
* Temperatur **0** wird *nicht* gefiltert: 0,000 Grad ist eine Temperatur, und
  circle filtert dort ebenfalls nicht
  (`lib/cputhrottle.cpp:408-418`).

Die Tag-Kennungen, die ID-Konstanten und die Werteformate stehen in
`docs/hardware/mailbox-framebuffer.md` §3d, mit Zeilenangaben in
`references/circle/include/circle/bcmpropertytags.h` (`:39`, `:41`, `:184`,
`:196`) und `lib/cputhrottle.cpp:394-418`. Nichts davon aus dem Gedaechtnis
(CLAUDE.md Regel 6).

## 3. Der Pruefling: `C:\SYSINFO.NCP`

`sdk/examples/sysinfo.c`, installiert von `tools/make-sd-image.sh`; mit
`SD_SYSINFO_AUTO=1` zusaetzlich in `\AUTO\`, womit ein kopfloser QEMU-Lauf die
Zeilen ohne einen Klick auf die serielle Konsole bringt.

Er setzt **beide** Aufrufformen ab und vergleicht sie, und er prueft das
reservierte dritte Argument. Gemessen in QEMU `raspi4b`:

```
SYSINFO: NewTOS system values through Ssystem (ACC06 part 1)
SYSINFO: ARM clock : 700.0 MHz  (700000000 Hz)
SYSINFO:   value form agrees: yes (700000000)
SYSINFO: CPU temp  : 25.0 C  (25000 m C)
SYSINFO:   value form agrees: yes (25000)
SYSINFO: arg2 reserved, Ssystem(2000, 0, 1) = -25  (EINVAL, as specified)
SYSINFO: done.
```

**QEMU beantwortet beide Tags** — mit den festen Modellzahlen 700,0 MHz und
25,0 C, ueber Laeufe hinweg identisch. Sie stehen so im Protokoll; erfunden ist
nichts, gemessen ist es auch nicht. Der erwartete ehrliche `ERROR`-Pfad war im
Emulator deshalb nicht zu sehen und ist stattdessen am Trap belegt (§4).

## 4. Hosttest am Trap

`tos-api/tests/gemdos_ssystem_cpuinfo.rs`, fuenf Faelle, alle gruen:

1. Wertform (`arg1 == 0`) liefert beide Werte im Rueckgabe-LONG;
2. Zeigerform schreibt den Wert und liefert `E_OK`;
3. ein Brett ohne Sensor liefert `ERROR` und schreibt **nichts** — der Puffer
   behaelt sein `0xDEADBEEF`;
4. `arg2 != 0` → `EINVAL`, ebenfalls ohne Schreibzugriff;
5. 1999, 2002 und 2999 bleiben `EINVFN`, und die Kennungsprobe
   `Ssystem(-1, …)` liefert weiterhin `E_OK`.

## 5. Teil 2 — `sdk/examples/sysacc.c`

Geschrieben, uebersetzt sauber mit `-Wall -Wextra`. **Laeuft nicht**, und der
Dateikopf sagt in drei Punkten warum:

* `menu_register` (35) traegt Titel und Anwendungs-ID ein und gibt eine
  stabile Schlitznummer zurueck (`tos-api/src/aes/menu.rs:1105-1130`) — mehr
  nicht;
* niemand haengt diesen Schlitz in die Desk-Klappliste und niemand sendet
  `AC_OPEN` (EmuTOS `gemctrl.c:337-346` → `ap_sendmsg`,
  `gemwmlib.c:851-862`; Gegenstueck `mn_cleanup`, `gemmnlib.c:740-748`);
* einen Accessory-Lader gibt es nicht.

Gegen den heutigen Kern meldet er sich an und bleibt in `evnt_mesag` stehen.
Das ist ACC02/ACC03. **Ein Geruest wurde nicht darum herum gebaut.**

Der Kasten ist ein einziges `form_alert` (Lead-Entscheidung), die
Dateiendung bleibt ACC01 ueberlassen — der Bauhinweis im Kopf nennt bewusst
`.TBD`, und `make-sd-image.sh` installiert die Datei nicht.

Eine Einzelheit, die beim Schreiben auffiel und im Quelltext vermerkt ist:
EmuTOS legt die Menuenummer des Accessorys bei `AC_OPEN` in **msg[4]**, bei
`AC_CLOSE` aber in **msg[3]**. Die Unsymmetrie ist EmuTOS' eigene.

## 6. Nachweise

| Was | Ergebnis |
|---|---|
| `cargo run -p xtask -- test` | **944 / 0 / 1** (vorher 939/0/1; +5 neue Faelle) |
| Bau `--board rpi4`, frisch | tos-api **64**, kernel **94** — Delta 0 |
| Bau `--board qemu-virt`, frisch | tos-api **64**, kernel **94** — Delta 0 |
| `cargo clippy --features board-rpi4` | tos-api **80**, kernel **125** — Delta 0 |
| `cargo clippy --features board-qemu-virt` | tos-api **80**, kernel **125** — Delta 0 |
| QEMU `raspi4b` | beide Werte gedruckt, Schreibtisch kommt hoch, **0** `[EXC]` / `[PANIC]` |
| QEMU `virt` | startet sauber bis „desktop up", **0** `[EXC]` / `[PANIC]` |
| `fsck_msdos -n` | sauber |

Ein Zwischenstand, der hierher gehoert: der erste Clippy-Lauf stand bei
kernel **126** — `deref_addrof` in `mailbox::query_id_value`. Statt die
Warnung per `#[allow]` stillzustellen (wie es `get_memory` tut), liest die
Funktion die Antwort jetzt ueber die Puffer-VA, genau wie `call_property` den
Antwortcode der Nachricht liest. Danach wieder 125.

## 7. Was nur die echte Pi 4 entscheiden kann

Dass der Treiber die **Firmware** liest und nicht etwas anderes, zeigt sich
erst, wo die Firmware keine Konstante sagt. Auf der Pi 4 duerfen dort nicht
700,0 MHz und nicht exakt 25,0 C stehen, und die Temperatur muss sich nach
einem Lastlauf veraendert haben. Hardwarepunkt **64**.

## 8. Ausserhalb des Auftrags gesehen, nicht angefasst

* `GET_CLOCK_RATE` (`0x0003_0002`) meldet den **eingestellten** Takt; circle
  fragt zuerst `GET_CLOCK_RATE_MEASURED` (`0x0003_0047`) und faellt erst dann
  hierauf zurueck (`references/circle/lib/machineinfo.cpp:459-471`). Welcher
  auf der Pi 4 die ehrlichere Zahl liefert, entscheidet ein
  Hardwarelauf — eigenes Ticket.
* Der AUTO-Textschirm ist im Emulator kuerzer als 0,12 s sichtbar und damit
  nicht als Bild zu belegen (zwei Verfahren, RUNINFO §3). Ein Bild der zwei
  Zeilen gibt es erst mit dem Accessory.
