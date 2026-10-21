# Ajustes do dmgbuild para o DMG do publicar.sh: janela com 600 × 400 de conteúdo e fundo.png (e o @2x),
# o app à esquerda e o atalho de Aplicativos à direita, na altura da seta do fundo.
#   dmgbuild -s scripts/dmg/ajustes.py -D app=<SDTally.app> -D pasta=scripts/dmg "SDTally <versão>" <saída.dmg>
# O dmgbuild executa este arquivo sem __file__: a pasta do fundo vem em -D pasta.
import os.path

app = defines["app"]  # noqa: F821 (o dmgbuild injeta defines)
aqui = defines.get("pasta", "scripts/dmg")  # noqa: F821
nome = os.path.basename(app)

format = "UDZO"
files = [app]
symlinks = {"Applications": "/Applications"}
background = os.path.join(aqui, "fundo.png")
# 32 pt a mais: a barra de título come a altura da janela.
window_rect = ((200, 120), (600, 432))
default_view = "icon-view"
show_status_bar = show_tab_view = show_toolbar = show_pathbar = show_sidebar = False
icon_size = 128
text_size = 13
icon_locations = {nome: (150, 200), "Applications": (450, 200)}
