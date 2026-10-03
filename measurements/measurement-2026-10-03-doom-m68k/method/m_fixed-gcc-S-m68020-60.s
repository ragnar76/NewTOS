#NO_APP
	.text
	.even
	.globl	_FixedMul
_FixedMul:
	move.l 8(%sp),-(%sp)
	smi %d0
	extb.l %d0
	move.l %d0,-(%sp)
	move.l 12(%sp),-(%sp)
	smi %d0
	extb.l %d0
	move.l %d0,-(%sp)
	jsr ___muldi3
	add.w #16,%sp
	move.w %d0,%d1
	swap %d1
	move.l %d1,%d0
	rts
	.even
	.globl	_FixedDiv
_FixedDiv:
	move.l %d3,-(%sp)
	move.l %d2,-(%sp)
	move.l 12(%sp),%d0
	move.l 16(%sp),%d2
	move.l %d0,%d1
	jlt .L11
	moveq #14,%d3
	asr.l %d3,%d1
	move.l %d2,%d3
	jlt .L12
.L6:
	cmp.l %d1,%d3
	jgt .L4
	eor.l %d2,%d0
	jlt .L8
	move.l #2147483647,%d0
	move.l (%sp)+,%d2
	move.l (%sp)+,%d3
	rts
.L4:
	move.l %d0,%d1
	smi %d0
	extb.l %d0
	swap %d0
	swap %d1
	move.w %d1,%d0
	clr.w %d1
	move.l %d2,-(%sp)
	smi %d3
	extb.l %d3
	move.l %d3,-(%sp)
	move.l %d1,-(%sp)
	move.l %d0,-(%sp)
	jsr ___divdi3
	add.w #16,%sp
	move.l %d1,%d0
	move.l (%sp)+,%d2
	move.l (%sp)+,%d3
	rts
.L12:
	neg.l %d3
	jra .L6
.L11:
	neg.l %d1
	moveq #14,%d3
	asr.l %d3,%d1
	move.l %d2,%d3
	jge .L6
	jra .L12
.L8:
	move.l #-2147483648,%d0
	move.l (%sp)+,%d2
	move.l (%sp)+,%d3
	rts
