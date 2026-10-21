// snapheld: quais arquivos um snapshot MONTADO segura, em bytes físicos exatos.
// Para cada oid da lista (saída do snapdiff, tipos 2 e 3), acha o arquivo no snapshot pelo
// fsgetpath(fsid_do_snapshot, oid), mapeia os blocos físicos (F_LOG2PHYS_EXT) e desconta os
// blocos do mesmo oid nas montagens de comparação (volume vivo e/ou snapshots vizinhos).
// Blocos restantes = presos por este snapshot. União global deduplica clones dentro do snapshot.
//   Contra o vivo:            snapheld <snap_mnt> /System/Volumes/Data            < oids
//   Privado (como o fsctl):   snapheld <snap_mnt> <snap_anterior_mnt> <snap_seguinte_mnt> < oids
// Somente leitura. Precisa de root só porque montar snapshot precisa (o dono roda; ver o .md).
// Saída: TSV bytes_presos, estado, oid, caminho (stdout); total deduplicado (stderr).
#define EXTENTS_NO_MAIN
#include "extents.c"
#include <sys/mount.h>
#include <sys/fsgetpath.h>
#include <assert.h>

static ext_t *held; static int nheld, capheld;
static void keep(off_t dev, off_t len) {
  if (len <= 0) return;
  if (nheld == capheld) held = realloc(held, (capheld = capheld ? capheld * 2 : 1024) * sizeof *held);
  held[nheld++] = (ext_t){ dev, len };
}

// a - b, com a e b ordenados por dev; guarda o resto e devolve a soma.
static long long subtract(ext_t *a, int na, ext_t *b, int nb) {
  long long sum = 0; int j = 0;
  for (int i = 0; i < na; i++) {
    off_t lo = a[i].dev, hi = a[i].dev + a[i].len;
    while (j < nb && b[j].dev + b[j].len <= lo) j++;
    for (int k = j; k < nb && b[k].dev < hi && lo < hi; k++) {
      if (b[k].dev > lo) { keep(lo, b[k].dev - lo); sum += b[k].dev - lo; }
      if (b[k].dev + b[k].len > lo) lo = b[k].dev + b[k].len;
    }
    if (hi > lo) { keep(lo, hi - lo); sum += hi - lo; }
  }
  return sum;
}

static ext_t *merge_sorted(ext_t *v, int n, int *out) {    /* b pode vir de várias montagens */
  qsort(v, n, sizeof *v, cmp); int m = 0;
  for (int i = 0; i < n; i++) {
    if (m && v[i].dev <= v[m-1].dev + v[m-1].len) {
      off_t e = v[i].dev + v[i].len; if (e > v[m-1].dev + v[m-1].len) v[m-1].len = e - v[m-1].dev;
    } else v[m++] = v[i];
  }
  *out = m; return v;
}

static int path_in(const char *mnt, fsid_t *fsid, uint64_t oid, char *out, size_t sz) {
  char rel[4096]; ssize_t n = fsgetpath(rel, sizeof rel, fsid, oid);
  if (n <= 0) return -1;
  if (!strncmp(rel, mnt, strlen(mnt))) snprintf(out, sz, "%s", rel);   /* já vem com o ponto de montagem */
  else snprintf(out, sz, "%s%s", mnt, rel);
  return 0;
}

static int selftest(void) {                                          /* snapheld -t */
  ext_t a[] = {{0,100},{200,100},{400,50}}, b[] = {{50,10},{90,150},{420,100}};
  assert(subtract(a, 3, b, 3) == 50 + 30 + 60 + 20);                 /* [0,50)+[60,90) [240,300) [400,420) */
  ext_t c[] = {{10,5},{12,10},{40,1}}; int m; merge_sorted(c, 3, &m);
  assert(m == 2 && c[0].len == 12);
  nheld = 0; puts("selftest ok"); return 0;
}

int main(int argc, char **argv) {
  if (argc == 2 && !strcmp(argv[1], "-t")) return selftest();
  if (argc < 3) { fprintf(stderr, "uso: %s <snap_mnt> <cmp_mnt> [cmp_mnt2] < saida_do_snapdiff\n", argv[0]); return 2; }
  int ncmp = argc - 2; struct statfs sf[3];
  for (int i = 0; i < ncmp + 1; i++) if (statfs(argv[1 + i], &sf[i])) { perror(argv[1 + i]); return 1; }
  if (!memcmp(&sf[0].f_fsid, &sf[1].f_fsid, sizeof(fsid_t))) fprintf(stderr, "aviso: snapshot e comparação têm o mesmo fsid (teste de encanamento: resultado deve ser 0)\n");
  char line[8192], p[4096], q[4096]; long long total = 0, estimated = 0; unsigned long long seen = 0, miss = 0;
  while (fgets(line, sizeof line, stdin)) {
    unsigned t; unsigned long long oid;
    if (sscanf(line, "%u %llu", &t, &oid) != 2 || t == 1) continue;   /* tipo 1 = criado depois: não está no snapshot */
    seen++;
    if (path_in(argv[1], &sf[0].f_fsid, oid, p, sizeof p)) { miss++; continue; }
    struct stat st; if (lstat(p, &st) || !S_ISREG(st.st_mode)) continue;
    int na; ext_t *a = map(p, &na, 0);
    if (!a) {                                                        /* comprimido ou ilegível: alocado inteiro, marcado */
      printf("%lld\testimado_%s\t%llu\t%s\n", (long long)st.st_blocks * 512, errno == ENOTSUP ? "comprimido" : "erro", oid, p);
      estimated += (long long)st.st_blocks * 512; continue;
    }
    ext_t *b = NULL; int nb = 0; const char *state = "apagado";
    for (int i = 0; i < ncmp; i++) {
      if (path_in(argv[2 + i], &sf[1 + i].f_fsid, oid, q, sizeof q)) continue;
      int nc; ext_t *c = map(q, &nc, 0); if (!c) continue;
      b = realloc(b, (nb + nc) * sizeof *b); memcpy(b + nb, c, nc * sizeof *c); nb += nc; free(c);
      state = "modificado";
    }
    if (nb) b = merge_sorted(b, nb, &nb);
    long long h = subtract(a, na, b, nb);
    if (h) printf("%lld\t%s\t%llu\t%s\n", h, state, oid, p);
    free(a); free(b);
  }
  int m; merge_sorted(held, nheld, &m);
  for (int i = 0; i < m; i++) total += held[i].len;
  fprintf(stderr, "oids=%llu sem_caminho_no_snapshot=%llu presos_exatos_dedup=%lld estimados_comprimidos=%lld\n", seen, miss, total, estimated);
  return 0;
}
