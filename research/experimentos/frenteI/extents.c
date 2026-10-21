// extents: mapa físico de um arquivo (F_LOG2PHYS_EXT) e bytes compartilhados entre dois arquivos.
// Serve para comparar a cópia de um arquivo num snapshot montado com a versão viva: blocos do
// snapshot que o vivo não referencia são os que o snapshot segura. Somente leitura, sem root.
// Uso: extents <arq>            -> lista (off_arquivo, off_dispositivo, bytes)
//      extents <antigo> <novo>  -> bytes físicos de <antigo> que não estão em <novo>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <unistd.h>
#include <sys/stat.h>

typedef struct { off_t dev, len; } ext_t;

static int cmp(const void *a, const void *b) { off_t x = ((ext_t*)a)->dev, y = ((ext_t*)b)->dev; return x < y ? -1 : x > y; }

// Devolve extents físicos ordenados por offset no dispositivo. Buracos (esparso) não aparecem.
// NULL + errno em falha (ENOTSUP = arquivo comprimido pelo decmpfs: dados no resource fork).
static ext_t *map(const char *p, int *n, int verbose) {
  int fd = open(p, O_RDONLY); if (fd < 0) return NULL;
  struct stat st; fstat(fd, &st);
  int cap = 64; ext_t *v = malloc(cap * sizeof *v); *n = 0;
  for (off_t off = 0; off < st.st_size;) {
    struct log2phys l = { .l2p_contigbytes = st.st_size - off, .l2p_devoffset = off };
    if (fcntl(fd, F_LOG2PHYS_EXT, &l) < 0) {
      if (errno == ERANGE) { off += 4096; continue; }        /* buraco: pula (ponytail: passo de 4 KiB, lento em esparso grande) */
      int e = errno; close(fd); free(v); errno = e; return NULL;
    }
    if (l.l2p_contigbytes <= 0) break;
    if (verbose) printf("%lld\t%lld\t%lld\n", (long long)off, (long long)l.l2p_devoffset, (long long)l.l2p_contigbytes);
    if (*n == cap) v = realloc(v, (cap *= 2) * sizeof *v);
    v[(*n)++] = (ext_t){ l.l2p_devoffset, l.l2p_contigbytes };
    off += l.l2p_contigbytes;
  }
  close(fd);
  qsort(v, *n, sizeof *v, cmp);
  return v;
}

#ifndef EXTENTS_NO_MAIN
static ext_t *map_or_die(const char *p, int *n, int verbose) {
  ext_t *v = map(p, n, verbose); if (!v) { fprintf(stderr, "%s: %s\n", p, strerror(errno)); exit(1); } return v;
}

int main(int argc, char **argv) {
  if (argc == 2) { int n; map_or_die(argv[1], &n, 1); return 0; }
  if (argc != 3) { fprintf(stderr, "uso: %s <arq> | <antigo> <novo>\n", argv[0]); return 2; }
  int na, nb; ext_t *a = map_or_die(argv[1], &na, 0), *b = map_or_die(argv[2], &nb, 0);
  long long total = 0, shared = 0;
  for (int i = 0; i < na; i++) {                             /* interseção de intervalos ordenados */
    total += a[i].len;
    for (int j = 0; j < nb; j++) {
      off_t lo = a[i].dev > b[j].dev ? a[i].dev : b[j].dev;
      off_t hi = a[i].dev + a[i].len < b[j].dev + b[j].len ? a[i].dev + a[i].len : b[j].dev + b[j].len;
      if (hi > lo) shared += hi - lo;
    }
  }
  printf("antigo_fisico=%lld compartilhado=%lld so_no_antigo=%lld\n", total, shared, total - shared);
  return 0;
}
#endif
