// stm_categorias: reproduz, fora do sandbox, as 8 categorias do Ajustes > Armazenamento que vêm das
// extensões com.apple.STMExtension.{Applications, Documents, Developer, Mail, Trash, GarageBand,
// OtherUsers, iOSFiles}. Regras lidas nos binários (appex + StorageManagementService, arm64e):
//   - tamanho de pasta = STMSizeDirectoryWithExcludedChildURLs (StorageManagement, privado), o mesmo
//     _NodeSizingV2 do DesktopServicesPriv que o STMDirectorySizer embutido em cada appex usa;
//   - tamanho de arquivo = alocado (NSURLTotalFileAllocatedSizeKey, = st_blocks*512 se não comprimido).
// Só leitura. Única chamada fora de tamanho: CacheDeleteCopyItemizedPurgeableSpaceWithInfo (Mail), que
// só CONSULTA o purgeable do serviço com.apple.MailCacheDelete (não purga).
// Antes de tudo: não materializar dataless e não atualizar atime (setiopolicy_np). A travessia é da
// Apple (NodeSizingV2), não nossa: não dá para impor "não cruzar DEVID"; os caminhos daqui são do
// volume Data. Onde não mede, imprime "nao_medido" (nunca 0 por falta de medida).
// uso: ./stm_categorias [developer|otherusers|trash|ios|garageband|mail|documents|documents-semicloud|
//                         apps|apps-v|cache|volume]...   (sem argumento: todas menos volume; apps-v lista app a app)
// build: clang -fobjc-arc -framework Foundation -o stm_categorias stm_categorias.m
// Conferência 29/09: Developer 2.050.097.152 e OtherUsers 884.736 iguais ao byte ao painel de 28/09.
// Apps (com os restritos sem container) 81.686.274.048 às 15:05; o painel de 28/09 14:11 deu
// 81.500.606.464, mas os containers mudam ~165 MB em 20 min: só vale comparar no mesmo minuto.
#import <Foundation/Foundation.h>
#include <dlfcn.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <unistd.h>

typedef unsigned long long (*SizeFn)(NSURL *, NSArray *);
typedef CFDictionaryRef (*CDFn)(CFDictionaryRef);
static SizeFn stmSize;
static NSString *H;  // home
static NSNumber *homeVol;

static BOOL existe(NSString *p) { struct stat s; return lstat(p.fileSystemRepresentation, &s) == 0; }
static BOOL ehPasta(NSString *p) { struct stat s; return lstat(p.fileSystemRepresentation, &s) == 0 && S_ISDIR(s.st_mode); }

// -1 = caminho não existe (a Apple não soma o que não tem tamanho)
static long long tam(NSString *p, NSArray *excl) {
  if (!existe(p)) return -1;
  NSURL *u = [NSURL fileURLWithPath:p];
  if (!ehPasta(p)) { NSNumber *n; [u getResourceValue:&n forKey:NSURLTotalFileAllocatedSizeKey error:nil]; return n.longLongValue; }
  NSMutableArray *e = [NSMutableArray array];
  for (NSString *x in excl) [e addObject:[NSURL fileURLWithPath:x]];
  return (long long)stmSize(u, e);
}
static NSString *h(NSString *rel) { return [H stringByAppendingPathComponent:rel]; }
static void linha(NSString *cat, NSString *item, long long v) {
  if (v < 0) printf("%-11s\tausente\t%s\n", cat.UTF8String, item.UTF8String);
  else printf("%-11s\t%lld\t%s\n", cat.UTF8String, v, item.UTF8String);
}
static long long somaLista(NSString *cat, NSArray *ps) {
  long long t = 0;
  for (NSString *p in ps) { long long v = tam(p, nil); linha(cat, p, v); if (v > 0) t += v; }
  return t;
}
static NSArray *filhos(NSString *d, BOOL pulaPonto) {
  NSMutableArray *r = [NSMutableArray array];
  for (NSString *n in [[NSFileManager defaultManager] contentsOfDirectoryAtPath:d error:nil])
    if (!(pulaPonto && [n hasPrefix:@"."])) [r addObject:[d stringByAppendingPathComponent:n]];
  return r;
}
static void total(NSString *cat, long long t) { printf("%-11s\t%lld\tTOTAL\n", cat.UTF8String, t); }

// Developer: DeveloperStorageExtension. Itens com caminho sem tamanho não somam; a tela só lista > 10 MB,
// mas o total soma tudo. ~/Library/Caches/com.apple.dt.Xcode fica 0 no painel pelo sandbox da extensão
// (só lê ~/Library/Developer e /AppleInternal/Developer); aqui é medido e mostrado à parte.
static void developer(void) {
  NSMutableArray *ps = [@[ h(@"Library/Developer/CoreSimulator/Caches"), h(@"Library/Developer/Xcode/DerivedData"),
                           h(@"Library/Developer/Xcode/Installs"), h(@"Library/Developer/Xcode/Archives"),
                           @"/Library/Developer/CommandLineTools", @"/Library/Developer/CoreSimulator" ] mutableCopy];
  for (NSString *os in @[ @"iOS", @"watchOS", @"tvOS", @"macOS", @"visionOS" ])
    [ps addObjectsFromArray:filhos(h([NSString stringWithFormat:@"Library/Developer/Xcode/%@ DeviceSupport", os]), YES)];
  [ps addObjectsFromArray:filhos(@"/AppleInternal/Developer/KDKs", YES)];
  total(@"developer", somaLista(@"developer", ps));
  linha(@"developer*", h(@"Library/Caches/com.apple.dt.Xcode (fora do painel: sandbox)"), tam(h(@"Library/Caches/com.apple.dt.Xcode"), nil));
}

// OtherUsers: /Users/Shared + /Users/Deleted Users + outros usuários (SMUtilities_Client, root).
static void otherusers(void) {
  long long t = somaLista(@"otherusers", @[ @"/Users/Shared/", @"/Users/Deleted Users/" ]);
  for (NSString *u in filhos(@"/Users", YES)) {
    NSString *n = u.lastPathComponent;
    if ([u isEqual:H] || [n isEqual:@"Shared"] || [n isEqual:@"Deleted Users"] || [n isEqual:@"Guest"]) continue;
    printf("otherusers \tnao_medido\t%s (precisa root: sudo ./stmsize_ai %s, de frenteM)\n", u.UTF8String, u.UTF8String);
  }
  total(@"otherusers", t);
}

// Trash: filhos sem "." de ~/.Trash e /.Trashes/<uid>; só entra no total o que está no volume da home.
static void trash(void) {
  long long t = 0;
  NSArray *raizes = @[ h(@".Trash"), [NSString stringWithFormat:@"/.Trashes/%u", getuid()] ];
  for (NSString *r in raizes) {
    if (!existe(r)) { linha(@"trash", r, -1); continue; }
    NSArray *fs = filhos(r, YES);
    if (!fs.count) printf("trash      \t0\t%s (vazia)\n", r.UTF8String);
    for (NSString *p in fs) {
      NSNumber *v; [[NSURL fileURLWithPath:p] getResourceValue:&v forKey:NSURLVolumeIdentifierKey error:nil];
      long long b = tam(p, nil); linha(@"trash", p, b);
      if (b > 0 && [v isEqual:homeVol]) t += b;
    }
  }
  total(@"trash", t);
}

// iOSFiles: backups do MobileSync (sem "." e sem "Logs") + .ipsw do Configurator e do iTunes.
static void ios(void) {
  NSMutableArray *ps = [NSMutableArray array];
  for (NSString *p in filhos(h(@"Library/Application Support/MobileSync/Backup"), YES))
    if (![p.lastPathComponent isEqual:@"Logs"]) [ps addObject:p];
  NSMutableArray *dirs = [NSMutableArray array];
  for (NSString *g in @[ @"K36BKF7T3D", @"4WXS7A4F54" ])
    [dirs addObject:h([NSString stringWithFormat:@"Library/Group Containers/%@.group.com.apple.configurator/Library/Caches/Firmware", g])];
  for (NSString *d in @[ @"iPhone", @"iPad", @"iPod", @"Apple TV" ])
    [dirs addObject:h([NSString stringWithFormat:@"Library/iTunes/%@ Software Updates", d])];
  for (NSString *d in dirs) {
    if (!existe(d)) { linha(@"ios", d, -1); continue; }
    for (NSString *p in filhos(d, YES)) if ([p.pathExtension isEqual:@"ipsw"]) [ps addObject:p];
  }
  total(@"ios", somaLista(@"ios", ps));
}

// GarageBand: só caminhos no FS raiz (filterPaths). ChordTrainer e Learn to Play (fora Basic Lessons)
// não são contados aqui e caem em Dados do Sistema.
static void garageband(void) {
  NSMutableArray *ps = [@[ @"/Library/Application Support/GarageBand/Instrument Library" ] mutableCopy];
  for (NSString *l in @[ @"Sampler Instruments", @"EXS Factory Samples", @"Alchemy Samples", @"Ultrabeat Samples",
                         @"Channel Strip Settings", @"Plug-In Settings" ])
    [ps addObject:[@"/Library/Application Support/Logic" stringByAppendingPathComponent:l]];
  [ps addObjectsFromArray:@[ @"/Library/Audio/Impulse Responses/Apple/", @"/Library/Audio/Apple Loops/Apple",
                             @"/Library/Application Support/GarageBand/Learn to Play/Basic Lessons" ]];
  total(@"garageband", somaLista(@"garageband", ps));
}

// Mail: tamanho(~/Library/Mail) − purgeable do com.apple.MailCacheDelete (urgência 3). Negativo = desconhecido.
static void mail(BOOL usarCache) {
  long long pasta = tam(h(@"Library/Mail"), nil);
  linha(@"mail", h(@"Library/Mail"), pasta);
  void *cd = dlopen("/System/Library/PrivateFrameworks/CacheDelete.framework/CacheDelete", RTLD_LAZY);
  CDFn f = cd ? (CDFn)dlsym(cd, "CacheDeleteCopyItemizedPurgeableSpaceWithInfo") : NULL;
  NSDictionary *q = @{ @"CACHE_DELETE_VOLUME" : H, @"CACHE_DELETE_SERVICES" : @[ @"com.apple.MailCacheDelete" ],
                       @"CACHE_DELETE_URGENCY" : @3, @"CACHE_DELETE_NO_CACHE" : @(!usarCache) };
  CFDictionaryRef r = f ? f((__bridge CFDictionaryRef)q) : NULL;
  NSNumber *pg = r ? ((__bridge NSDictionary *)r)[@"com.apple.MailCacheDelete"] : nil;
  if (!pg) { printf("mail       \tnao_medido\tpurgeable (CacheDelete sem resposta)\n"); if (r) CFRelease(r); return; }
  printf("mail       \t%lld\tpurgeable com.apple.MailCacheDelete (usarCache=%d)\n", pg.longLongValue, usarCache);
  CFRelease(r);
  if (pasta < 0 || pasta - pg.longLongValue < 0) printf("mail       \tnao_medido\tTOTAL (desconhecido)\n");
  else total(@"mail", pasta - pg.longLongValue);
}

// ---- Apps (ApplicationsStorageExtension) ----
static NSArray *appBundles(void) {  // NSMetadataQuery igual ao da extensão
  NSMetadataQuery *q = [NSMetadataQuery new];
  q.predicate = [NSPredicate predicateWithFormat:@"kMDItemContentType == 'com.apple.application-bundle'"];
  q.searchScopes = @[ @"/Applications/", @"/System/Applications", H ];
  __block BOOL fim = NO;
  id obs = [[NSNotificationCenter defaultCenter] addObserverForName:NSMetadataQueryDidFinishGatheringNotification
                                                             object:q queue:nil usingBlock:^(NSNotification *n) { fim = YES; }];
  [q startQuery];
  NSDate *lim = [NSDate dateWithTimeIntervalSinceNow:120];
  while (!fim && [lim timeIntervalSinceNow] > 0) [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.2]];
  [q stopQuery];
  [[NSNotificationCenter defaultCenter] removeObserver:obs];
  NSMutableArray *r = [NSMutableArray array];
  for (NSUInteger i = 0; i < q.resultCount; i++) {
    NSString *p = [[q resultAtIndex:i] valueForAttribute:NSMetadataItemPathKey];
    if (p) [r addObject:p];
  }
  return r;
}
static NSDictionary *mapaContainers(void) {  // bundle id -> ~/Library/Containers/<x>/Data
  NSMutableDictionary *m = [NSMutableDictionary dictionary];
  for (NSString *c in filhos(h(@"Library/Containers"), YES)) {
    NSDictionary *md = [NSDictionary dictionaryWithContentsOfFile:[c stringByAppendingPathComponent:@".com.apple.containermanagerd.metadata.plist"]];
    NSString *id = md[@"MCMMetadataIdentifier"];
    if (id) m[id] = [c stringByAppendingPathComponent:@"Data"];
  }
  return m;
}
static NSArray *appsHome;  // apps achados sob a home fora de ~/Library (Documents os pula)
// Pasta sob /Applications que contém o app (_FolderThatContainsInApplications): tira "/", "Applications"
// e "System" dos componentes; com 2+ sobrando, devolve "/Applications/<primeiro>". Senão nil.
static NSString *pastaEmApplications(NSString *p) {
  NSMutableArray *c = [NSMutableArray array];
  for (NSString *x in p.pathComponents) if (![@[ @"/", @"Applications", @"System" ] containsObject:x]) [c addObject:x];
  return c.count >= 2 ? [@"/Applications" stringByAppendingPathComponent:c[0]] : nil;
}
static void apps(BOOL verboso) {
  NSSet *fora = [NSSet setWithArray:@[ @"com.apple.Music", @"com.apple.iBooksX", @"com.apple.mail", @"com.apple.TV",
                                        @"com.apple.podcasts", @"com.apple.Photos", @"com.apple.MobileSMS" ]];
  NSSet *pastaPai = [NSSet setWithArray:@[ @"net.maxon.cinema4d", @"com.sidefx.HoudiniFX" ]];  // USE_PARENT_FOLDER_SIZE
  NSArray *todos = appBundles();
  NSDictionary *cont = mapaContainers();
  NSMutableArray *emHome = [NSMutableArray array], *itens = [NSMutableArray array];
  NSMutableDictionary *grupos = [NSMutableDictionary dictionary];
  long long nPulados = 0, somaRestritos = 0;
  NSString *lib = [h(@"Library") stringByAppendingString:@"/"];
  for (NSString *p in todos) {
    // -[STMApplicationSizer shouldIgnoreItemAtPath:]
    struct stat s;  // stat (segue symlink), como _PathIsRestricted
    BOOL restrito = stat(p.fileSystemRepresentation, &s) == 0 && (s.st_flags & SF_RESTRICTED);
    NSString *pasta = pastaEmApplications(p);
    if (restrito && pasta) { nPulados++; continue; }
    if ([p hasPrefix:[h(@".Trash") stringByAppendingString:@"/"]] || [p hasPrefix:@"/.Trashes/"] || [p hasPrefix:lib]) { nPulados++; continue; }
    NSString *bid = [NSBundle bundleWithPath:p].bundleIdentifier;
    if (bid && [fora containsObject:bid]) { nPulados++; continue; }
    BOOL aninhado = NO;  // _IsNestedAppPath: algum ancestral .app que é aplicação
    for (NSString *q = p.stringByDeletingLastPathComponent; q.length > 1; q = q.stringByDeletingLastPathComponent)
      if ([q hasSuffix:@".app"]) { NSNumber *ia; [[NSURL fileURLWithPath:q] getResourceValue:&ia forKey:NSURLIsApplicationKey error:nil]; if (ia.boolValue) aninhado = YES; }
    if (aninhado) { nPulados++; continue; }
    // -[STMAppMonitor _updateAppItems:]: container (web app do Safari usa o proxy dentro do container do WebApp)
    NSString *dc = [bid hasPrefix:@"com.apple.Safari.WebApp."]
      ? h([NSString stringWithFormat:@"Library/Containers/com.apple.Safari.WebApp/Data/Library/Containers/%@", bid]) : cont[bid];
    if (dc && !existe(dc)) dc = nil;
    // restrito sem container sai da LISTA, mas o tamanho do bundle entra no total (acumulador [sp+0x60] de
    // _updateAppItems, antes do filtro de volume). Cético 29/09.
    if (!dc && restrito) { somaRestritos += MAX(tam(p, nil), 0); nPulados++; continue; }
    NSMutableDictionary *it = [@{ @"p" : p, @"nome" : p.lastPathComponent.stringByDeletingPathExtension, @"bid" : bid ?: @"",
                                  @"a" : @(MAX(tam(p, nil), 0)), @"d" : @(dc ? MAX(tam(dc, nil), 0) : 0) } mutableCopy];
    [itens addObject:it];
    if (pasta) { if (!grupos[pasta]) grupos[pasta] = [NSMutableArray array]; [grupos[pasta] addObject:it]; }
    if ([p hasPrefix:[H stringByAppendingString:@"/"]]) [emHome addObject:p];
  }
  // pasta em /Applications com o mesmo nome do app (ou override): esse item vira a pasta inteira e os
  // outros apps da pasta saem da lista.
  for (NSString *pasta in grupos) {
    NSMutableDictionary *dono = nil;
    for (NSMutableDictionary *it in grupos[pasta])
      if ([it[@"nome"] isEqual:pasta.lastPathComponent] || [pastaPai containsObject:it[@"bid"]]) { dono = it; break; }
    if (!dono) continue;
    dono[@"a"] = @(MAX(tam(pasta, nil), 0)); dono[@"p"] = pasta;
    for (NSMutableDictionary *it in grupos[pasta]) if (it != dono) [itens removeObject:it];
  }
  long long somaApp = 0, somaDados = 0;
  for (NSDictionary *it in itens) {
    NSNumber *v; [[NSURL fileURLWithPath:it[@"p"]] getResourceValue:&v forKey:NSURLVolumeIdentifierKey error:nil];
    if (![v isEqual:homeVol]) { nPulados++; continue; }
    somaApp += [it[@"a"] longLongValue]; somaDados += [it[@"d"] longLongValue];
    if (verboso) printf("apps       \t%lld\t%lld\t%s\n", [it[@"a"] longLongValue], [it[@"d"] longLongValue], [it[@"p"] UTF8String]);
  }
  long long nItens = itens.count;
  appsHome = emHome;
  long long as = tam(h(@"Library/Application Support"), nil), ms = tam(h(@"Library/Application Support/MobileSync"), nil);
  printf("apps       \t%lld\tbundles (%lld itens, %lld pulados)\n", somaApp, nItens, nPulados);
  printf("apps       \t%lld\tcontainers ~/Library/Containers/<id>/Data dos apps listados\n", somaDados);
  printf("apps       \t%lld\tbundles restritos sem container (fora da lista, dentro do total)\n", somaRestritos);
  linha(@"apps", h(@"Library/Application Support"), as);
  linha(@"apps", h(@"Library/Application Support/MobileSync (subtraído)"), ms);
  // Os overrides (Steam, Radar, Minecraft, Battle.net, Webex) só movem bytes entre itens: total igual.
  total(@"apps", somaApp + somaDados + somaRestritos + (as > 0 ? as : 0) - (ms > 0 ? ms : 0));
}

// Documents (SPInfoDocumentsStorageUsageReporter, no StorageManagementService): home inteira menos os
// excluídos (defaultExcludedURLs + biblioteca do Fotos + 5 caminhos fixos + _APPPATHS_ das extensões),
// e sem os .app. ~/Desktop e ~/Documents só saem se "Mesa e Documentos" do iCloud estiver ligado
// (o log de 28/09 mostra os dois excluídos). Caminhos ocultos da home (~/.cache, ~/.cargo...) CONTAM aqui.
static void documents(BOOL icloudDD) {
  if (!appsHome) apps(NO);
  NSMutableArray *ex = [@[ h(@"Applications"), h(@"Library"), h(@"Pictures/Photos Library.photoslibrary"),
                           h(@"Music/iTunes/iTunes Media"), h(@"Music/Music/Media"), h(@"Movies/Apple TV/Media"),
                           h(@"Movies/TV/Media"), h(@"Movies/TV/TV Library"),
                           // _APPPATHS_ de Music, TV e Trash que caem na home
                           h(@"Music/Music/Music Library.musiclibrary"), h(@"Music/Music/Previous Libraries.localized"),
                           h(@"Music/Music/Media.localized"), h(@"Movies/TV/Media.localized"),
                           h(@"Movies/TV/Previous Libraries.localized"), h(@"Movies/TV/TV Library.tvlibrary"), h(@".Trash") ] mutableCopy];
  if (icloudDD) [ex addObjectsFromArray:@[ h(@"Desktop"), h(@"Documents") ]];
  for (NSString *a in appsHome) {
    BOOL coberto = NO;
    for (NSString *e in ex) if ([a hasPrefix:[e stringByAppendingString:@"/"]]) coberto = YES;
    if (!coberto) [ex addObject:a];
  }
  NSMutableArray *vivos = [NSMutableArray array];
  for (NSString *e in ex) if (existe(e)) [vivos addObject:e];
  NSDate *t = [NSDate date];
  long long v = tam(H, vivos);
  printf("documents  \t%ld\texcluídos existentes (iCloud Mesa e Documentos=%d)\n", (long)vivos.count, icloudDD);
  printf("documents  \t%.1fs\ttempo\n", -[t timeIntervalSinceNow]);
  total(@"documents", v);
}

// Base de "Dados do Sistema": o painel usa Disponível = livre + purgeable (urgência 3, a mesma da
// chave ImportantUsage; síntese §1). purgeable = ImportantUsage − livre, os dois do volume da home.
static void volume(void) {
  NSURL *u = [NSURL fileURLWithPath:H]; NSNumber *livre, *imp;
  [u getResourceValue:&livre forKey:NSURLVolumeAvailableCapacityKey error:nil];
  [u getResourceValue:&imp forKey:NSURLVolumeAvailableCapacityForImportantUsageKey error:nil];
  if (!livre || !imp) { printf("volume     	nao_medido	purgeable\n"); return; }
  printf("volume     \t%lld\tlivre\nvolume     \t%lld\tImportantUsage\n", livre.longLongValue, imp.longLongValue);
  printf("volume     \t%lld\tpurgeable\n", imp.longLongValue - livre.longLongValue);
}

static void cache(void) {
  NSDictionary *sms = [NSDictionary dictionaryWithContentsOfFile:h(@"Library/Preferences/com.apple.StorageManagement.Service.plist")];
  printf("cache      \t%s\tSPLastDocumentsSize\n", [[sms[@"SPLastDocumentsSize"] description] ?: @"ausente" UTF8String]);
  NSDictionary *ml = [NSDictionary dictionaryWithContentsOfFile:h(@"Library/Containers/com.apple.STMExtension.Mail/Data/Library/Preferences/com.apple.STMExtension.Mail.plist")];
  printf("cache      \t%s\tCachedMailFolderSize\n", [[ml[@"CachedMailFolderSize"] description] ?: @"ausente" UTF8String]);
  for (NSString *f in filhos(h(@"Library/Preferences/ByHost"), YES)) {
    if (![f.lastPathComponent hasPrefix:@"com.apple.settings.storage."]) continue;
    NSDictionary *d = [NSDictionary dictionaryWithContentsOfFile:f];
    for (NSString *k in @[ @"AppsSizer", @"ContainerSizer", @"DevCachesSizer" ]) {
      NSDictionary *is = d[k][@"ItemSizes"]; long long s = 0;
      for (NSNumber *n in is.allValues) if ([n isKindOfClass:NSNumber.class]) s += n.longLongValue;
      printf("cache      \t%lld\t%s (%lu itens, %s)\n", s, k.UTF8String, (unsigned long)is.count, [[d[k][@"EventIDDate"] description] ?: @"?" UTF8String]);
    }
  }
}

int main(int argc, char **argv) {
  if (setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF) ||
      setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES, IOPOL_SCOPE_PROCESS, IOPOL_ATIME_UPDATES_OFF)) { perror("setiopolicy_np"); return 1; }
  @autoreleasepool {
    void *hs = dlopen("/System/Library/PrivateFrameworks/StorageManagement.framework/StorageManagement", RTLD_LAZY);
    stmSize = hs ? (SizeFn)dlsym(hs, "STMSizeDirectoryWithExcludedChildURLs") : NULL;
    if (!stmSize) { fprintf(stderr, "símbolo ausente\n"); return 1; }
    H = NSHomeDirectory();
    NSNumber *v; [[NSURL fileURLWithPath:H] getResourceValue:&v forKey:NSURLVolumeIdentifierKey error:nil]; homeVol = v;
    NSMutableArray *cs = [NSMutableArray array];
    for (int i = 1; i < argc; i++) [cs addObject:@(argv[i])];
    if (!cs.count) cs = [@[ @"cache", @"developer", @"otherusers", @"trash", @"ios", @"garageband", @"mail", @"apps", @"documents" ] mutableCopy];
    for (NSString *c in cs) {
      NSDate *t = [NSDate date];
      if ([c isEqual:@"developer"]) developer();
      else if ([c isEqual:@"otherusers"]) otherusers();
      else if ([c isEqual:@"trash"]) trash();
      else if ([c isEqual:@"ios"]) ios();
      else if ([c isEqual:@"garageband"]) garageband();
      else if ([c isEqual:@"mail"]) { mail(YES); mail(NO); }
      else if ([c isEqual:@"apps"]) apps(NO);
      else if ([c isEqual:@"apps-v"]) apps(YES);
      else if ([c isEqual:@"documents"]) documents(YES);
      else if ([c isEqual:@"documents-semicloud"]) documents(NO);
      else if ([c isEqual:@"cache"]) cache();
      else if ([c isEqual:@"volume"]) volume();
      else { fprintf(stderr, "categoria desconhecida: %s\n", c.UTF8String); continue; }
      fprintf(stderr, "[%s %.1fs]\n", c.UTF8String, -[t timeIntervalSinceNow]);
      fflush(stdout);
    }
  }
  return 0;
}
