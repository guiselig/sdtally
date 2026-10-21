// Frente G: chama funções privadas do APFS.framework (só leitura) e imprime o resultado.
// Uso: apfsspace disk3s5 [disk3]
#include <stdio.h>
#include <stdint.h>
#include <dlfcn.h>
#include <CoreFoundation/CoreFoundation.h>

typedef int (*ext_fn)(const char *, CFDictionaryRef *);
typedef int (*vsi_fn)(const char *, uint64_t *, uint64_t *);

int main(int argc, char **argv) {
    void *h = dlopen("/System/Library/PrivateFrameworks/APFS.framework/APFS", RTLD_NOW);
    if (!h) { fprintf(stderr, "dlopen: %s\n", dlerror()); return 1; }
    ext_fn ext = (ext_fn)dlsym(h, "APFSExtendedSpaceInfo");
    vsi_fn vsi = (vsi_fn)dlsym(h, "APFSVolumeGetSpaceInfo");
    for (int i = 1; i < argc; i++) {
        CFDictionaryRef d = NULL;
        int r = ext(argv[i], &d);
        printf("== APFSExtendedSpaceInfo(%s) = 0x%x\n", argv[i], r);
        if (!r && d) { CFShow(d); CFRelease(d); }
        uint64_t a = 0, b = 0;
        r = vsi(argv[i], &a, &b);
        printf("== APFSVolumeGetSpaceInfo(%s) = 0x%x  a=%llu b=%llu\n", argv[i], r, a, b);
    }
    return 0;
}
