/*
 * acctprobe.c -- test helper of measurement-2026-10-07-acctitle (TASK/ACCTITLE),
 * not product code.
 *
 * What WinDom's menu_exec_cmd (sdk/windom/windom-1.21.3/src/menu_execcmd.c)
 * does on every unhandled key: walk the installed menu tree from the Desk
 * drop-down on and strstr() every entry's ob_spec string. The AES writes the
 * registered desk accessories' titles into that drop-down (menu_fixup,
 * EmuTOS aes/gemmnlib.c:89-125), so the program reads AES-owned strings.
 *
 * The probe builds a conforming menu bar in its own memory (Desk box with
 * About, separator and 2 + NUM_ACCS reserved items, gemmnlib.c:110-118),
 * installs it with menu_bar(tree, 1), prints every Desk entry's ob_spec and
 * then READS each string from EL0 (the access zView faulted on), prints it,
 * removes the bar and ends. Output goes to the console (serial).
 *
 * Expected on a correct AES: the accessory entry (SYSACC.NAC, " NewTOS
 * System") is readable and printed. On HEAD 3895575 the entry's ob_spec is a
 * kernel high-half address and the read is a memory violation.
 */
#include <stdio.h>
#include <string.h>
#include <mint/osbind.h>
#include <gem.h>

#define NREC 17

static OBJECT tree[NREC];
static char t_desk[] = " Desk ";
static char t_file[] = " File ";
static char s_about[] = "  About acctprobe ";
static char s_sep[] = "------------------";
static char s_resv[] = "  reserved        ";
static char s_quit[] = "  Quit       ^Q ";

static void say(const char *s) { Cconws(s); }

static void ob(int i, int next, int head, int tail, int type, int flags, void *spec,
               int x, int y, int w, int h)
{
	tree[i].ob_next = next;
	tree[i].ob_head = head;
	tree[i].ob_tail = tail;
	tree[i].ob_type = type;
	tree[i].ob_flags = flags;
	tree[i].ob_state = 0;
	tree[i].ob_spec.free_string = spec;
	tree[i].ob_x = x;
	tree[i].ob_y = y;
	tree[i].ob_width = w;
	tree[i].ob_height = h;
}

int main(void)
{
	char buf[256];
	int i, n;
	short wc, hc, wb, hb;
	short id;

	id = appl_init();
	(void)graf_handle(&wc, &hc, &wb, &hb);
	/* let the accessories finish their own start (menu_register) */
	evnt_timer(500L);

	/* 0 ROOT, 1 THEBAR, 2 THEACTIVE, 3/4 titles, 5 the menus, 6 Desk box
	 * (7 About, 8 separator, 9..14 reserved), 15 File box (16 Quit). */
	ob(0, -1, 1, 5, G_IBOX, 0, 0, 0, 0, 640, 480);
	ob(1, 5, 2, 2, G_BOX, 0, (void *)0x1100L, 0, 0, 640, hc + 3);
	ob(2, 1, 3, 4, G_IBOX, 0, 0, 16, 0, 12 * wc, hc + 3);
	ob(3, 4, -1, -1, G_TITLE, 0, t_desk, 0, 0, 6 * wc, hc + 3);
	ob(4, 2, -1, -1, G_TITLE, 0, t_file, 6 * wc, 0, 6 * wc, hc + 3);
	ob(5, 0, 6, 15, G_IBOX, 0, 0, 0, hc + 3, 640, 9 * hc);
	ob(6, 15, 7, 14, G_BOX, 0, (void *)0xFF1100L, 16, 0, 20 * wc, 8 * hc);
	ob(7, 8, -1, -1, G_STRING, 0, s_about, 0, 0, 20 * wc, hc);
	ob(8, 9, -1, -1, G_STRING, 0, s_sep, 0, hc, 20 * wc, hc);
	tree[8].ob_state = OS_DISABLED;
	for (i = 9; i <= 14; i++)
		ob(i, i == 14 ? 6 : i + 1, -1, -1, G_STRING, 0, s_resv, 0, (i - 7) * hc, 20 * wc, hc);
	ob(15, 5, 16, 16, G_BOX, 0, (void *)0xFF1100L, 16 + 6 * wc, 0, 16 * wc, hc);
	ob(16, 15, -1, -1, G_STRING, OF_LASTOB, s_quit, 0, 0, 16 * wc, hc);

	sprintf(buf, "ACCTPROBE: appl_init=%d sizeof(OBJECT)=%d tree=%p\r\n", id, (int)sizeof(OBJECT), (void *)tree);
	say(buf);
	n = menu_bar(tree, 1);
	sprintf(buf, "ACCTPROBE: menu_bar(tree,1)=%d desk box head=%d tail=%d\r\n", n, tree[6].ob_head, tree[6].ob_tail);
	say(buf);

	/* print every linked Desk entry's ob_spec first ... */
	for (i = tree[6].ob_head; i != 6 && i >= 0 && i < NREC; i = tree[i].ob_next) {
		sprintf(buf, "ACCTPROBE: desk entry %d type=%d ob_spec=%p\r\n", i, tree[i].ob_type, (void *)tree[i].ob_spec.free_string);
		say(buf);
	}
	/* ... then read each string from EL0, as menu_exec_cmd's strstr does */
	for (i = tree[6].ob_head; i != 6 && i >= 0 && i < NREC; i = tree[i].ob_next) {
		const char *s = tree[i].ob_spec.free_string;
		sprintf(buf, "ACCTPROBE: read entry %d ...\r\n", i);
		say(buf);
		sprintf(buf, "ACCTPROBE: entry %d len=%d [%s] strstr(\" ^Q \")=%p\r\n", i, (int)strlen(s), s, (void *)strstr(s, " ^Q "));
		say(buf);
	}
	menu_bar(tree, 0);
	say("ACCTPROBE: done\r\n");
	appl_exit();
	return 0;
}
