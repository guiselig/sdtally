// cdprobe: purgeable por serviço via CacheDelete.framework (privado), SOMENTE LEITURA.
// Chama só CacheDeleteCopyPurgeableSpaceWithInfo e CacheDeleteCopyItemizedPurgeableSpaceWithInfo.
// Nunca chama nada de purge. Compara com statfs e com as chaves públicas de capacidade do NSURL.
// uso: ./cdprobe [volume] [urgência 0-4]
#import <Foundation/Foundation.h>
#include <dlfcn.h>
#include <sys/mount.h>
typedef CFDictionaryRef (*CopyInfoFn)(CFDictionaryRef);
int main(int argc, char **argv) {
  @autoreleasepool {
    NSString *vol = argc > 1 ? @(argv[1]) : @"/System/Volumes/Data";
    int urg = argc > 2 ? atoi(argv[2]) : 4;
    void *h = dlopen("/System/Library/PrivateFrameworks/CacheDelete.framework/CacheDelete", RTLD_LAZY);
    if (!h) { fprintf(stderr, "%s\n", dlerror()); return 1; }
    CopyInfoFn tot = (CopyInfoFn)dlsym(h, "CacheDeleteCopyPurgeableSpaceWithInfo");
    CopyInfoFn itm = (CopyInfoFn)dlsym(h, "CacheDeleteCopyItemizedPurgeableSpaceWithInfo");
    if (!tot || !itm) { fprintf(stderr, "símbolo ausente\n"); return 1; }
    NSMutableDictionary *info = [@{ @"CACHE_DELETE_VOLUME": vol, @"CACHE_DELETE_URGENCY_LIMIT": @(urg) } mutableCopy];
    if (getenv("CD_NO_CACHE")) info[@"CACHE_DELETE_NO_CACHE"] = @YES; /* pede recálculo, não purga */
    struct statfs s; statfs(vol.fileSystemRepresentation, &s);
    uint64_t freeB = (uint64_t)s.f_bavail * s.f_bsize, totalB = (uint64_t)s.f_blocks * s.f_bsize;
    NSURL *u = [NSURL fileURLWithPath:vol];
    NSDictionary *rv = [u resourceValuesForKeys:@[NSURLVolumeAvailableCapacityKey, NSURLVolumeAvailableCapacityForImportantUsageKey, NSURLVolumeAvailableCapacityForOpportunisticUsageKey] error:nil];
    NSDictionary *a = CFBridgingRelease(tot((__bridge CFDictionaryRef)info));
    NSDictionary *b = CFBridgingRelease(itm((__bridge CFDictionaryRef)info));
    printf("volume %s  urgência %d\n", vol.UTF8String, urg);
    printf("statfs: total %llu  livre(f_bavail) %llu\n", totalB, freeB);
    printf("NSURL: available %s  important %s  opportunistic %s\n", [[rv[NSURLVolumeAvailableCapacityKey] description] UTF8String],
      [[rv[NSURLVolumeAvailableCapacityForImportantUsageKey] description] UTF8String], [[rv[NSURLVolumeAvailableCapacityForOpportunisticUsageKey] description] UTF8String]);
    long long imp = [rv[NSURLVolumeAvailableCapacityForImportantUsageKey] longLongValue];
    printf("purgeable(NSURL important − statfs livre) = %lld\n", imp - (long long)freeB);
    printf("CopyPurgeableSpaceWithInfo: AMOUNT %s  FREESPACE %s  NONPURGEABLE %s\n", [[a[@"CACHE_DELETE_AMOUNT"] description] UTF8String],
      [[a[@"CACHE_DELETE_FREESPACE"] description] UTF8String], [[a[@"CACHE_DELETE_NONPURGEABLE_AMOUNT"] description] UTF8String]);
    printf("Itemized: TOTAL_AVAILABLE %s  SHARED_PURGEABLE %s  TOTAL_FSPURGEABLE %s  FREESPACE %s\n",
      [[b[@"CACHE_DELETE_TOTAL_AVAILABLE"] description] UTF8String], [[b[@"CACHE_DELETE_SHARED_PURGEABLE"] description] UTF8String],
      [[b[@"CACHE_DELETE_TOTAL_FSPURGEABLE"] description] UTF8String], [[b[@"CACHE_DELETE_FREESPACE"] description] UTF8String]);
    unsigned long long sum = 0;
    NSArray *keys = [[b allKeys] sortedArrayUsingComparator:^NSComparisonResult(id x, id y) { return [b[y] isKindOfClass:NSNumber.class] && [b[x] isKindOfClass:NSNumber.class] ? [b[y] compare:b[x]] : [x compare:y]; }];
    printf("-- por serviço (bytes), só > 0 --\n");
    for (NSString *k in keys) {
      if ([k hasPrefix:@"CACHE_DELETE_"] || ![b[k] isKindOfClass:NSNumber.class]) continue;
      unsigned long long v = [b[k] unsignedLongLongValue]; sum += v;
      if (v) printf("%16llu  %s\n", v, k.UTF8String);
    }
    printf("soma dos serviços: %llu\n", sum);
    for (NSString *k in keys) if ([k hasPrefix:@"CACHE_DELETE_"] && ![b[k] isKindOfClass:NSNumber.class] && ![k isEqual:@"CACHE_DELETE_NAME_MAP"] && ![k isEqual:@"CACHE_DELETE_VOLUME"])
      printf("extra %s = %s\n", k.UTF8String, [[b[k] description] UTF8String]);
  }
  return 0;
}
