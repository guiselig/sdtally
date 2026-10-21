// snapdiff: lista objetos (inodes) que mudaram entre dois snapshots APFS do mesmo volume,
// via o mesmo fsctl que o backupd usa em -[TMSnapshotDiffer enumerateChangesBetweenSnapshots:error:].
// Somente leitura. Sem root (testado como uid 501 com FDA herdado). NUNCA apontar para o destino do TM.
// Uso: snapdiff <mount> <xid_antigo> <xid_novo> [-p]   (-p: resolve caminho no volume vivo com fsgetpath)
// Saída: tipo(1..3) oid [caminho|-errno]; no stderr, contagem por tipo.
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <errno.h>
#include <sys/mount.h>
#include <sys/fsgetpath.h>

int fsctl(const char *, unsigned long, void *, unsigned int);
#define APFSIOC_SNAP_DIFF 0xC0304A10UL   /* _IOWR('J', 0x10, 48); nome nosso, o oficial não é público */
#define CAP 1000                          /* mesmo lote do backupd */

struct snap_diff_args {                   /* layout lido da desmontagem do backupd (sp+0x20) */
  uint64_t earlier_xid, later_xid;
  uint32_t capacity, count;               /* in: capacidade; out: preenchidos */
  uint64_t *oids;                         /* CAP x u64 */
  uint8_t *types;                         /* CAP x u8; backupd só aceita 1..3 */
  uint64_t cursor;                        /* 0 no início; o kernel avança */
};
_Static_assert(sizeof(struct snap_diff_args) == 48, "layout");

int main(int argc, char **argv) {
  if (argc < 4) { fprintf(stderr, "uso: %s <mount> <xid_antigo> <xid_novo> [-p]\n", argv[0]); return 2; }
  int resolve = argc > 4 && !strcmp(argv[4], "-p");
  struct statfs sf; if (statfs(argv[1], &sf)) { perror("statfs"); return 1; }
  static uint64_t oids[CAP]; static uint8_t types[CAP];
  struct snap_diff_args a = { strtoull(argv[2], 0, 10), strtoull(argv[3], 0, 10), 0, 0, 0, 0, 0 };
  uint64_t per[256] = {0}, total = 0, calls = 0;
  char path[4096];
  do {
    memset(oids, 0, sizeof oids); memset(types, 0, sizeof types);
    a.capacity = CAP; a.count = 0; a.oids = oids; a.types = types;
    if (fsctl(argv[1], APFSIOC_SNAP_DIFF, &a, 0) < 0) { fprintf(stderr, "fsctl: %s (errno %d)\n", strerror(errno), errno); return 1; }
    calls++;
    for (uint32_t i = 0; i < a.count; i++) {
      per[types[i]]++; total++;
      printf("%u %llu", types[i], (unsigned long long)oids[i]);
      if (resolve) {
        ssize_t n = fsgetpath(path, sizeof path, &sf.f_fsid, oids[i]);
        if (n > 0) printf(" %s", path); else printf(" -%d", errno);
      }
      putchar('\n');
    }
  } while (a.count == CAP);
  fprintf(stderr, "chamadas=%llu objetos=%llu", (unsigned long long)calls, (unsigned long long)total);
  for (int t = 0; t < 256; t++) if (per[t]) fprintf(stderr, " tipo%d=%llu", t, (unsigned long long)per[t]);
  fputc('\n', stderr);
  return 0;
}
