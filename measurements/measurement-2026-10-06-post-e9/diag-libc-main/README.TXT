Diagnosis (post-E9 point 2): why libc crt0 does NOT call mintlib's _main.

First attempt: crt0 -> _main (= __libc_main, mintlib/main.c:75-305), which
ends in exit(main()). Result: every write through stdout failed (printf/
fputs rc -1, errno 37 = EBADF), files and atexit were fine.

stdoutdbg.c (one source, variants by -D, all on kernel8-p1, QEMU raspi4b):
  dbg / dbg2DB / dbg3DB / dbg4DB   crt0 -> _main          : printf -1, EBADF
  dbg2DR / dbgred                  HEAD crt0 (Pterm)      : printf ok
  dbg3DM  HEAD crt0 + setvbuf(stdout,NULL,_IOLBF,0) by hand: printf ok
          (so setvbuf alone is not it)
  dbg4DN  crt0 -> _main, then stdout back to binary mode : still EBADF
  every variant: write(1)/Fwrite(1)/__stdio_text_write ok (the fd is fine,
          the stream is not)
  dbg5    final crt0 -> exit(main())                     : printf ok

Cause (source reading): __libc_main's init_stream_mode() (main.c:41-60)
replaces stdout's io functions wholesale (__set_binmode, stdio/binmode.c:
34-43) AFTER the stream was initialised -- which for a tty had set the seek
function to NULL (stdio/stdio_init.c:58-70). With a seek function again,
the first flush asks for the position (internals.c:100-131
__stdio_check_offset -> __stdio_seek -> lseek(1, 0, SEEK_CUR) -> Fseek).
NewTOS Fseek on a handle that resolves to the console returns EIHNDL (-37
-> EBADF; tos-api/src/bdos/fs/file.rs fseek: ofd_of() -> None). EmuTOS
answers 0 for every handle function except Fread/Fwrite when the handle is
a character device (bdos/bdosmain.c:571-641: "only do things on read and
write ... return 0"). So mintlib's own start-up runs on EmuTOS and fails on
NewTOS because of that GEMDOS deviation -- not fixed here (out of scope),
reported. The shipped crt0 calls exit(main()) only.
