// itlib: lê a biblioteca do Music/TV pela API pública iTunesLibrary.framework (ITLibrary) e
// soma fileSize dos itens com arquivo local, separando por mediaKind. Só leitura.
// Serve para reproduzir "Music" e "TV" do Ajustes sem o AMPLibraryAgent, que exige o
// entitlement restrito com.apple.amp.library.client (AMFI mata binário ad-hoc que o declara).
// uso: ./itlib [-v]
#import <Foundation/Foundation.h>
#import <iTunesLibrary/iTunesLibrary.h>
#include <sys/resource.h>
#include <sys/stat.h>

int main(int argc, char **argv) {
  setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF);
  setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES, IOPOL_SCOPE_PROCESS, IOPOL_ATIME_UPDATES_OFF);
  @autoreleasepool {
    BOOL verbose = argc > 1 && !strcmp(argv[1], "-v");
    NSError *e = nil;
    ITLibrary *lib = [ITLibrary libraryWithAPIVersion:@"1.1" options:ITLibInitOptionNone error:&e];
    if (!lib) { fprintf(stderr, "ITLibrary: %s\n", e.description.UTF8String); return 1; }
    printf("biblioteca: %s  itens: %lu\n", lib.musicFolderLocation.path.UTF8String, (unsigned long)lib.allMediaItems.count);
    // Por mediaKind: Σ fileSize (valor gravado na biblioteca), Σ st_size e Σ alocado do arquivo.
    NSMutableDictionary<NSNumber *, NSMutableArray *> *acc = [NSMutableDictionary dictionary];
    for (ITLibMediaItem *it in lib.allMediaItems) {
      NSNumber *k = @(it.mediaKind);
      NSMutableArray *a = acc[k] ?: (acc[k] = [@[@0, @0, @0, @0, @0] mutableCopy]);
      a[0] = @([a[0] unsignedLongLongValue] + 1);
      if (!it.location || it.locationType != ITLibMediaItemLocationTypeFile) continue;
      struct stat st;
      if (lstat(it.location.fileSystemRepresentation, &st)) continue;   // sem arquivo local
      a[1] = @([a[1] unsignedLongLongValue] + 1);
      a[2] = @([a[2] unsignedLongLongValue] + it.fileSize);
      a[3] = @([a[3] unsignedLongLongValue] + (unsigned long long)st.st_size);
      a[4] = @([a[4] unsignedLongLongValue] + (unsigned long long)st.st_blocks * 512);
      if (verbose) printf("%lu\t%llu\t%lld\t%s\n", (unsigned long)it.mediaKind, it.fileSize, (long long)st.st_size, it.location.path.UTF8String);
    }
    printf("mediaKind\titens\tcom_arquivo\tΣfileSize\tΣst_size\tΣalocado\n");
    for (NSNumber *k in [acc.allKeys sortedArrayUsingSelector:@selector(compare:)]) {
      NSArray *a = acc[k];
      printf("%s\n", [NSString stringWithFormat:@"%lu\t%@\t%@\t%@\t%@\t%@", k.unsignedLongValue, a[0], a[1], a[2], a[3], a[4]].UTF8String);
    }
  }
  return 0;
}
