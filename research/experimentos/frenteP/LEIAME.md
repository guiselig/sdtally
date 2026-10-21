# Frente P: reprodutores das categorias do Ajustes > Armazenamento

Cada programa refaz, fora do painel, o número de uma categoria com a mesma regra que a extensão
da Apple usa (lida no binário). Todos só leem: bancos de apps são copiados para um temporário, o
CacheDelete só é consultado e o dir-stats roda com buffer zerado. Os programas em C/Objective-C e
o `icloud.py` ligam `setiopolicy_np` (dataless e atime OFF) antes de tudo. Resultado e fontes:
`research/frentes/P-categorias-ajustes.md`.

## Pré-requisitos

- Terminal com **Acesso Total ao Disco** (Fotos, Mensagens, Mail e containers). Sem FDA, abrir a
  biblioteca do Fotos pode travar por mais de 2 min (síntese §4.9).
- Sem sudo. A única parte que precisa de root é "outros usuários" (ver abaixo).
- Compile fora do iCloud (o binário não deve ir para o repositório):

```sh
cd "research/experimentos/frenteP"
B=/tmp/frenteP-bin; mkdir -p $B        # ou o scratchpad da sessão
clang -fobjc-arc -framework Foundation -o $B/stm_categorias stm_categorias.m
clang -fobjc-arc -framework Foundation -o $B/fotos_pl fotos_pl.m
clang -fobjc-arc -framework Foundation -framework CoreServices -lsqlite3 -o $B/livros livros.m
clang -O2 -o $B/tamanhos tamanhos.c    # opcional: lógico x alocado de pastas
```

## Tudo de uma vez (conta da tela)

```sh
perl -e 'alarm 900; exec @ARGV' python3 todas.py $B
```

Imprime cada categoria, o agregado "Developer and Others", Σ categorias, usado do Data,
purgeable e **Dados do Sistema = usado − purgeable − Σ**. Leva ~45 s. Categoria que falhar sai
`nao_medido` e a conta também (nunca 0).

## Um por um

| Categoria | Comando | Linha que vale |
|---|---|---|
| Aplicativos | `perl -e 'alarm 580; exec @ARGV' $B/stm_categorias apps` (`apps-v` lista app a app) | `apps … TOTAL` |
| Documentos | `$B/stm_categorias documents` (`documents-semicloud` se "Mesa e Documentos" do iCloud estiver desligado) | `documents … TOTAL` |
| Desenvolvedor | `$B/stm_categorias developer` | `TOTAL` |
| Mail | `$B/stm_categorias mail` | 1º `TOTAL` |
| Lixo, GarageBand, Arquivos do iOS, Outros usuários | `$B/stm_categorias trash garageband ios otherusers` | `TOTAL` de cada |
| Purgeable do volume | `$B/stm_categorias volume` | `purgeable` |
| Caches do painel (oráculo, pode estar velho) | `$B/stm_categorias cache` | `SPLastDocumentsSize`, `CachedMailFolderSize`, ByHost |
| Música / TV | `python3 musica.py music` / `python3 musica.py tv` (`--lista` por faixa) | `(como o Ajustes) =` |
| Fotos | `$B/fotos_pl` | linha `lógico`: **r2 − r4** |
| Mensagens, Podcasts | `python3 bancos.py messages podcasts` | `sem ck_sync_state (a do Ajustes)`; `Podcasts =` |
| Livros | `$B/livros` | `Books (como o Ajustes) =` |
| iCloud Drive (estimativa, grau B) | `python3 icloud.py` | `iCloud Drive (estimativa) =` |

Arquivos que não são reprodutores do valor, mas documentam a chamada da Apple (recusada fora
do app por entitlement ou TCC): `fotos.m` (CPSConfigurationClient), `musica_tv.m`
(AMPLFrameworkClient) e `itlib.m` (ITLibrary pública; só roda se o terminal tiver "Mídia e Apple
Music"). Build igual ao dos outros `.m`, mais `-framework iTunesLibrary` no `itlib.m`.

## Conferir com a tela (o dono)

O log do painel dura menos de ~21 h. Para comparar no mesmo minuto:

```sh
# terminal 1: grava o que o painel calcula
log stream --style ndjson --predicate 'process == "StorageManagementService" OR process BEGINSWITH "com.apple.STMExtension"' > /tmp/painel.ndjson
# abrir Ajustes > Geral > Armazenamento e esperar ~2 min; então, no terminal 2:
python3 todas.py $B
defaults read com.apple.StorageManagement.Service SPLastDocumentsSize
```

No log, o valor de cada categoria está em `StorageLogInvestigation - <id>: <bytes>` e o conjunto
final em `Skipping near-identical categories update` / `STMVolumeCategory`.

Outros usuários (só se houver outra conta no Mac; precisa de root):
`sudo research/experimentos/frenteM/stmsize_ai /Users/<outro>`.
