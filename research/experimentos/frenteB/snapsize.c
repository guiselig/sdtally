// snapsize: lista snapshots APFS (fs_snapshot_list) e mede o tamanho privado de cada um
// com fsctl(path, 0xC0184A13, {from_xid, to_xid, out}) — o mesmo ioctl que
// +[TMSnapshot calculatePrivateSizeOfSnapshotsFromXID:toXID:...] (TimeMachine.framework) usa.
// Somente leitura. Sem root. uso: ./snapsize [/System/Volumes/Data]
// ATENÇÃO: no volume de destino do Time Machine com backup em curso, o processo ficou em
// estado U (kernel) por ~5 min. Não apontar para /Volumes/Backups* sem timeout.
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/attr.h>
#include <sys/snapshot.h>
int fsctl(const char *, unsigned long, void *, unsigned int); /* libc; header não vem no SDK */

#define APFSIOC_SNAP_PRIVATE_SIZE 0xC0184A13UL /* _IOWR('J', 0x13, 24 bytes); nome nosso, não oficial */
struct range { uint64_t from_xid, to_xid, out_bytes; };

static int priv(const char *mp, uint64_t a, uint64_t b, uint64_t *out) {
  struct range r = { a, b, 0 };
  if (fsctl(mp, APFSIOC_SNAP_PRIVATE_SIZE, &r, 0) != 0) return -1;
  *out = r.out_bytes; return 0;
}

int main(int argc, char **argv) {
  const char *mp = argc > 1 ? argv[1] : "/System/Volumes/Data";
  int fd = open(mp, O_RDONLY);
  if (fd < 0) { perror("open"); return 1; }
  struct attrlist al = { .bitmapcount = ATTR_BIT_MAP_COUNT,
    .commonattr = ATTR_CMN_RETURNED_ATTRS | ATTR_CMN_NAME | ATTR_CMN_OBJID | ATTR_CMN_CRTIME };
  char buf[64 * 1024];
  uint64_t xids[256]; char names[256][256]; int n = 0;
  for (;;) {
    int c = fs_snapshot_list(fd, &al, buf, sizeof buf, 0);
    if (c < 0) { fprintf(stderr, "fs_snapshot_list: %s (errno %d)\n", strerror(errno), errno); break; }
    if (c == 0) break;
    char *p = buf;
    for (int i = 0; i < c && n < 256; i++) {
      char *e = p; uint32_t len = *(uint32_t *)p; char *q = p + 4;
      attribute_set_t rs = *(attribute_set_t *)q; q += sizeof(attribute_set_t);
      attrreference_t nr = *(attrreference_t *)q; char *nm = q + nr.attr_dataoffset; q += sizeof nr;
      /* Empírico (conferido contra `diskutil apfs listSnapshots`): no snapshot APFS o
         ATTR_CMN_OBJID (fid_objno | fid_generation<<32) é o XID do snapshot. */
      uint64_t xid = 0;
      if (rs.commonattr & ATTR_CMN_OBJID) { fsobj_id_t o; memcpy(&o, q, sizeof o); xid = (uint64_t)o.fid_objno | ((uint64_t)o.fid_generation << 32); q += sizeof o; }
      snprintf(names[n], 256, "%s", nm); xids[n++] = xid;
      p = e + len;
    }
  }
  close(fd);
  uint64_t tot = 0, sum = 0;
  /* ordena por xid (mais antigo primeiro) para o cumulativo */
  for (int i = 0; i < n; i++) for (int j = i + 1; j < n; j++) if (xids[j] < xids[i]) {
    uint64_t t = xids[i]; xids[i] = xids[j]; xids[j] = t; char tn[256]; memcpy(tn, names[i], 256); memcpy(names[i], names[j], 256); memcpy(names[j], tn, 256); }
  printf("%-52s %10s %16s %16s\n", "snapshot", "xid", "privado_bytes", "cumulativo(0..xid)");
  for (int i = 0; i < n; i++) {
    uint64_t v = 0, c = 0;
    if (priv(mp, xids[i], xids[i], &v) != 0) { printf("%-52s %10llu fsctl: %s\n", names[i], xids[i], strerror(errno)); continue; }
    if (priv(mp, 0, xids[i], &c) != 0) /* como o Utilitário de Disco: faixa 0..xid */ c = (uint64_t)-1;
    printf("%-52s %10llu %16llu %16lld\n", names[i], xids[i], v, (long long)c); sum += v;
  }
  if (n) {
    uint64_t lo = xids[0], hi = xids[0];
    for (int i = 1; i < n; i++) { if (xids[i] < lo) lo = xids[i]; if (xids[i] > hi) hi = xids[i]; }
    if (priv(mp, lo, hi, &tot) == 0) printf("faixa xid %llu..%llu (todos): %llu bytes\nsoma dos privados individuais: %llu bytes\n", lo, hi, tot, sum);
    else printf("faixa: fsctl: %s\n", strerror(errno));
  }
  return 0;
}
