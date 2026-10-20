# GEM Hello -- template Makefile (sdk/mk/newtos.mk does the work).
#
#   make                         -> HELLO.NAP
#   make card DEST=/Volumes/X/   -> copy HELLO.NAP (+ RSC) there
#   make clean
#
# Outside the SDK tree: make NEWTOS_SDK=/path/to/sdk

PROG   = HELLO
KIND   = nap
SRCS   = hello.c
LIBS   =
RSC    =
CFLAGS = -Wall

NEWTOS_SDK ?= ../..
include $(NEWTOS_SDK)/mk/newtos.mk
