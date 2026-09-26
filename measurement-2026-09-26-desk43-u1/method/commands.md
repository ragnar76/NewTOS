# Wie die Messung gefahren wurde

Host: macOS, QEMU `qemu-system-aarch64`, Board `raspi4b`, 640×480, **Dev-Profil**
(kein `--release`), headless (`-display none`). Kein echter Pi, kein Flashen,
kein `/Volumes/BOOT`. Arbeitsdateien lagen im Job-Scratch, das SD-Image ist
**nicht** mitgeliefert (Auflage „kein `*.img`").

## 0. Das Testmedium

```
tools/make-sd-image.sh "$SCRATCH/sd.img" 64
```

Das ist die einzige zulässige Quelle für `C:` ([[auto-folder-console-only]]);
das Image enthält u. a. `C:\AUTO\HELLO.NCP` (läuft vor der AES) und zwei
Accessories (`C:\ACC\SYSACC.NAC`, `C:\SYSACC2.NAC`). `C:\NEWDESK.INF` ist ein
Platzhaltertext, also greift `inf::build_inf()` — kein Fenster aus `cnx_get`.

## 1. Die Aufzeichnung einschalten (Scratch, zurückgedreht)

Der Tracer ist im Baum **aus** (`QED_SVC_TRACE = false`). Für die Messung wurde
er per Hand eingeschaltet und danach per Hand zurückgesetzt — kein `git`-Eingriff
([[Git hygiene]]). Der exakte Scratch-Stand ist `scratch-measurement.diff`:

1. `kernel/src/dbg_qedtrace.rs` — `QED_SVC_TRACE: false → true`.
2. `kernel/src/dbg_qedtrace.rs` — die QED-spezifische Umleitung der
   Geometrie-Aufrufe in den `PRO`-Puffer abgeschaltet (`let geometry = false && …`).
   **Grund:** `record()` schiebt `wind_create`/`wind_open`/`wind_calc` und die
   Work-Abfragen in einen eigenen, nie geleerten Puffer, der am *Kopf* jeder
   Datei steht. Für QED war das richtig; für einen Sequenzvergleich gegen
   `deskmain.c` zerstört es genau die Reihenfolge, die geprüft werden soll.
   `trace/QEDTRACE-widened-gate-with-pro-hoist.log` ist derselbe Boot **mit**
   der Umleitung — der Unterschied ist ausschliesslich die Position der beiden
   Geometriezeilen.

Beide Änderungen sind im ausgelieferten Baum **nicht** enthalten:
`QED_SVC_TRACE` steht wieder auf `false`, die `PRO`-Umleitung ist wieder aktiv.

## 2. Bauen und fahren

```
cargo run -p xtask -- build --board rpi4
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial stdio \
    -kernel kernel8.img -drive file="$SCRATCH/sd.img",if=sd,format=raw
# nach ~45 s per Signal beendet (macOS hat kein `timeout`):
# ( cmd & pid=$!; sleep 45; kill $pid )
```

Der Desktop kommt hoch (`[desk] desktop up (1 drives, 0 window(s) …)`) und
blockiert dann in `evnt_multi`. **Genau dieser Eintritt schreibt die Datei:**
`pre_dispatch` leert den Puffer bei jedem `evnt_multi`-Eintritt nach C:. Weil
headless nie ein Ereignis kommt, gibt es keinen zweiten Eintritt — die Datei
hält den kompletten Anlauf.

## 3. Die Datei holen

```
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage "$SCRATCH/sd.img" | head -1 | awk '{print $1}')
cp /Volumes/NEWTOS/QEDTRACE.LOG  <ziel>
hdiutil detach "$DEV"
```

## 4. Die Kontrolle (das „vorher")

`scratch-control-el0-gate.diff` setzt das alte Tor `dbg_from_el0` wieder ein,
sonst identischer Bau und identischer Lauf. **Vorher wurde `QEDTRACE.LOG` aus
dem Image gelöscht** — beim ersten Anlauf war genau das der Fehler: die Datei
des vorigen Laufs blieb liegen und sah aus wie ein Ergebnis.

Ergebnis: `serial/boot-control-el0-gate.log` zeigt denselben Boot bis
`[desk] desktop up` — und auf dem Image entsteht **überhaupt keine**
`QEDTRACE.LOG`. Mit dem EL0-Tor sieht der Tracer den Desktop nicht, und weil
die einzige `evnt_multi`-Quelle in diesem Lauf der Desktop ist, wird auch nie
geschrieben. (Die beiden Accessories warten mit `evnt_mesag`, nicht mit
`evnt_multi`, lösen also keinen Flush aus.)

## 5. Die Gegenprobe im ausgelieferten Stand

Nach dem Zurückdrehen: `QEDTRACE.LOG` gelöscht, neu gebaut, neu gefahren
(`serial/boot-final-tracer-off.log`). Desktop kommt hoch, **keine Datei** —
`QED_SVC_TRACE = false` faltet beide Haken weg, das erweiterte Tor hat keine
Laufzeitwirkung.

## 6. Die Driftprobe

```
tools/desk-drift-probe.sh -v > analysis/drift-probe.txt
```
