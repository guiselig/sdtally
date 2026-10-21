// Lê FSIOC_GET_GRAFT_INFO (xnu bsd/sys/fsctl.h): só consulta, não altera nada.
// Uso: graftinfo <caminho>...  Para cada caminho: nº de grafts do volume, registros, e se o inode está num graft.
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <errno.h>
#include <sys/ioctl.h>
int fsctl(const char *, unsigned long, void *, unsigned int);
typedef struct {
  uint64_t gi_graft_file, gi_graft_dir, gi_inum_base;
  uint64_t gi_inum_len:56, gi_graft_lut_reduction:8;
  uint64_t gi_graft_flags;
} fsioc_graft_info_t;
typedef struct {
  uint16_t ggi_count, ggi_graft_index; uint8_t ggi_is_in_graft; uint8_t pad[3]; void *ggi_buffer;
} fsioc_get_graft_info_t;
#define FSIOC_GET_GRAFT_INFO _IOWR('J', 102, fsioc_get_graft_info_t)
int main(int c, char **v) {
  for (int i = 1; i < c; i++) {
    fsioc_get_graft_info_t g = {0};
    if (fsctl(v[i], FSIOC_GET_GRAFT_INFO, &g, 0)) { printf("%s: count errno %d %s\n", v[i], errno, strerror(errno)); continue; }
    unsigned n = g.ggi_count; fsioc_graft_info_t r[256]; memset(r, 0, sizeof r);
    fsioc_get_graft_info_t h = {0}; h.ggi_count = n > 256 ? 256 : n; h.ggi_buffer = r;
    if (fsctl(v[i], FSIOC_GET_GRAFT_INFO, &h, 0)) { printf("%s: list errno %d %s\n", v[i], errno, strerror(errno)); continue; }
    printf("%s: grafts=%u escritos=%u in_graft=%u index=%u\n", v[i], n, h.ggi_count, h.ggi_is_in_graft, h.ggi_graft_index);
    if (i == 1) for (unsigned k = 0; k < h.ggi_count; k++)
      printf("  [%u] file=%llu dir=%llu inum_base=%llu inum_len=%llu lut_red=%u flags=0x%llx\n", k + 1,
        r[k].gi_graft_file, r[k].gi_graft_dir, r[k].gi_inum_base, (unsigned long long)r[k].gi_inum_len, (unsigned)r[k].gi_graft_lut_reduction, r[k].gi_graft_flags);
  }
}
