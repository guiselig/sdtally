# Runbook: `workspaceStorage` do VS Code com dezenas de GB

## Sintoma

```
~/Library/Application Support/Code/User/workspaceStorage/   19 GB
```

Uma pasta de hash concentra quase tudo; as outras somam poucas centenas de MB.

## Diagnóstico

```bash
du -sh ~/Library/Application\ Support/Code/User/workspaceStorage/* | sort -rh | head
```

Se um workspace domina, desça nele:

```bash
W=~/Library/Application\ Support/Code/User/workspaceStorage/<hash>
du -sh "$W"/* | sort -rh | head
```

O nome da subpasta é o ID da extensão culpada (`redhat.java`, `ms-python.python`, etc).

### Identificar o projeto

```bash
cat "$W/workspace.json"
```

Retorna o `folder` ou um ponteiro para `Code/Workspaces/<id>/workspace.json`. Vale seguir
a cadeia — saber qual projeto é muda a decisão.

---

## Causa: `redhat.java` (JDT Language Server)

Caso confirmado na auditoria de 2026-08-09:

```
$W/redhat.java/jdt_ws/Downloads_227f6993/bin/
├── <show>.mov   10,29 GB
└── <show>.m4v   10,29 GB
```

**O que aconteceu:** a pasta `Downloads` foi aberta como projeto no VS Code. O JDT Language
Server a tratou como projeto Java, criou a estrutura Eclipse (`src/`, `bin/`) e **copiou
todo o conteúdo não-Java para `bin/`**, que é a pasta de saída de build.

Não é índice. Não é cache. São cópias literais dos arquivos.

O nome da pasta (`Downloads_227f6993`) delata o projeto de origem.

### Verificar antes de apagar

```bash
# o que tem lá dentro?
find "$W/redhat.java/jdt_ws" -type f -size +100M -exec stat -f '%z %Sm %N' {} \;

# os originais ainda existem?
find ~ -name "<nome do arquivo>" -size +1G 2>/dev/null

# quando foi usado pela última vez?
find "$W" -type f -mtime -60 | wc -l
```

Retorno `0` no último comando = workspace abandonado.

**Só apague depois de confirmar que os originais existem em outro lugar.** É o passo que
separa limpeza de perda de dados.

---

## Correção

```bash
# mover para o Lixo (não use rm)
mv "$W/redhat.java/jdt_ws" ~/.Trash/jdt_ws_recuperado
```

Se o workspace inteiro estiver abandonado, a pasta de hash toda pode ir.

## Prevenção

Não abra `Downloads`, `Documents` ou o home como projeto no VS Code com extensões de
linguagem ativas. Language servers assumem que a pasta raiz é um projeto e agem conforme.

Para inspecionar arquivo solto, abra o arquivo — não a pasta.

---

## Generalização para o app

Regra de detecção, independente de extensão:

1. Um filho de `workspaceStorage` maior que 10× a mediana dos irmãos → suspeito.
2. Dentro dele, arquivo com extensão de mídia ou arquivo compactado → **quase certamente
   não é cache**.
3. Procurar o original por nome e tamanho no home.
4. Reportar como "cópia acidental" com o caminho do original, não como "cache".

O mesmo padrão vale para qualquer pasta `bin/`, `build/`, `out/`, `target/` ou `dist/`
contendo mídia. Ferramenta de build não produz `.mov`.
