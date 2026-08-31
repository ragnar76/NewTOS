NewTOS -- VDI reference dumper on real hardware
===============================================

COPY TO THE SD CARD
  kernel8.img   -> boot partition, replacing the existing one
  DUMPVDI.NAP   -> ROOT of C:   (not a subdirectory)
  VDI.TSV       -> ROOT of C:   (not a subdirectory)

  kernel8.img is a RELEASE build and carries this session's five VDI
  fixes plus the bulk screen clear. Without it you would measure the
  old kernel.

WHY THE ROOT OF C:
  The dumper reads VDI.TSV and writes VDI.BIN and VDI.LOG through
  relative paths, and the desktop does not set the current directory
  before launching a program (launch_native has no Dsetpath). The root
  is the most likely place the working directory points at after boot.

HOW TO RUN
  Double-click DUMPVDI.NAP on the desktop. It is a GEM app and takes
  over the whole screen. It waits for RETURN at the end.

WHAT YOU SHOULD SEE
  DUMPVDI build 2704, record format 2
  VDI.TSV: 120 cases, CRC A0706A6D
  appl_init id <n>, graf_handle <n>, v_opnvwk handle <n>
  work_in / DEV_TAB / SIZ_TAB ...
  running 120 cases
  all 120 cases ran
  VDI.BIN: 120 records, 191072 bytes

  The DEV_TAB line is the interesting part of this first run: it says
  what geometry and colour depth NewTOS actually runs at on your
  monitor. That settles how much of the comparison is device
  difference, with nothing computed or assumed.

IF IT FAILS
  'VDI.TSV: the case table could not be read' means the working
  directory is not the root. That is a useful answer too -- tell me
  and I will make the dumper find it.

  Other messages: appl_init failed / graf_handle returned no handle /
  v_opnvwk returned no handle.

  VDI.LOG is written even on failure, and once before the cases start,
  so a mid-run hang still leaves the workstation setup on disk.

BRING BACK
  VDI.BIN and VDI.LOG from the root of C:.
  Leave VDI.TSV untouched -- its bytes are the identity the dump
  carries, and editing it invalidates the measurement.
