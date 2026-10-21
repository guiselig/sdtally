// fotos_pl: reproduz "Photos" do Ajustes sem o serviço com.apple.CloudPhotosConfiguration (que
// exige entitlement restrito), chamando a mesma função de tamanho que ele usa, direto:
//   PhotosStorageExtension -> CPSConfigurationClient estimateLibrarySize -> serviço XPC ->
//   -[PLServerPhotoLibraryBundle impl_calculateTotalSizeWithResult:] -> para cada caminho de
//   -[PLPhotoLibraryPathManagerUBF pathsForLibraryFilesystemSizeCalculation] (= baseDirectory):
//   +[PLFileUtilities calculateTotalSizeOfFilesAtPath:p calculatePurgeable:YES allocatedSize:NO result:]
//   (PhotoLibraryServicesCore, privado). Soma NSURLFileSizeKey (lógico) de cada arquivo e, à
//   parte, dos que têm NSURLIsPurgeableKey. A extensão mostra total − purgeable.
// Só leitura (a função só lê chaves de recurso). Precisa de FDA no terminal (a biblioteca é
// protegida; sem FDA, abrir a biblioteca já travou > 2 min, síntese §4.9).
// uso: ./fotos_pl [~/Pictures/Photos Library.photoslibrary]
#import <Foundation/Foundation.h>
#include <dlfcn.h>
#include <sys/resource.h>

@interface PLFUShim : NSObject
+ (BOOL)calculateTotalSizeOfFilesAtPath:(NSString *)p calculatePurgeable:(BOOL)cp allocatedSize:(BOOL)as
                                 result:(void (^)(unsigned long long, unsigned long long, unsigned long long,
                                                  unsigned long long, NSError *))r;
@end

int main(int argc, char **argv) {
  if (setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF) ||
      setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES, IOPOL_SCOPE_PROCESS, IOPOL_ATIME_UPDATES_OFF)) { perror("setiopolicy_np"); return 1; }
  @autoreleasepool {
    NSString *lib = argc > 1 ? @(argv[1]) : [NSHomeDirectory() stringByAppendingPathComponent:@"Pictures/Photos Library.photoslibrary"];
    if (!dlopen("/System/Library/PrivateFrameworks/PhotoLibraryServicesCore.framework/PhotoLibraryServicesCore", RTLD_LAZY)) {
      fprintf(stderr, "dlopen: %s\n", dlerror()); return 1;
    }
    Class C = NSClassFromString(@"PLFileUtilities");
    for (int alloc = 0; alloc < 2; alloc++) {   // 0 = o que o Ajustes usa; 1 = alocado, para comparar
      NSDate *t = [NSDate date];
      __block unsigned long long a = 0, b = 0, c = 0, d = 0; __block NSError *err = nil;
      BOOL ok = [(id)C calculateTotalSizeOfFilesAtPath:lib calculatePurgeable:YES allocatedSize:alloc
                                                result:^(unsigned long long x1, unsigned long long x2, unsigned long long x3,
                                                         unsigned long long x4, NSError *e) { a = x1; b = x2; c = x3; d = x4; err = e; }];
      printf("%s ok=%d r1=%llu r2=%llu r3=%llu r4=%llu erro=%s %.1fs\n", alloc ? "alocado" : "lógico ", ok, a, b, c, d,
             err ? err.description.UTF8String : "-", -t.timeIntervalSinceNow);
    }
  }
  return 0;
}
