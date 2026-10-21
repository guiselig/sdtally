# compara dir-stats (fsctl) com soma própria de st_blocks, separando comprimidos (UF_COMPRESSED)
import os, sys, stat, subprocess
UF_COMPRESSED = 0x20
for d in sys.argv[1:]:
    seen=set(); tot=0; comp=0; n=0; ncomp=0
    for r, ds, fs in os.walk(d):
        for x in ds + fs:
            st = os.lstat(os.path.join(r, x)); n += 1
            if st.st_ino in seen: continue
            seen.add(st.st_ino); b = st.st_blocks * 512; tot += b
            if st.st_flags & UF_COMPRESSED: comp += b; ncomp += 1
    ds_ = subprocess.run([os.path.join(os.path.dirname(__file__) or '.', 'dirstats'), d], capture_output=True, text=True).stdout.split('\t')
    print(f"{d}: dirstats={ds_[1]} walk={tot} walk_sem_comprimidos={tot-comp} comprimidos={ncomp} ({comp} B)")
