#include <stdio.h>
#include <fcntl.h>
#include <errno.h>
#include <string.h>
#include <sys/snapshot.h>
int main(int c, char **v){ int fd=open(v[1],O_RDONLY); int r=fs_snapshot_create(fd,v[2],0); printf("fs_snapshot_create=%d errno=%d %s\n",r,errno,strerror(errno)); return 0;}
