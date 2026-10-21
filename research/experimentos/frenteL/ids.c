// fileid, parentid, cloneid, clone_refcnt, ext_flags, privatesize, allocsize de cada caminho.
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <sys/attr.h>
#include <unistd.h>
int main(int c, char **v) {
  for (int i = 1; i < c; i++) {
    struct attrlist al = {0}; al.bitmapcount = ATTR_BIT_MAP_COUNT;
    al.commonattr = ATTR_CMN_RETURNED_ATTRS | ATTR_CMN_OBJTYPE | ATTR_CMN_FLAGS | ATTR_CMN_FILEID | ATTR_CMN_PARENTID;
    al.fileattr = ATTR_FILE_ALLOCSIZE;
    al.forkattr = ATTR_CMNEXT_PRIVATESIZE | ATTR_CMNEXT_CLONEID | ATTR_CMNEXT_EXT_FLAGS | ATTR_CMNEXT_CLONE_REFCNT;
    char b[512] = {0};
    if (getattrlist(v[i], &al, b, sizeof b, FSOPT_NOFOLLOW | FSOPT_ATTR_CMN_EXTENDED)) { perror(v[i]); continue; }
    attribute_set_t *r = (attribute_set_t *)(b + 4); char *p = b + 4 + sizeof *r;
    uint32_t t = 0, fl = 0, rc = 0; uint64_t id = 0, par = 0, alloc = 0, priv = 0, cid = 0, ef = 0;
    if (r->commonattr & ATTR_CMN_OBJTYPE) { memcpy(&t, p, 4); p += 4; }
    if (r->commonattr & ATTR_CMN_FLAGS) { memcpy(&fl, p, 4); p += 4; }
    if (r->commonattr & ATTR_CMN_FILEID) { memcpy(&id, p, 8); p += 8; }
    if (r->commonattr & ATTR_CMN_PARENTID) { memcpy(&par, p, 8); p += 8; }
    if (r->fileattr & ATTR_FILE_ALLOCSIZE) { memcpy(&alloc, p, 8); p += 8; }
    if (r->forkattr & ATTR_CMNEXT_PRIVATESIZE) { memcpy(&priv, p, 8); p += 8; }
    if (r->forkattr & ATTR_CMNEXT_CLONEID) { memcpy(&cid, p, 8); p += 8; }
    if (r->forkattr & ATTR_CMNEXT_EXT_FLAGS) { memcpy(&ef, p, 8); p += 8; }
    if (r->forkattr & ATTR_CMNEXT_CLONE_REFCNT) { memcpy(&rc, p, 4); p += 4; }
    printf("t=%u fl=0x%x id=%llu par=%llu cid=%llu ref=%u ef=0x%llx alloc=%llu priv=%llu  %s\n", t, fl, id, par, cid, rc, ef, alloc, priv, v[i]);
  }
}
