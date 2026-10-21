// stmsize: chama STMSizeDirectoryWithExcludedChildURLs(NSURL*, NSArray* excluídos) do
// StorageManagement.framework (privado) — a MESMA função que o Storage.appex usa para a linha
// "Apple Intelligence" (GenerativeModels + Visual). Só medição; a função só dimensiona.
// Assinatura recuperada da desmontagem: x0 = NSURL, x1 = nil (excluídos) → uint64 bytes.
#import <Foundation/Foundation.h>
#include <dlfcn.h>
typedef unsigned long long (*SizeFn)(NSURL *, NSArray *);
int main(int argc, char **argv) {
  @autoreleasepool {
    void *h = dlopen("/System/Library/PrivateFrameworks/StorageManagement.framework/StorageManagement", RTLD_LAZY);
    SizeFn f = h ? (SizeFn)dlsym(h, "STMSizeDirectoryWithExcludedChildURLs") : NULL;
    if (!f) { fprintf(stderr, "símbolo ausente: %s\n", dlerror()); return 1; }
    for (int i = 1; i < argc; i++) {
      NSDate *t = [NSDate date];
      unsigned long long b = f([NSURL fileURLWithPath:@(argv[i])], nil);
      printf("%llu\t%.2fs\t%s\n", b, -[t timeIntervalSinceNow], argv[i]);
    }
  }
  return 0;
}
