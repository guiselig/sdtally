// tamanhos: soma lógica (st_size) e alocada de cada pasta dada, com as regras da síntese §4:
// setiopolicy_np (dataless e atime OFF), não desce em SF_DATALESS, não cruza DEVID, repete em
// EINTR, pula raiz de enxerto (cloneid != fileid na pasta aberta, Frente L), ATTR_CMN_ERROR lido
// logo após o attribute_set. Hardlink contado uma vez (fileid). Só leitura.
// Serve para comparar o valor de uma extensão do Ajustes com o disco (lógico x alocado).
// uso: ./tamanhos [-l] [-e ext1,ext2] <pasta>...   (-e: só arquivos com essas extensões, sem ponto;
//      -l: lista cada arquivo como "L\tst_size\talocado\tcaminho")
// saída: arquivos  Σst_size  Σalocado  dataless(arq/pastas)  erros  pasta
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <unistd.h>
#include <sys/attr.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/vnode.h>

#ifndef SF_DATALESS
#define SF_DATALESS 0x40000000
#endif

typedef struct { unsigned long long files, logical, alloc, dl_files, dl_dirs, errors, grafts, otherdev; } Sum;
static unsigned long long *seen; static size_t nseen, capseen;   // fileids de hardlinks já contados
static const char *exts;                                         // filtro opcional de extensão
static int list;

static int seen_add(unsigned long long id) {
  for (size_t i = 0; i < nseen; i++) if (seen[i] == id) return 0;   // ponytail: busca linear, poucos hardlinks em pastas de mídia
  if (nseen == capseen) { capseen = capseen ? capseen * 2 : 256; seen = realloc(seen, capseen * sizeof *seen); }
  seen[nseen++] = id; return 1;
}

static int ext_ok(const char *name) {
  if (!exts) return 1;
  const char *dot = strrchr(name, '.');
  if (!dot) return 0;
  size_t n = strlen(dot + 1);
  for (const char *p = exts; *p;) {
    const char *c = strchr(p, ','); size_t l = c ? (size_t)(c - p) : strlen(p);
    if (l == n && !strncasecmp(p, dot + 1, n)) return 1;
    p += l + (c ? 1 : 0);
  }
  return 0;
}

static int open_dir(const char *p) {
  int fd;
  do fd = open(p, O_RDONLY | O_DIRECTORY | O_NOFOLLOW); while (fd < 0 && errno == EINTR);
  return fd;
}

static void walk(const char *path, dev_t dev, int depth, Sum *s) {
  int fd = open_dir(path);
  if (fd < 0) { s->errors++; return; }
  if (depth > 0) {   // raiz de enxerto: não descer (o conteúdo repete blocos da imagem)
    struct { uint32_t len; attribute_set_t rs; uint64_t id, cid; } __attribute__((packed)) r2;
    struct attrlist a2 = {ATTR_BIT_MAP_COUNT, 0, ATTR_CMN_RETURNED_ATTRS | ATTR_CMN_FILEID, 0, 0, 0, ATTR_CMNEXT_CLONEID};
    if (!fgetattrlist(fd, &a2, &r2, sizeof r2, FSOPT_ATTR_CMN_EXTENDED) && r2.cid && r2.cid != r2.id) {
      s->grafts++; close(fd); return;
    }
  }
  struct attrlist al = {ATTR_BIT_MAP_COUNT, 0,
    ATTR_CMN_RETURNED_ATTRS | ATTR_CMN_NAME | ATTR_CMN_ERROR | ATTR_CMN_DEVID | ATTR_CMN_OBJTYPE | ATTR_CMN_FLAGS | ATTR_CMN_FILEID,
    0, 0, ATTR_FILE_LINKCOUNT | ATTR_FILE_TOTALSIZE | ATTR_FILE_ALLOCSIZE, 0};
  static char buf[256 * 1024];   // reaproveitado: as subpastas só são visitadas depois do laço
  char **subs = NULL; size_t nsub = 0;
  for (;;) {
    int n = getattrlistbulk(fd, &al, buf, sizeof buf, FSOPT_NOFOLLOW);
    if (n < 0 && errno == EINTR) continue;
    if (n < 0) s->errors++;
    if (n <= 0) break;
    char *e = buf;
    for (int i = 0; i < n; i++) {
      char *p = e; uint32_t len; memcpy(&len, p, 4); e += len; p += 4;
      attribute_set_t rs; memcpy(&rs, p, sizeof rs); p += sizeof rs;
      if (rs.commonattr & ATTR_CMN_ERROR) { uint32_t er; memcpy(&er, p, 4); p += 4; if (er) { s->errors++; continue; } }
      attrreference_t nr; memcpy(&nr, p, sizeof nr); const char *name = p + nr.attr_dataoffset; p += sizeof nr;
      uint32_t d = 0, type = 0, flags = 0; uint64_t fid = 0;
      if (rs.commonattr & ATTR_CMN_DEVID) { memcpy(&d, p, 4); p += 4; }
      if (rs.commonattr & ATTR_CMN_OBJTYPE) { memcpy(&type, p, 4); p += 4; }
      if (rs.commonattr & ATTR_CMN_FLAGS) { memcpy(&flags, p, 4); p += 4; }
      if (rs.commonattr & ATTR_CMN_FILEID) { memcpy(&fid, p, 8); p += 8; }
      if (type == VDIR) {
        if ((dev_t)d != dev) { s->otherdev++; continue; }
        if (flags & SF_DATALESS) { s->dl_dirs++; continue; }
        subs = realloc(subs, (nsub + 1) * sizeof *subs);
        asprintf(&subs[nsub++], "%s/%s", path, name);
      } else if (type == VREG) {
        uint32_t links = 1; uint64_t lg = 0, al2 = 0;
        if (rs.fileattr & ATTR_FILE_LINKCOUNT) { memcpy(&links, p, 4); p += 4; }
        if (rs.fileattr & ATTR_FILE_TOTALSIZE) { memcpy(&lg, p, 8); p += 8; }
        if (rs.fileattr & ATTR_FILE_ALLOCSIZE) { memcpy(&al2, p, 8); p += 8; }
        if (!ext_ok(name)) continue;
        if (links > 1 && !seen_add(fid)) continue;
        if (flags & SF_DATALESS) s->dl_files++;
        s->files++; s->logical += lg; s->alloc += al2;
        if (list) printf("L\t%llu\t%llu\t%s/%s\n", (unsigned long long)lg, (unsigned long long)al2, path, name);
      }
    }
  }
  close(fd);
  for (size_t k = 0; k < nsub; k++) { walk(subs[k], dev, depth + 1, s); free(subs[k]); }
  free(subs);
}

int main(int argc, char **argv) {
  if (setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF) ||
      setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES, IOPOL_SCOPE_PROCESS, IOPOL_ATIME_UPDATES_OFF)) { perror("setiopolicy_np"); return 1; }
  int i = 1;
  if (argc > i && !strcmp(argv[i], "-l")) { list = 1; i++; }
  if (argc > i + 1 && !strcmp(argv[i], "-e")) { exts = argv[i + 1]; i += 2; }
  printf("arquivos\tst_size\talocado\tdataless_arq\tdataless_pastas\terros\tenxertos\toutro_dev\tpasta\n");
  for (; i < argc; i++) {
    struct stat st; Sum s = {0};
    if (lstat(argv[i], &st)) { printf("-\t-\t-\t-\t-\t%d\t-\t-\t%s\n", errno, argv[i]); continue; }
    walk(argv[i], st.st_dev, 0, &s);
    printf("%llu\t%llu\t%llu\t%llu\t%llu\t%llu\t%llu\t%llu\t%s\n", s.files, s.logical, s.alloc, s.dl_files, s.dl_dirs,
           s.errors, s.grafts, s.otherdev, argv[i]);
  }
  return 0;
}
