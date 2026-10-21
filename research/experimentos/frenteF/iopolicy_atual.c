#include <sys/resource.h>
// Mostra a política de materialização herdada pelo processo (0=DEFAULT 1=OFF 2=ON). clang iopolicy_atual.c -o gp
#include <stdio.h>
int main(){printf("proc=%d thread=%d (0=DEFAULT 1=OFF 2=ON)\n",getiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES,IOPOL_SCOPE_PROCESS),getiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES,IOPOL_SCOPE_THREAD));}
