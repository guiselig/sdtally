// musica_tv: reproduz "Music" e "TV" do Ajustes > Armazenamento chamando o mesmo pedido de
// leitura que MusicStorageExtension/TVStorageExtension fazem ao AMPLibraryAgent
// (AMPLibrary.framework, privado). Só leitura: nenhum método de remoção é chamado.
// Parâmetros lidos no binário das extensões (ITStorageUIManager):
//   Music: initWithClientInfo:@"extension" withMediaDomains:1, storageInfo domains 1, kinds 0x60
//   TV:    initWithClientInfo:@"extension" withMediaDomains:2, storageInfo domains 2, kinds 0x142
//   ignorePurgeable = pref automaticallyDeleteVideoAssetsAfterWatching (com.apple.Music / com.apple.TV)
// uso: ./musica_tv [music|tv] [-v]    saída: totalBytes purgeableBytes itens Σ FileSize
#import <Foundation/Foundation.h>
#include <dlfcn.h>
#include <sys/resource.h>

@interface AMPLFrameworkClientShim : NSObject
- (id)initWithClientInfo:(id)info withMediaDomains:(unsigned long long)d error:(id *)e;
- (BOOL)synchronousOpenMediaDomains:(id *)err withResponseDict:(id *)resp;
- (void)synchronousStorageInfoForDomains:(unsigned long long)d withMediaKinds:(unsigned long long)k
                         ignorePurgeable:(BOOL)ip withReply:(void (^)(id, id))r;
- (id)libraryFilePath;
@end

static BOOL pref(NSString *dom) {
  // Mesma leitura que a função 0x100004d08 da extensão: CFPreferencesCopyValue, usuário atual.
  id v = CFBridgingRelease(CFPreferencesCopyValue(CFSTR("automaticallyDeleteVideoAssetsAfterWatching"),
      (__bridge CFStringRef)dom, kCFPreferencesCurrentUser, kCFPreferencesAnyHost));
  return [v respondsToSelector:@selector(boolValue)] && [v boolValue];
}

int main(int argc, char **argv) {
  // Nada aqui atravessa disco, mas a regra da síntese §4 vale para todo programa do projeto.
  setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF);
  setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES, IOPOL_SCOPE_PROCESS, IOPOL_ATIME_UPDATES_OFF);
  @autoreleasepool {
    BOOL tv = argc > 1 && !strcmp(argv[1], "tv");
    BOOL verbose = argc > 2 && !strcmp(argv[2], "-v");
    unsigned long long dom = tv ? 2 : 1, kinds = tv ? 0x142 : 0x60;
    BOOL ip = pref(tv ? @"com.apple.TV" : @"com.apple.Music");
    if (!dlopen("/System/Library/PrivateFrameworks/AMPLibrary.framework/AMPLibrary", RTLD_LAZY)) {
      fprintf(stderr, "dlopen: %s\n", dlerror()); return 1;
    }
    Class C = NSClassFromString(@"AMPLFrameworkClient");
    if (!C) { fprintf(stderr, "AMPLFrameworkClient ausente\n"); return 1; }
    NSError *e = nil;
    AMPLFrameworkClientShim *c = [(AMPLFrameworkClientShim *)[C alloc] initWithClientInfo:@"extension" withMediaDomains:dom error:&e];
    if (!c) { fprintf(stderr, "init falhou: %s\n", e.description.UTF8String); return 1; }
    NSError *oe = nil; NSDictionary *resp = nil;
    BOOL ok = [c synchronousOpenMediaDomains:&oe withResponseDict:&resp];
    fprintf(stderr, "open=%d err=%s biblioteca=%s\n", ok, oe ? oe.description.UTF8String : "-",
            [[c respondsToSelector:@selector(libraryFilePath)] ? [c libraryFilePath] : @"?" description].UTF8String);
    __block NSDictionary *info = nil; __block id err = nil;
    [c synchronousStorageInfoForDomains:dom withMediaKinds:kinds ignorePurgeable:ip withReply:^(id a, id b) {
      info = a; err = b;
    }];
    if (!info) { fprintf(stderr, "sem resposta: %s\n", [err description].UTF8String); return 1; }
    NSArray *items = info[@"mediaItems"];
    unsigned long long soma = 0;
    for (NSDictionary *it in items) soma += [it[@"FileSize"] unsignedLongLongValue];
    printf("%s ignorePurgeable=%d totalBytes=%llu purgeableBytes=%llu itens=%lu somaFileSize=%llu\n",
           tv ? "TV" : "Music", ip, [info[@"totalBytes"] unsignedLongLongValue],
           [info[@"purgeableBytes"] unsignedLongLongValue], (unsigned long)items.count, soma);
    if (verbose) for (NSDictionary *it in items) printf("%s\n", it.description.UTF8String);
  }
  return 0;
}
