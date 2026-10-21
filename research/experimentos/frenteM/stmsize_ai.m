// stmsize_ai: chama STMSizeDirectoryWithExcludedChildURLs (StorageManagement.framework, privado),
// a função que o Storage.appex usa para "Apple Intelligence", e ao lado lê dir-stats do APFS
// (fsctl 0xC1104A71, buffer ZERADO: buf+4 != 0 escreve no volume, nunca fazer).
// Só leitura. Antes de qualquer travessia: não materializar dataless e não atualizar atime.
// uso: ./stmsize_ai <pasta>...   saída: stm_bytes  dirstats_bytes  segundos  pasta
#import <Foundation/Foundation.h>
#include <dlfcn.h>
#include <sys/resource.h>
#include <string.h>
int fsctl(const char *, unsigned long, void *, unsigned int);
typedef unsigned long long (*SizeFn)(NSURL *, NSArray *);
static long long dirstats(const char *p) {
  unsigned char b[0x110]; memset(b, 0, sizeof b);   // get; nunca mexer em b[4]
  if (fsctl(p, 0xC1104A71u, b, 0)) return -1;
  unsigned long long v; memcpy(&v, b + 0x40, 8); return (long long)v;
}
int main(int argc, char **argv) {
  if (setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF) ||
      setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES, IOPOL_SCOPE_PROCESS, IOPOL_ATIME_UPDATES_OFF)) { perror("setiopolicy_np"); return 1; }
  @autoreleasepool {
    void *h = dlopen("/System/Library/PrivateFrameworks/StorageManagement.framework/StorageManagement", RTLD_LAZY);
    SizeFn f = h ? (SizeFn)dlsym(h, "STMSizeDirectoryWithExcludedChildURLs") : NULL;
    if (!f) { fprintf(stderr, "símbolo ausente\n"); return 1; }
    for (int i = 1; i < argc; i++) {
      NSDate *t = [NSDate date];
      unsigned long long b = f([NSURL fileURLWithPath:@(argv[i])], nil);
      printf("%llu\t%lld\t%.2fs\t%s\n", b, dirstats(argv[i]), -[t timeIntervalSinceNow], argv[i]);
    }
  }
  return 0;
}
