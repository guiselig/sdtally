// tmsnap: tamanho privado de snapshots APFS sem root, via TimeMachine.framework (privado).
// Chama APENAS métodos de leitura: snapshotsForLiveVolumeMountPoint, calculatePrivateSizeOf*.
// uso: ./tmsnap [/System/Volumes/Data]
#import <Foundation/Foundation.h>
#import <objc/message.h>
#include <dlfcn.h>
typedef id (*F2)(id,SEL,id,NSError**);
typedef id (*F3)(id,SEL,id,id,NSError**);
typedef id (*FX)(id,SEL,unsigned long long,unsigned long long,id,NSError**);
int main(int argc, char **argv) {
  @autoreleasepool {
    NSString *mpS = argc > 1 ? @(argv[1]) : @"/System/Volumes/Data";
    NSURL *mp = [NSURL fileURLWithPath:mpS isDirectory:YES];
    if (!dlopen("/System/Library/PrivateFrameworks/TimeMachine.framework/Versions/A/TimeMachine", RTLD_LAZY)) { puts(dlerror()); return 1; }
    Class S = NSClassFromString(@"TMSnapshot");
    NSError *err = nil;
    NSArray *snaps = ((F2)objc_msgSend)(S, NSSelectorFromString(@"snapshotsForLiveVolumeMountPoint:error:"), mpS, &err);
    if (err) printf("snapshots err=%s\n", err.description.UTF8String);
    snaps = [snaps sortedArrayUsingDescriptors:@[[NSSortDescriptor sortDescriptorWithKey:@"xid" ascending:YES]]];
    err = nil;
    id total = ((F2)objc_msgSend)(S, NSSelectorFromString(@"calculatePrivateSizeOfSnapshotsForLiveVolumeMountPoint:error:"), mpS, &err);
    printf("TOTAL privado (todos os snapshots): %s bytes err=%s\n", [[total description] UTF8String], err ? err.description.UTF8String : "-");
    printf("%-50s %10s %6s %16s %16s\n", "nome", "xid", "flags", "privado(nome)", "privado(xid..xid)");
    for (id s in snaps) {
      NSString *n = [s valueForKey:@"snapshotName"]; unsigned long long x = [[s valueForKey:@"xid"] unsignedLongLongValue];
      NSError *e1 = nil, *e2 = nil;
      id a = ((F3)objc_msgSend)(S, NSSelectorFromString(@"calculatePrivateSizeOfSnapshotsWithNames:forLiveVolumeMountPoint:error:"), @[n], mp, &e1);
      id b = ((FX)objc_msgSend)(S, NSSelectorFromString(@"calculatePrivateSizeOfSnapshotsFromXID:toXID:forLiveVolumeMountPoint:error:"), x, x, mp, &e2);
      printf("%-50s %10llu 0x%-4x %16s %16s%s%s\n", n.UTF8String, x, [[s valueForKey:@"flags"] intValue],
             [[a description] UTF8String], [[b description] UTF8String],
             e1 ? [@" e1=" stringByAppendingString:e1.localizedDescription].UTF8String : "",
             e2 ? [@" e2=" stringByAppendingString:e2.localizedDescription].UTF8String : "");
    }
    if (snaps.count) { NSError *e = nil;
      id c = ((FX)objc_msgSend)(S, NSSelectorFromString(@"calculatePrivateSizeOfSnapshotsFromXID:toXID:forLiveVolumeMountPoint:error:"),
               [[snaps.firstObject valueForKey:@"xid"] unsignedLongLongValue], [[snaps.lastObject valueForKey:@"xid"] unsignedLongLongValue], mp, &e);
      printf("privado(xid primeiro..último): %s err=%s\n", [[c description] UTF8String], e ? e.description.UTF8String : "-"); }
  }
  return 0;
}
