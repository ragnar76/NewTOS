#NO_APP
	.text
#APP
	.text
	.even
	.globl	_kbdvec_hook
_kbdvec_hook:
	movem.l	%d1-%d2/%a1,-(%sp)
	moveq	#0,%d1
	move.b	_kbdvec_w,%d1
	move.l	%d1,%d2
	addq.b	#1,%d2
	andi.b	#63,%d2
	cmp.b	_kbdvec_r,%d2
	beq.s	1f
	lea	_kbdvec_ring,%a1
	move.b	%d0,0(%a1,%d1.w)
	move.b	%d2,_kbdvec_w
1:	movem.l	(%sp)+,%d1-%d2/%a1
	move.l	_kbdvec_old,-(%sp)
	rts

	.globl	_kbdvec_etv_term
_kbdvec_etv_term:
	movem.l	%d0/%a0,-(%sp)
	move.l	_kbdvec_slot,%d0
	beq.s	2f
	move.l	%d0,%a0
	move.l	#_kbdvec_hook,%d0
	cmp.l	(%a0),%d0
	bne.s	2f
	move.l	_kbdvec_old,(%a0)
	clr.l	_kbdvec_slot
2:	move.l	#_kbdvec_etv_term,%d0
	cmp.l	0x408,%d0
	bne.s	3f
	move.l	_kbdvec_etv_old,0x408
3:	movem.l	(%sp)+,%d0/%a0
	tst.l	_kbdvec_etv_old
	beq.s	4f
	move.l	_kbdvec_etv_old,-(%sp)
4:	rts

#NO_APP
.comm _kbdvec_etv_old,4
.comm _kbdvec_slot,4
.comm _kbdvec_old,4
.comm _kbdvec_r,2
.comm _kbdvec_w,2
.comm _kbdvec_ring,64
