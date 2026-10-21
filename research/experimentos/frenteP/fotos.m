// fotos: reproduz "Photos" do Ajustes > Armazenamento chamando o mesmo pedido de leitura que
// PhotosStorageExtension faz (CloudPhotoServicesConfiguration.framework, privado).
// Lido no binário (-[PhotosConfigurationManager _estimateLibrarySize], 0x100001a50):
//   [[CPSConfigurationClient configurationClient] estimateLibrarySizeWithOptions:nil progress:NULL
//     completionHandler:^(long long ok, NSDictionary *info, NSError *e)]
//   se ok == 1: total = TotalBytes − (Purgeable se TotalBytes > Purgeable, senão 0)
//   se a biblioteca do sistema (CPSSystemPhotoLibraryPathKey) não está no volume da home: 0.
// Só leitura: nada de requestCloudPhotoServiceStateChange nem ação de resumo.
// uso: ./fotos
#import <Foundation/Foundation.h>
#include <dlfcn.h>
#include <sys/resource.h>

@interface CPSShim : NSObject
+ (id)configurationClient;
- (void)startWithCompletionHandler:(void (^)(BOOL, NSError *))h;
- (void)estimateLibrarySizeWithOptions:(id)o progress:(id *)p completionHandler:(void (^)(long long, NSDictionary *, NSError *))h;
- (id)cachedStatusInformation;
@end

static NSString *sym(void *h, const char *n) { void **p = dlsym(h, n); return p ? (__bridge NSString *)*p : nil; }

int main(void) {
  setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF);
  setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES, IOPOL_SCOPE_PROCESS, IOPOL_ATIME_UPDATES_OFF);
  @autoreleasepool {
    void *h = dlopen("/System/Library/PrivateFrameworks/CloudPhotoServicesConfiguration.framework/CloudPhotoServicesConfiguration", RTLD_LAZY);
    if (!h) { fprintf(stderr, "dlopen: %s\n", dlerror()); return 1; }
    NSString *kTotal = sym(h, "CPSPhotoLibrarySizeInformationTotalBytesKey");
    NSString *kPurg = sym(h, "CPSPhotoLibrarySizeInformationPurgeableBytesKey");
    NSString *kCur = sym(h, "CPSPhotoLibrarySizeInformationCurrentBytesKey");
    NSString *kPath = sym(h, "CPSSystemPhotoLibraryPathKey");
    CPSShim *c = [NSClassFromString(@"CPSConfigurationClient") configurationClient];
    dispatch_semaphore_t s = dispatch_semaphore_create(0);
    [c startWithCompletionHandler:^(BOOL ok, NSError *e) {
      fprintf(stderr, "start ok=%d %s\n", ok, e ? e.description.UTF8String : "");
      dispatch_semaphore_signal(s);
    }];
    if (dispatch_semaphore_wait(s, dispatch_time(DISPATCH_TIME_NOW, 60 * NSEC_PER_SEC))) { fprintf(stderr, "start: timeout\n"); return 1; }
    NSDictionary *st = [c cachedStatusInformation];
    NSString *libPath = st[kPath];
    printf("biblioteca do sistema: %s\n", libPath.UTF8String ?: "(nenhuma)");
    NSDate *t0 = [NSDate date];
    __block long long ok = 0; __block NSDictionary *info = nil; __block NSError *err = nil;
    [c estimateLibrarySizeWithOptions:nil progress:NULL completionHandler:^(long long r, NSDictionary *i, NSError *e) {
      ok = r; info = i; err = e; dispatch_semaphore_signal(s);
    }];
    if (dispatch_semaphore_wait(s, dispatch_time(DISPATCH_TIME_NOW, 300 * NSEC_PER_SEC))) { fprintf(stderr, "estimate: timeout\n"); return 1; }
    printf("resultado=%lld em %.2fs erro=%s\ninfo=%s\n", ok, -t0.timeIntervalSinceNow, err ? err.description.UTF8String : "-",
           info.description.UTF8String);
    if (ok != 1) return 1;
    long long tot = [info[kTotal] longLongValue], pur = [info[kPurg] longLongValue];
    long long v = tot - (tot > pur ? pur : 0);
    // Mesma regra de volume da extensão: NSURLVolumeURLKey da biblioteca x da home.
    if (libPath.length) {
      NSURL *a = nil, *b = nil;
      [[NSURL fileURLWithPath:libPath isDirectory:YES] getResourceValue:&a forKey:NSURLVolumeURLKey error:nil];
      [[NSURL fileURLWithPath:NSHomeDirectory() isDirectory:YES] getResourceValue:&b forKey:NSURLVolumeURLKey error:nil];
      if (![a isEqual:b]) { printf("biblioteca fora do volume da home: Photos = 0\n"); v = 0; }
    }
    printf("Photos (como o Ajustes) = %lld  (TotalBytes %lld, PurgeableBytes %lld, CurrentBytes %lld)\n",
           v, tot, pur, [info[kCur] longLongValue]);
  }
  return 0;
}
