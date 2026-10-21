// Lista classes/métodos de frameworks privados que casam com um regex (somente leitura).
#import <Foundation/Foundation.h>
#import <objc/runtime.h>
#include <dlfcn.h>
int main(int argc, char **argv) {
  @autoreleasepool {
    if (argc < 3) { fprintf(stderr, "uso: %s <framework-binary> <regex>\n", argv[0]); return 2; }
    void *h = dlopen(argv[1], RTLD_LAZY);
    if (!h) { fprintf(stderr, "dlopen: %s\n", dlerror()); return 1; }
    Dl_info info; NSRegularExpression *re = [NSRegularExpression regularExpressionWithPattern:@(argv[2]) options:NSRegularExpressionCaseInsensitive error:nil];
    unsigned n; const char **names = objc_copyClassNamesForImage(
      ({ const char *img = argv[1]; unsigned c; const char **imgs = objc_copyImageNames(&c); const char *found = img;
         for (unsigned i=0;i<c;i++) if (strstr(imgs[i], img) || strstr(img, imgs[i])) { found = imgs[i]; break; } found; }), &n);
    (void)info;
    for (unsigned i = 0; i < n; i++) {
      Class cls = objc_getClass(names[i]);
      for (int meta = 0; meta < 2; meta++) {
        Class c = meta ? object_getClass(cls) : cls; unsigned m; Method *ms = class_copyMethodList(c, &m);
        for (unsigned j = 0; j < m; j++) {
          NSString *sel = NSStringFromSelector(method_getName(ms[j]));
          NSString *full = [NSString stringWithFormat:@"%s %c[%s %@]", names[i], meta?'+':'-', names[i], sel];
          if ([re numberOfMatchesInString:full options:0 range:NSMakeRange(0, full.length)])
            printf("%c[%s %s] %s\n", meta?'+':'-', names[i], sel.UTF8String, method_getTypeEncoding(ms[j]));
        }
        free(ms);
      }
    }
    printf("(%u classes na imagem)\n", n);
  }
  return 0;
}
