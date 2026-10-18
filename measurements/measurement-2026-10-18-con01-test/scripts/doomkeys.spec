# DOOMKEYS -- Doom (#Z C:\DOOM\DOOM.NAP), armed launch:DOOM: hold Up (move), Esc (menu), Down, Down (menu cursor), Esc (close menu).
at 3000
nop
mark o0-before
wait 100
key 0x48
mark o1-up-tap
wait 200
key 0x01
mark o2-esc-menu
wait 200
key 0x50
mark o3-down1
wait 200
key 0x50
mark o4-down2
wait 200
key 0x01
mark o5-esc-close
wait 300
