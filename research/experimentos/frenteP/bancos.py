#!/usr/bin/env python3
"""bancos: reproduz "Messages" e "Podcasts" do Ajustes > Armazenamento lendo CÓPIAS dos bancos dos apps.

messages (com.apple.messages.StorageManagementExtension), lido no IMDPersistence:
  valor = Σ das 4 classes de IMDAttachmentSpaceTakenByAttachmentClass =
  SELECT SUM(total_bytes) FROM attachment WHERE hide_attachment=0 AND transfer_state=5
  A função escolhe o WHERE pelo argumento: 0 = sem ck_sync_state, 1 = "!= 0", outro = "!= 1". A extensão
  chama com 0 (lido no binário da extensão e no IMDPersistence), então a variante do Ajustes é a sem
  ck_sync_state; as outras duas saem só para comparação.
  Não é o tamanho de ~/Library/Messages: só anexos baixados (transfer_state 5) e visíveis.
podcasts (com.apple.podcasts.MacPodcastsStorageExtension), lido no binário:
  episódios MTEpisode com currentMediaEnclosure.localURL != NULL AND > "" cujo arquivo existe;
  valor = Σ stm_calculateSizeOfItemAtURL (StorageManagement) de cada um. Essa função lê
  NSURLTotalFileAllocatedSizeKey para arquivo (pasta: _SizeFolder); aqui st_blocks*512 aproxima isso.
  Se a extensão não abre o banco, usa o tamanho de MTConstants.sharedCacheDirectory =
  <grupo 243LU875E5.groups.com.apple.podcasts>/Library/Cache (lido no PodcastsFoundation; não reproduzido).
Só leitura: os bancos (com -wal/-shm) são copiados para um temporário apagado no fim.
uso: python3 bancos.py [messages|podcasts]...
"""
import glob
import os
import shutil
import sqlite3
import sys
import tempfile
import urllib.parse

HOME = os.path.expanduser("~")


def copia(src):
    d = tempfile.mkdtemp()
    for f in glob.glob(glob.escape(src) + "*"):   # banco + -wal + -shm: sem o WAL faltam gravações recentes
        shutil.copy2(f, d)
    return d, os.path.join(d, os.path.basename(src))


def messages():
    d, db = copia(f"{HOME}/Library/Messages/chat.db")
    try:
        c = sqlite3.connect(db)
        for filtro in ("ck_sync_state != 1 AND ", "ck_sync_state != 0 AND ", ""):
            (v,) = c.execute(f"SELECT SUM(total_bytes) FROM attachment WHERE {filtro}hide_attachment=0 AND transfer_state = 5").fetchone()
            print(f"Messages [{filtro or 'sem ck_sync_state (a do Ajustes) '}]= {v or 0}")
    finally:
        shutil.rmtree(d)


def podcasts():
    d, db = copia(f"{HOME}/Library/Group Containers/243LU875E5.groups.com.apple.podcasts/Documents/MTLibrary.sqlite")
    try:
        c = sqlite3.connect(db)
        rows = c.execute("SELECT m.ZLOCALURL FROM ZMTEPISODE e JOIN ZMTMEDIAENCLOSURE m ON m.Z_PK = e.ZCURRENTMEDIAENCLOSURE "
                         "WHERE m.ZLOCALURL IS NOT NULL AND m.ZLOCALURL > ''").fetchall()
    finally:
        shutil.rmtree(d)
    total = n = 0
    for (u,) in rows:
        p = urllib.parse.unquote(urllib.parse.urlparse(u).path) if u.startswith("file:") else u
        try:
            st = os.lstat(p)
        except OSError:
            continue
        n += 1
        total += st.st_blocks * 512
    print(f"Podcasts = {total}  episódios com localURL {len(rows)}, com arquivo {n}")


if __name__ == "__main__":
    for a in sys.argv[1:] or ["messages", "podcasts"]:
        {"messages": messages, "podcasts": podcasts}[a]()
