// Tamanho de pasta sem abrir: ATTR_DIR_ALLOCSIZE / DATALENGTH e RECURSIVE_GENCOUNT (APFS dir-stats).
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <stdint.h>
#include <sys/attr.h>
#include <unistd.h>
int main(int argc, char **argv) {
  for (int i = 1; i < argc; i++) {
    struct attrlist al = {0};
    al.bitmapcount = ATTR_BIT_MAP_COUNT;
    al.commonattr = ATTR_CMN_RETURNED_ATTRS;
    al.dirattr = ATTR_DIR_ALLOCSIZE | ATTR_DIR_DATALENGTH;
    al.forkattr = ATTR_CMNEXT_RECURSIVE_GENCOUNT;
    char buf[256]; memset(buf, 0, sizeof buf);
    int rc = getattrlist(argv[i], &al, buf, sizeof buf, FSOPT_NOFOLLOW | FSOPT_ATTR_CMN_EXTENDED);
    if (rc) { printf("%-70s erro %s\n", argv[i], strerror(errno)); continue; }
    attribute_set_t *ret = (attribute_set_t *)(buf + 4);
    char *p = buf + 4 + sizeof(attribute_set_t);
    int64_t alloc = -1, len = -1; uint64_t gen = 0;
    if (ret->dirattr & ATTR_DIR_ALLOCSIZE) { memcpy(&alloc, p, 8); p += 8; }
    if (ret->dirattr & ATTR_DIR_DATALENGTH) { memcpy(&len, p, 8); p += 8; }
    if (ret->forkattr & ATTR_CMNEXT_RECURSIVE_GENCOUNT) { memcpy(&gen, p, 8); p += 8; }
    printf("%-70s dir=0x%x ext=0x%x alloc=%.3f GiB len=%.3f GiB gencount=%llu\n", argv[i], ret->dirattr, ret->forkattr,
           alloc / 1073741824.0, len / 1073741824.0, (unsigned long long)gen);
  }
}
