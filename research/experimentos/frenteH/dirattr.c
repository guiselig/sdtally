// dirattr: getattrlist numa pasta SEM abri-la (só leitura). Pede atributos de pasta e ext.
// uso: ./dirattr <pasta>...
#include <sys/attr.h>
#include <unistd.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <errno.h>
struct __attribute__((packed)) R {
  uint32_t len; attribute_set_t ret;
  uint32_t flags; uint64_t fileid;
  uint32_t linkcount, entrycount; off_t allocsize, datalength;
  uint64_t privsize, gencount; uint64_t extflags;
};
int main(int c, char **v) {
  for (int i = 1; i < c; i++) {
    struct attrlist al = {ATTR_BIT_MAP_COUNT, 0, ATTR_CMN_RETURNED_ATTRS | ATTR_CMN_FLAGS | ATTR_CMN_FILEID, 0,
      ATTR_DIR_LINKCOUNT | ATTR_DIR_ENTRYCOUNT | ATTR_DIR_ALLOCSIZE | ATTR_DIR_DATALENGTH, 0,
      ATTR_CMNEXT_PRIVATESIZE | ATTR_CMNEXT_RECURSIVE_GENCOUNT | ATTR_CMNEXT_EXT_FLAGS};
    struct R r; memset(&r, 0, sizeof r);
    if (getattrlist(v[i], &al, &r, sizeof r, FSOPT_NOFOLLOW | FSOPT_ATTR_CMN_EXTENDED | FSOPT_PACK_INVAL_ATTRS)) {
      printf("%s: erro %s\n", v[i], strerror(errno)); continue; }
    printf("%s: ret cmn=%x dir=%x fork=%x | flags=%x fileid=%llu link=%u entries=%u alloc=%lld datalen=%lld priv=%llu gencount=%llu ext=%llx\n",
      v[i], r.ret.commonattr, r.ret.dirattr, r.ret.forkattr, r.flags, r.fileid, r.linkcount, r.entrycount,
      (long long)r.allocsize, (long long)r.datalength, r.privsize, r.gencount, r.extflags);
  }
}
