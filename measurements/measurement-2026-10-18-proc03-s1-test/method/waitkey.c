/* waitkey.c -- test scaffolding (Tester): console program that waits in Cconin; a Ctrl-C ends it directly (xterm(-32)) with no Pexec parent. */
#include <newtos.h>
int main(void)
{
    Cconws("WAITKEY: waiting in Cconin (Ctrl-C ends me)\r\n");
    Cconin();
    Cconws("WAITKEY: Cconin returned a key\r\n");
    return 0;
}
