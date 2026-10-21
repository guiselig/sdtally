# O modelo de armazenamento do macOS

## "System Data" não é uma categoria

O painel de Armazenamento (Ajustes > Geral > Armazenamento) classifica arquivos em
Aplicativos, Música, Documentos, Fotos, Developer, macOS e **System Data**.

System Data é o resto. Não existe um lugar chamado "System Data" no disco. É o que sobra
depois que o classificador do Finder atribuiu tudo que reconheceu.

Consequência prática: para explicar N GB de System Data, você mede o disco inteiro e
subtrai o que o Finder já classificou. Não existe atalho.

## De onde os números vêm

Caso real (auditoria de 2026-08-09, 437 GB de 494 GB usados, 166 GB em System Data):

| Bloco | Tamanho |
|---|---|
| `~/Library` | 153 GB |
| `/private/var` (db, vm, folders) | 6,0 GB |
| `/opt/homebrew` | 5,8 GB |
| `/Library` (nível sistema) | 5,0 GB |
| `/usr/local` (MySQL, TeXLive) | 2,0 GB |

Soma ≈ 172 GB, contra 166 GB reportados. A diferença cabe na sobreposição entre
categorias e no arredondamento GB/GiB.

**Conclusão: System Data ≈ `~/Library`.** É onde o scanner deve gastar seu tempo.

## Anatomia do `~/Library`

Ordenado pelo caso real acima:

| Subpasta | Tamanho | Natureza |
|---|---|---|
| `Mobile Documents` | 53 GB | Dados reais de iCloud Drive. **Não é lixo** |
| `Application Support` | 40 GB | Estado de apps. Misto: dado real + lixo |
| `Caches` | 32 GB | Descartável, com ressalvas (ver runbook de sync) |
| `Group Containers` | 8,3 GB | Dados de apps sandboxed. Misto |
| `Containers` | 5,4 GB | Dados de apps sandboxed. Misto |
| `Preferences` | 4,6 GB | Deveria ser KB. GBs aqui indicam app abusando |
| `Logs` | 2,7 GB | Descartável |
| `CloudStorage` | 2,4 GB | Google Drive, Dropbox. Placeholders |

`Preferences` merecendo GBs é sinal de anomalia — no caso real, um benchmark guardava
4,5 GB de assets de cena ali.

## GB vs GiB

O Finder usa GB decimal (10^9). `du` e `df` usam GiB binário (2^30). Diferença de ~7%.
Um relatório que mistura as duas unidades parece inconsistente sem estar errado.

O app deve escolher uma unidade e rotular explicitamente.

## Time Machine: snapshots locais

Costumam ser o maior vilão em Macs com backup ativo — dezenas de GB invisíveis.

```bash
tmutil listlocalsnapshots /
```

Se a saída listar snapshots, cada um pode segurar GBs de arquivos já apagados. Remover
**somente** pela ferramenta oficial:

```bash
tmutil deletelocalsnapshots <data-do-snapshot>
```

Nunca manipular o arquivo diretamente.

No caso real auditado não havia snapshots — daí o vilão ter sido outro. O app precisa
checar, não presumir.

## Ver também

- [02-armadilhas-de-medicao.md](02-armadilhas-de-medicao.md) — por que `du` engana
- [04-catalogo-de-alvos.md](04-catalogo-de-alvos.md) — o que dá para limpar
