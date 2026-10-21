// attrtag.c - Frente C: o que e ATTR_CMNEXT_ATTRIBUTION_TAG e se identifica o app criador.
// Somente leitura em arquivos existentes (getattrlist nao le conteudo).
// Uso: attrtag [-q] <arquivo>...   -> hash do tag via getattrlist; -q tambem tenta F_QUERY_TAG (abre o arquivo)
// Resultado em macOS 27 / APFS Data: volume nao suporta (supported=0), hash sempre 0, F_QUERY_TAG = EPERM.
//      attrtag -mk <arquivo novo>  -> cria arquivo novo (so no scratchpad) >small, e consulta
// Compilar: cc -O2 -o attrtag attrtag.c
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdlib.h>
#include <sys/attr.h>
#include <sys/mount.h>

static void volcap(const char *p) {
    struct attrlist al = {0};
    al.bitmapcount = ATTR_BIT_MAP_COUNT;
    al.volattr = ATTR_VOL_INFO | ATTR_VOL_CAPABILITIES;
    struct { uint32_t len; vol_capabilities_attr_t caps; } __attribute__((packed)) buf;
    struct statfs sf; statfs(p, &sf);
    if (getattrlist(sf.f_mntonname, &al, &buf, sizeof buf, 0) != 0) { perror("vol getattrlist"); return; }
    uint32_t v = buf.caps.valid[VOL_CAPABILITIES_INTERFACES], c = buf.caps.capabilities[VOL_CAPABILITIES_INTERFACES];
    printf("volume %s: ATTRIBUTION_TAG valid=%d supported=%d\n", sf.f_mntonname,
           !!(v & VOL_CAP_INT_ATTRIBUTION_TAG), !!(c & VOL_CAP_INT_ATTRIBUTION_TAG));
}

static int want_fcntl = 0;
static void query(const char *p) {
    struct attrlist al = {0};
    al.bitmapcount = ATTR_BIT_MAP_COUNT;
    al.commonattr = ATTR_CMN_RETURNED_ATTRS;
    al.forkattr = ATTR_CMNEXT_ATTRIBUTION_TAG;
    struct { uint32_t len; attribute_set_t ret; uint64_t tag; } __attribute__((packed)) b;
    memset(&b, 0, sizeof b);
    if (getattrlist(p, &al, &b, sizeof b, FSOPT_ATTR_CMN_EXTENDED | FSOPT_NOFOLLOW) != 0) {
        printf("%s: getattrlist errno=%d (%s)\n", p, errno, strerror(errno)); return;
    }
    if (b.ret.forkattr & ATTR_CMNEXT_ATTRIBUTION_TAG) printf("%s: tag_hash=0x%016llx", p, (unsigned long long)b.tag);
    else printf("%s: sem tag (atributo nao retornado)", p);
    if (!want_fcntl) { printf("\n"); return; }
    // F_QUERY_TAG exige fd. O_RDONLY sem read() nao dispara leitura de conteudo.
    int fd = open(p, O_RDONLY | O_NOFOLLOW | O_NONBLOCK);
    if (fd < 0) { printf(" | open errno=%d\n", errno); return; }
    fattributiontag_t t; memset(&t, 0, sizeof t); t.ft_flags = F_QUERY_TAG;
    if (fcntl(fd, F_ATTRIBUTION_TAG, &t) == 0) printf(" | F_QUERY_TAG name='%s' hash=0x%016llx\n", t.ft_attribution_name, t.ft_hash);
    else printf(" | F_QUERY_TAG errno=%d (%s)\n", errno, strerror(errno));
    close(fd);
}

int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "uso: %s [-mk novo] arquivo...\n", argv[0]); return 2; }
    int i = 1;
    if (!strcmp(argv[1], "-q")) { want_fcntl = 1; i = 2; }
    if (i < argc - 1 && !strcmp(argv[i], "-mk")) {
        i++;
        int fd = open(argv[i], O_CREAT | O_EXCL | O_WRONLY, 0644);
        if (fd < 0) { perror("create"); return 1; }
        char *buf = calloc(1, 1 << 20); memset(buf, 'x', 1 << 20);
        for (int k = 0; k < 8; k++) write(fd, buf, 1 << 20);  // 8 MiB: "excluding small files"
        fsync(fd); close(fd);
    }
    volcap(argv[i]);
    for (; i < argc; i++) query(argv[i]);
    return 0;
}
