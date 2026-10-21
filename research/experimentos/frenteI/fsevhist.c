// fsevhist: lê o histórico do FSEvents de um volume desde um instante (ex.: a data de um snapshot
// local do Time Machine) e imprime caminho, inode e flags de cada evento. Somente leitura, sem root.
// Uso: fsevhist <mount> <epoch_desde|id:N> [prefixo_relativo_ao_volume] [-q]
//   id:N usa o event id N direto (FSEventsGetLastEventIdForDeviceBeforeTime devolve 0 no macOS 27)
//   -q: só contagem por tipo (para volumes inteiros)
// Também imprime UUID do banco de eventos, id atual e id antes do instante.
#include <CoreServices/CoreServices.h>
#include <sys/stat.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int quiet; static unsigned long long n, nrem, nren, nmod, ncre, nscan, nfile, ndir;
static FSEventStreamEventId first_id, last_id;

static void cb(ConstFSEventStreamRef s, void *ctx, size_t num, void *paths, const FSEventStreamEventFlags f[], const FSEventStreamEventId ids[]) {
  CFArrayRef arr = paths;
  for (size_t i = 0; i < num; i++) {
    if (f[i] & kFSEventStreamEventFlagHistoryDone) {
      fprintf(stderr, "historico_ok eventos=%llu removidos=%llu renomeados=%llu modificados=%llu criados=%llu mustscan=%llu arquivos=%llu pastas=%llu primeiro_id=%llu ultimo_id=%llu\n",
              n, nrem, nren, nmod, ncre, nscan, nfile, ndir, first_id, last_id);
      CFRunLoopStop(CFRunLoopGetMain()); return;
    }
    n++; if (!first_id) first_id = ids[i]; last_id = ids[i];
    if (f[i] & kFSEventStreamEventFlagItemRemoved) nrem++;
    if (f[i] & kFSEventStreamEventFlagItemRenamed) nren++;
    if (f[i] & kFSEventStreamEventFlagItemModified) nmod++;
    if (f[i] & kFSEventStreamEventFlagItemCreated) ncre++;
    if (f[i] & kFSEventStreamEventFlagMustScanSubDirs) nscan++;
    if (f[i] & kFSEventStreamEventFlagItemIsFile) nfile++;
    if (f[i] & kFSEventStreamEventFlagItemIsDir) ndir++;
    if (quiet) continue;
    CFDictionaryRef d = CFArrayGetValueAtIndex(arr, i);
    CFStringRef p = CFDictionaryGetValue(d, kFSEventStreamEventExtendedDataPathKey);
    CFNumberRef ino = CFDictionaryGetValue(d, kFSEventStreamEventExtendedFileIDKey);
    char buf[4096] = "?"; long long inode = -1;
    if (p) CFStringGetCString(p, buf, sizeof buf, kCFStringEncodingUTF8);
    if (ino) CFNumberGetValue(ino, kCFNumberLongLongType, &inode);
    printf("%llu\t0x%08x\t%lld\t%s\n", ids[i], f[i], inode, buf);
  }
}

int main(int argc, char **argv) {
  if (argc < 3) { fprintf(stderr, "uso: %s <mount> <epoch_desde> [prefixo] [-q]\n", argv[0]); return 2; }
  const char *prefix = ""; for (int i = 3; i < argc; i++) { if (!strcmp(argv[i], "-q")) quiet = 1; else prefix = argv[i]; }
  struct stat st; if (stat(argv[1], &st)) { perror("stat"); return 1; }
  dev_t dev = st.st_dev;
  CFUUIDRef u = FSEventsCopyUUIDForDevice(dev);
  CFStringRef us = u ? CFUUIDCreateString(NULL, u) : CFSTR("(nulo)");
  char ubuf[64]; CFStringGetCString(us, ubuf, sizeof ubuf, kCFStringEncodingUTF8);
  CFAbsoluteTime t = strtod(argv[2], 0) - kCFAbsoluteTimeIntervalSince1970;
  FSEventStreamEventId since = strncmp(argv[2], "id:", 3) ? FSEventsGetLastEventIdForDeviceBeforeTime(dev, t)
                                                            : strtoull(argv[2] + 3, 0, 10);
  fprintf(stderr, "dev=%d uuid=%s id_atual=%llu id_antes_do_instante=%llu\n", dev, ubuf, FSEventsGetCurrentEventId(), since);
  CFStringRef ps = CFStringCreateWithCString(NULL, prefix, kCFStringEncodingUTF8);
  CFArrayRef pa = CFArrayCreate(NULL, (const void **)&ps, 1, &kCFTypeArrayCallBacks);
  FSEventStreamRef s = FSEventStreamCreateRelativeToDevice(NULL, cb, NULL, dev, pa, since, 0.1,
      kFSEventStreamCreateFlagFileEvents | kFSEventStreamCreateFlagUseCFTypes | kFSEventStreamCreateFlagUseExtendedData | kFSEventStreamCreateFlagNoDefer);
  if (!s) { fprintf(stderr, "stream nula\n"); return 1; }
  FSEventStreamScheduleWithRunLoop(s, CFRunLoopGetMain(), kCFRunLoopDefaultMode);
  if (!FSEventStreamStart(s)) { fprintf(stderr, "start falhou\n"); return 1; }
  CFRunLoopRun();
  FSEventStreamStop(s); FSEventStreamInvalidate(s); FSEventStreamRelease(s);
  return 0;
}
