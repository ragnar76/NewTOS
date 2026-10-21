# TDV -- TeraDesk (#Z), armed up:DESKTOP, card td-<v> with TERADESK.INF = TERADESK-QED.INF but options mode=0 (TEXTMODE, desk.h:73)
# and flds=0x001F (size, date, time, attributes): dclick the drive C icon (195,40) (measurement-2026-10-20-sdk01a-test tnav.spec)
# -> the C:\ window as a text list with dates. TeraDesk reads those dates itself (xfilesys.c x_attr -> Fsfirst -> dta_to_xattr;
# x_xreaddir), not through libc stat(): the dump is the no-regression check, old vs new libc.
at 300
park
move 195 40
dclick
wait 500
mark t1-window
move 507 205
wait 100
mark t2-end
wait 200
