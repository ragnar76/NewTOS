/* tmpfile-link.c -- does mintlib tmpfile() link with the NewTOS SDK? (TASK/PROC04 point 5, tmpfile pattern check) */
#include <stdio.h>
int main(void) { FILE *fp = tmpfile(); return fp ? 0 : 1; }
