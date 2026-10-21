// livros: reproduz "Books" do Ajustes > Armazenamento (com.apple.iBooksX.DiskSpaceEfficiency).
// Lido no binário (BooksStorageExtension, arm64e):
//   -[DiskSpaceEfficiencyHandler totalSpaceUsed] = -[BookManager totalSpaceUsed] = ivar _totalSpaceUsed,
//   somado em _loadResourceItems: com [BKAgent fetchBooks:] (BookKit, privado) e, para cada livro,
//   extensionItemFromBookMetaData:added: que DESCARTA o livro se: path vazio, isSample, itemName vazio,
//   sem BKGeneratedItemId e sem itemId, ou [NSFileManager isUbiquitousItemAtURL:path] (livro no iCloud).
//   Tamanho do livro = max(BKAllocatedSize, kMDItemFSSize do path via MDItemCreate).
// O BKAgentService só aceita o próprio Books (fora dele: Cocoa 4097). Então, se o agente recusar,
// a lista vem de uma CÓPIA do banco que o agente mantém (BKLibrary-1-091020131601.sqlite, tabela
// ZBKLIBRARYASSET): path=ZPATH, isSample=ZISSAMPLE, itemName=ZTITLE, itemId=ZASSETID|ZSTOREID,
// BKAllocatedSize=ZFILESIZE (correspondência de nomes é hipótese; o filtro e o max são do binário).
// Só leitura: chama apenas fetchBooks: (listagem); o banco é copiado para um temporário.
// build: clang -fobjc-arc -framework Foundation -framework CoreServices -lsqlite3 -o livros livros.m
#import <Foundation/Foundation.h>
#import <CoreServices/CoreServices.h>
#include <dlfcn.h>
#include <sqlite3.h>
#include <sys/resource.h>

@interface BKShim : NSObject
- (id)initWithError:(NSError **)e;
- (void)fetchBooks:(void (^)(NSArray *, NSError *))r;
@end

// Lista do banco copiado, no mesmo formato de dicionário que fetchBooks: devolve.
static NSArray *books_from_db(void) {
  NSString *src = [NSHomeDirectory() stringByAppendingPathComponent:
    @"Library/Containers/com.apple.iBooksX/Data/Documents/BKLibrary/BKLibrary-1-091020131601.sqlite"];
  NSString *dir = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
  NSFileManager *fm = NSFileManager.defaultManager;
  [fm createDirectoryAtPath:dir withIntermediateDirectories:YES attributes:nil error:nil];
  NSString *dst = [dir stringByAppendingPathComponent:src.lastPathComponent];
  for (NSString *suf in @[@"", @"-wal", @"-shm"])   // WAL junto, senão faltam gravações recentes
    [fm copyItemAtPath:[src stringByAppendingString:suf] toPath:[dst stringByAppendingString:suf] error:nil];
  NSMutableArray *out = [NSMutableArray array];
  sqlite3 *db; sqlite3_stmt *st;
  if (sqlite3_open(dst.fileSystemRepresentation, &db) == SQLITE_OK &&
      sqlite3_prepare_v2(db, "SELECT ZPATH, ZISSAMPLE, ZTITLE, ZASSETID, ZSTOREID, ZFILESIZE FROM ZBKLIBRARYASSET", -1, &st, NULL) == SQLITE_OK) {
    while (sqlite3_step(st) == SQLITE_ROW) {
      NSString *(^col)(int) = ^NSString *(int i) { const unsigned char *t = sqlite3_column_text(st, i); return t ? @((const char *)t) : @""; };
      [out addObject:@{@"path": col(0), @"isSample": @(sqlite3_column_int(st, 1)), @"itemName": col(2), @"itemId": col(3),
                       @"BKGeneratedItemId": col(4), @"BKAllocatedSize": @(sqlite3_column_int64(st, 5))}];
    }
    sqlite3_finalize(st);
  } else { fprintf(stderr, "sqlite: %s\n", sqlite3_errmsg(db)); out = nil; }
  sqlite3_close(db);
  [fm removeItemAtPath:dir error:nil];   // só o temporário criado aqui
  return out;
}

int main(void) {
  setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF);
  setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES, IOPOL_SCOPE_PROCESS, IOPOL_ATIME_UPDATES_OFF);
  @autoreleasepool {
    if (!dlopen("/System/Library/PrivateFrameworks/BookKit.framework/Versions/A/BookKit", RTLD_LAZY)) { fprintf(stderr, "%s\n", dlerror()); return 1; }
    NSError *e = nil;
    __block NSArray *books = nil; __block NSError *err = nil;
    BKShim *ag = [[NSClassFromString(@"BKAgent") alloc] initWithError:&e];
    if (ag) {
      dispatch_semaphore_t s = dispatch_semaphore_create(0);
      [ag fetchBooks:^(NSArray *b, NSError *x) { books = b; err = x; dispatch_semaphore_signal(s); }];
      if (dispatch_semaphore_wait(s, dispatch_time(DISPATCH_TIME_NOW, 60 * NSEC_PER_SEC))) fprintf(stderr, "fetchBooks: timeout\n");
    } else fprintf(stderr, "BKAgent recusou (%s); usando cópia do BKLibrary\n", e.localizedDescription.UTF8String);
    if (!books) books = books_from_db();
    if (!books) { fprintf(stderr, "sem lista de livros: não medido\n"); return 1; }
    NSFileManager *fm = NSFileManager.defaultManager;
    unsigned long long total = 0; unsigned n = 0, fora = 0;
    for (NSDictionary *b in books) {
      NSString *path = b[@"path"], *name = b[@"itemName"];
      NSString *gid = [b[@"BKGeneratedItemId"] description], *iid = [b[@"itemId"] description];
      const char *motivo = NULL;
      if (!path.length) motivo = "sem path";
      else if ([b[@"isSample"] boolValue]) motivo = "amostra";
      else if (!name.length || (!gid.length && !iid.length)) motivo = "sem nome/id";
      else if ([fm isUbiquitousItemAtURL:[NSURL fileURLWithPath:path]]) motivo = "no iCloud";
      if (motivo) { fora++; printf("fora (%s)\t%s\n", motivo, path.UTF8String ?: "-"); continue; }
      unsigned long long sz = [b[@"BKAllocatedSize"] unsignedLongLongValue];
      MDItemRef it = MDItemCreate(NULL, (__bridge CFStringRef)path);
      if (it) {
        NSNumber *fs = CFBridgingRelease(MDItemCopyAttribute(it, kMDItemFSSize));
        if (fs.unsignedLongLongValue > sz) sz = fs.unsignedLongLongValue;
        CFRelease(it);
      }
      n++; total += sz; printf("conta\t%llu\t%s\n", sz, path.UTF8String);
    }
    printf("Books (como o Ajustes) = %llu  livros contados %u, descartados %u, total na lista %lu\n", total, n, fora,
           (unsigned long)books.count);
  }
  return 0;
}
