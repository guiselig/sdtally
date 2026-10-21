// dirstats: lê as estatísticas de pasta do APFS (dir-stats) SEM abrir a pasta. SOMENTE LEITURA.
// fsctl(path, 0xC1104A71, buf[0x110]) com buf zerado = "get" (é o que `apfs.util -S` faz,
// desmontado em __text 0x100000d80). O campo em buf+4 = 1 seria "set maintain-dir-stats":
// este programa NUNCA escreve nesse campo.
// Saída: descendentes, bytes físicos, gen-count, caminho. uso: ./dirstats <pasta>...
#include <unistd.h>
int fsctl(const char *, unsigned long, void *, unsigned int);
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <errno.h>
#define APFS_DIRSTATS 0xC1104A71u
int main(int c, char **v) {
  int rc = 0;
  for (int i = 1; i < c; i++) {
    uint8_t b[0x110]; memset(b, 0, sizeof b);        // op = 0 (get)
    if (fsctl(v[i], APFS_DIRSTATS, b, 0)) { fprintf(stderr, "%s: %s\n", v[i], strerror(errno)); rc = 1; continue; }
    uint64_t gen, desc, phys;
    memcpy(&gen, b + 0x30, 8); memcpy(&desc, b + 0x38, 8); memcpy(&phys, b + 0x40, 8);
    printf("%llu\t%llu\t%llu\t%s\n", desc, phys, gen, v[i]);
  }
  return rc;
}
