// Frente G: chama o AppleAPFSUserClient direto, só com os seletores de LEITURA que o
// APFS.framework usa em funções Get*: 6 (APFSContainerGetSpaceInfo, saída 0x4c8 B)
// 7 (APFSVolumeGetSpaceInfo, entrada = índice do volume, saída 16 B: tamanho do container, fs_used)
// e 0x1e (APFSStatisticsProcessContainer / apfs_stats, entrada = índice, saída 0x110 B:
// contadores do superbloco do volume e nós das árvores omap/fsroot/extentref).
// Offsets conferidos contra a saída do apfs_stats nos volumes do disk1. RAW=1 despeja tudo.
// Uso: apfsuc disk3 [indice_volume ...]   (disk3s5 -> índice 4)
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <IOKit/IOKitLib.h>

int main(int argc, char **argv) {
    if (argc < 2) return 2;
    io_service_t media = IOServiceGetMatchingService(kIOMainPortDefault, IOBSDNameMatching(kIOMainPortDefault, 0, argv[1]));
    io_registry_entry_t cont = 0;
    // acha o AppleAPFSContainer cujo volume filho tem BSD Name "<argv[1]>s..."
    (void)media; io_iterator_t it; char want[64]; snprintf(want, sizeof want, "%ss", argv[1]);
    IOServiceGetMatchingServices(kIOMainPortDefault, IOServiceMatching("AppleAPFSContainer"), &it);
    for (io_service_t k; !cont && (k = IOIteratorNext(it)); ) {
        io_iterator_t ch; IORegistryEntryGetChildIterator(k, kIOServicePlane, &ch);
        for (io_registry_entry_t v; (v = IOIteratorNext(ch)); IOObjectRelease(v)) {
            CFStringRef bsd = IORegistryEntryCreateCFProperty(v, CFSTR("BSD Name"), NULL, 0); char b[64] = "";
            if (bsd) { CFStringGetCString(bsd, b, sizeof b, kCFStringEncodingUTF8); CFRelease(bsd); }
            if (!strncmp(b, want, strlen(want))) { cont = k; break; }
        }
    }
    if (!cont) { fprintf(stderr, "sem container\n"); return 1; }
    io_connect_t c;
    kern_return_t kr = IOServiceOpen(cont, mach_task_self(), 0, &c);
    printf("IOServiceOpen = 0x%x\n", kr); if (kr) return 1;
    uint8_t out[0x4c8]; size_t n = sizeof out; memset(out, 0, n);
    kr = IOConnectCallStructMethod(c, 6, NULL, 0, out, &n);
    printf("sel 6 = 0x%x, %zu bytes\n", kr, n);
    uint64_t *q = (uint64_t *)out;
    for (size_t i = 0; i < n / 8; i++) if (q[i]) printf("  [0x%03zx] %llu (0x%llx)\n", i * 8, q[i], q[i]);
    for (int a = 2; a < argc; a++) {
        uint32_t idx = (uint32_t)atoi(argv[a]); uint64_t o[2] = {0}; size_t on = sizeof o;
        kr = IOConnectCallStructMethod(c, 7, &idx, 4, o, &on);
        printf("sel 7 idx %u = 0x%x: %llu %llu\n", idx, kr, o[0], o[1]);
        // sel 0x1e: o que APFSStatisticsProcessContainer / apfs_stats usa por volume
        uint8_t st[0x110]; memset(st, 0, sizeof st); memcpy(st, &idx, 4); size_t sn = sizeof st;
        kr = IOConnectCallStructMethod(c, 0x1e, &idx, 4, st, &sn);
        printf("sel 0x1e idx %u = 0x%x, %zu bytes\n", idx, kr, sn);
        uint64_t *w = (uint64_t *)st;
        static const struct { int off; const char *nome; } F[] = {
            {0x08,"arquivos"},{0x10,"pastas"},{0x18,"snapshots"},{0x20,"nos omap"},{0x30,"nos fsroot"},
            {0x28,"nos extentref"},{0x38,"extents"},{0x40,"arquivos purgeable"},{0xa8,"bytes purgeable"}};
        for (size_t i = 0; !kr && i < sizeof F / sizeof F[0]; i++) printf("  %-20s %llu\n", F[i].nome, w[F[i].off / 8]);
        if (getenv("RAW")) for (size_t i = 0; !kr && i < sn / 8; i++) if (w[i]) printf("  [0x%03zx] %llu\n", i * 8, w[i]);
    }
    IOServiceClose(c);
    return 0;
}
