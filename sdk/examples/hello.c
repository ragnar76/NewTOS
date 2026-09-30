/*
 * hello.c -- the NewTOS Stage-1 SDK end-to-end proof.
 *
 * A native NewTOS console program (`.NCP`, ADR-018) compiled with clang: it
 * prints a banner through GEMDOS Cconws (0x09) and returns; crt0 then calls
 * Pterm (0x4C) with main's return value as the exit code.
 *
 * Installed as C:\HELLO2.NCP and reached only by a double-click, so it holds
 * the banner with newtos_pause() before it returns (TASK/T13).
 */
#include <newtos.h>

int main(void)
{
    Cconws("Hello from a C program built with clang!\r\n");
    newtos_pause();
    return 0;
}
