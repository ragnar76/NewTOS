# Console Hello -- template Makefile (sdk/mk/newtos.mk does the work).
#
#   make                         -> HELLO.NCP
#   make card DEST=/Volumes/X/   -> copy HELLO.NCP there
#   make clean
#
# KIND = ntp makes HELLO.NTP (the desktop asks for a command line first).
# Outside the SDK tree: make NEWTOS_SDK=/path/to/sdk

PROG   = HELLO
KIND   = ncp
SRCS   = hello.c
LIBS   =
CFLAGS = -Wall

NEWTOS_SDK ?= ../..
include $(NEWTOS_SDK)/mk/newtos.mk
