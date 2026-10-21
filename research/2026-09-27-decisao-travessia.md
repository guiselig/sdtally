# Decisão: travessia própria, com técnicas do dua-core — 2026-09-27

**Decisão:** escrever a travessia no próprio projeto (`getattrlistbulk` + pool paralelo),
sem depender do `dua-core`. Reaproveitar do `dua-core` (MIT) as técnicas listadas abaixo,
com atribuição no código.

Código lido: `Byron/dua-cli` commit `d190513` (2026-09-25), crate `dua-core` 4.1.0,
arquivos `crates/dua-lib/src/lib.rs`, `macos/mod.rs`, `macos/attributes.rs`.

---

## Por que não depender do `dua-core`

O `dua_core::Metadata` expõe só: `len`, `allocated_size`, `data_allocated_size`, `clone_id`,
`modified`, `dev`, `ino`, `nlink`, tipo. A lista de atributos pedida ao `getattrlistbulk` é
**constante no código** (`FTS_COMMON_ATTRIBUTES`, `FTS_FILE_ATTRIBUTES`, `APFS_*` em
`attributes.rs`). Não há como pedir mais nada.

Faltam, e são o núcleo do projeto:

| Atributo | Para quê | No `dua-core` |
|---|---|---|
| `ATTR_CMN_FLAGS & SF_DATALESS` | Placeholder de nuvem, 0 no disco | Lê as flags, mas só usa `SF_FIRMLINK`; não expõe |
| `ATTR_CMNEXT_PRIVATESIZE` | Bytes liberados ao apagar; presos em snapshot | Não pede |
| `EF_SHARES_ALL_BLOCKS` | Clone total × parcial | Só filtra por `EF_MAY_SHARE_BLOCKS` para liberar o `clone_id` |
| `EF_IS_SPARSE`, `EF_IS_PURGEABLE` | Esparso, purgeable por arquivo | Não expõe |
| `ATTR_CMNEXT_CLONE_REFCNT` | Quantos clones | Não pede |

Alternativas descartadas:

- **`dua-core` com `skip_metadata` + um `getattrlist` por arquivo:** perde a leitura em lote,
  que é o motivo de usar `getattrlistbulk`. Um syscall a mais por arquivo em 450 mil arquivos.
- **PR no `dua-core` para atributos configuráveis:** bom para o ecossistema, mas muda a API
  pública dele e deixa o projeto esperando review. Pode vir depois, sem bloquear.

## Medição: o paralelismo vale

`~/Library`, 456 mil arquivos, Mac de 10 núcleos (4 de performance). **Com backup do Time
Machine rodando (fase Copying)**, então os tempos têm ruído; repetir sem backup.

| Varredura | Tempo |
|---|---|
| `walk` próprio, 1 thread, com atributos APFS | 38,2 s e 23,2 s (duas rodadas) |
| `dua-core`, 1 thread | 20,1 s |
| `dua-core`, 4 threads | 4,8 s |
| `dua-core`, 8 threads | 13,4 s (fora da curva; backup) |
| `dua-core`, 12 threads | 3,4 s |

Conclusão medida: paralelo dá **~4–6×**. O pool é obrigatório; o formato de travessia do
`dua-core` é o modelo.

Divergência resolvida (medida): o `dua-core` somou **137,9 GiB** alocados em `~/Library` sem
deduplicar nada. O `walk` separa: 106,06 GiB únicos + **10,43 GiB de hardlinks repetidos** +
21,44 GiB de clones repetidos = 137,93 GiB. Bate ao centésimo. Os três números usam o mesmo
`ATTR_FILE_ALLOCSIZE`; resource forks não explicam diferença nenhuma aqui.

Consequência para o relatório: `~/Library` ocupa **106 GiB** reais. O `dua` sem flags diz 138,
o `du` diz 127 (deduplica hardlink, não clone). Três ferramentas, três números; só um é o que o
disco libera ao apagar tudo (e mesmo esse depende dos snapshots, via `PRIVATESIZE`).

## O que copiar do `dua-core` (MIT, com atribuição)

1. **Pool work-stealing** com `crossbeam` (deque LIFO local + injector + stealers), acordando
   um worker por roubo bem-sucedido.
2. **Fallback para `readdir` + `stat`** quando `getattrlistbulk` falha com `EINVAL`, `ENOTSUP`,
   `EOPNOTSUPP` ou `ENOSYS` (volumes não-APFS, SMB, FAT de pendrive).
3. **`ATTR_CMN_ERROR` (0x20000000)** na lista: erro por entrada sem abortar a pasta inteira.
4. **Firmlinks (`SF_FIRMLINK`)**: tratar à parte; `/Applications` e outros aparecem duas vezes
   entre o volume de sistema e o de dados.
5. **Sonda de I/O** (`probe_metadata` / `metadata_is_io_bound`): mede se a pasta está presa em
   I/O e troca de estratégia. Útil em HD externo; baixa prioridade no SSD interno.
6. **Buffer alinhado** para o `getattrlistbulk` e checagem bit a bit do `attribute_set`
   retornado (`attribute_is_packed`), que é o mesmo cuidado já descoberto no `walk.rs`.

## Próximo passo

Transformar `walk.rs` em crate de travessia paralela seguindo os itens 1–4, e medir de novo
sem backup rodando: tempo, soma com e sem dedup de hardlink, e comparação com `du -sxk`.
