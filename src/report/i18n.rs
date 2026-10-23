//! pt-BR e en em tabela estática (Frente E §1.3). Chave faltando num idioma quebra o teste;
//! na tela ela aparece como ⟦chave⟧, nunca some.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Pt,
    En,
}

impl Lang {
    pub fn de(s: &str) -> Option<Lang> {
        match s {
            "pt" | "pt-BR" | "pt_BR" => Some(Lang::Pt),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    /// pt se LC_ALL, LC_MESSAGES ou LANG (o primeiro não vazio) começa com "pt"; senão en.
    pub fn do_ambiente() -> Lang {
        let v = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().filter_map(|k| std::env::var(k).ok()).find(|v| !v.is_empty());
        if v.is_some_and(|v| v.starts_with("pt")) { Lang::Pt } else { Lang::En }
    }

    fn tabela(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Lang::Pt => PT,
            Lang::En => EN,
        }
    }

    pub fn t(self, k: &str) -> String {
        self.tabela().iter().find(|x| x.0 == k).map_or_else(|| format!("⟦{k}⟧"), |x| x.1.to_string())
    }

    /// Troca {0}, {1}... pelos argumentos numa passada só: texto inserido (um caminho com "{1}")
    /// não é varrido de novo.
    pub fn f(self, k: &str, args: &[&str]) -> String {
        let t = self.t(k);
        let mut out = String::with_capacity(t.len());
        let mut resto = t.as_str();
        while let Some(i) = resto.find('{') {
            out.push_str(&resto[..i]);
            let depois = &resto[i + 1..];
            match depois.find('}').and_then(|j| Some((j, depois[..j].parse::<usize>().ok().filter(|n| *n < args.len())?))) {
                Some((j, n)) => {
                    out.push_str(args[n]);
                    resto = &depois[j + 1..];
                }
                None => {
                    out.push('{');
                    resto = depois;
                }
            }
        }
        out.push_str(resto);
        out
    }

    /// Como `f`, com a chave singular `k_1` quando n == 1 (pt e en só têm "one" e "other").
    pub fn pf(self, k: &str, n: u64, args: &[&str]) -> String {
        let k1 = format!("{k}_1");
        let k = if n == 1 && self.tabela().iter().any(|x| x.0 == k1) { k1.as_str() } else { k };
        self.f(k, args)
    }

    /// `pf` com o próprio n formatado como {0}.
    pub fn p(self, k: &str, n: u64) -> String {
        self.pf(k, n, &[&self.int(n)])
    }

    /// Motivo de "parcial"/"não medido": as fontes escrevem em pt (o JSON do sdc-coleta é pt);
    /// em en troca os trechos conhecidos. Trecho novo sem tradução sai em pt, nunca some.
    pub fn motivo(self, r: &str) -> String {
        if self == Lang::Pt {
            return r.to_string();
        }
        // Motivo que começa por caminho ("/Volumes/Disco de Backup: saiu com erro"): só o que vem
        // depois do último ": " é traduzido, senão o " de " do caminho viraria " of ".
        let (caminho, resto) = match r.rfind(": ") {
            Some(i) if r.starts_with(['/', '~']) => r.split_at(i),
            _ => ("", r),
        };
        format!("{caminho}{}", MOTIVOS_EN.iter().fold(resto.to_string(), |s, (pt, en)| s.replace(pt, en)))
    }

    /// Separador decimal do idioma.
    pub fn dec(self) -> char {
        if self == Lang::Pt { ',' } else { '.' }
    }

    /// Inteiro com separador de milhar (pt 1.234.567; en 1,234,567).
    pub fn int(self, n: u64) -> String {
        let s = n.to_string();
        let sep = if self == Lang::Pt { '.' } else { ',' };
        let mut out = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i).is_multiple_of(3) {
                out.push(sep);
            }
            out.push(c);
        }
        out
    }
}


/// Trechos dos motivos (coleta.rs, sources/*, sys), do mais longo para o mais curto.
static MOTIVOS_EN: &[(&str, &str)] = &[
    (
        "arquivos sem tamanho alocado legível: datavault do sistema, mapeados fora das regiões, já fechados ou de processo inacessível",
        "files without a readable allocated size: system datavault, mapped outside regions, already closed, or owned by an unreadable process",
    ),
    ("MB lógicos pelo lsof, fora da soma", "logical MB per lsof, left out of the sum"),
    ("mais de 16 arquivos com o mesmo tamanho e as mesmas pontas", "more than 16 files with the same size and the same ends"),
    ("arquivo que mudou ou não abriu durante a comparação", "a file changed or did not open during the comparison"),
    ("limite de 20 GB lidos na comparação", "comparison stopped at the 20 GB read limit"),
    ("limite de 60 s na comparação", "comparison stopped at the 60 s limit"),
    ("mais de 5000 candidatos", "more than 5000 candidates"),
    ("travessia interrompida", "scan interrupted"),
    ("nenhum asset no log 'AssetGC determine' das últimas 24 h", "no asset in the 'AssetGC determine' log of the last 24 h"),
    ("símbolo CacheDeleteCopyItemizedPurgeableSpaceWithInfo ausente", "symbol CacheDeleteCopyItemizedPurgeableSpaceWithInfo missing"),
    ("CapacityInUse ausente em volume do disco interno", "CapacityInUse missing on an internal disk volume"),
    ("container do volume Data não encontrado", "Data volume container not found"),
    ("o volume do AppleAPFSUserClient não confere com o ATTR_VOL_SPACEUSED do Data", "the AppleAPFSUserClient volume does not match the Data ATTR_VOL_SPACEUSED"),
    ("contadores de nós do AppleAPFSUserClient incoerentes", "AppleAPFSUserClient node counters are inconsistent"),
    ("resposta curta do AppleAPFSUserClient", "short reply from the AppleAPFSUserClient"),
    ("IOServiceOpen do container falhou", "IOServiceOpen of the container failed"),
    ("do AppleAPFSUserClient falhou", "of the AppleAPFSUserClient failed"),
    ("IOBSDNameMatching sem resposta", "IOBSDNameMatching gave no reply"),
    ("statfs do volume Data errno", "statfs of the Data volume errno"),
    ("itens sem data de entrada no Lixo", "items without a Trash entry date"),
    ("; falta Acesso Total ao Disco", "; Full Disk Access is missing"),
    ("fora do registro do IOKit", "not in the IOKit registry"),
    ("não é um AppleAPFSContainer", "is not an AppleAPFSContainer"),
    ("nome de volume inválido", "invalid volume name"),
    ("algum item sem tamanho lido", "some item without a size read"),
    ("~/.Trash sem permissão", "~/.Trash: no permission"),
    ("~/.Trash não abriu", "~/.Trash did not open"),
    ("sem sufixo sN", "without an sN suffix"),
    ("o pai de ", "the parent of "),
    ("seletor ", "selector "),
    ("arquivos sem tamanho alocado legível", "files without a readable allocated size"),
    ("pasta ausente: modelos não baixados", "folder missing: models not downloaded"),
    ("pastas/entradas não lidas", "folders/entries not read"),
    ("dir-stats 0 (não mantido)", "dir-stats 0 (not maintained)"),
    ("falha interna (panic capturado)", "internal failure (panic caught)"),
    ("CacheDelete devolveu tipo inesperado", "CacheDelete returned an unexpected type"),
    ("não converteu a saída de", "did not convert the output of"),
    ("thread da travessia morreu", "scan thread died"),
    ("CacheDelete devolveu nulo", "CacheDelete returned null"),
    ("dlopen CacheDelete falhou", "dlopen of CacheDelete failed"),
    ("objetos com xattr ilegível", "objects with unreadable xattr"),
    ("caminho não é UTF-8", "path is not UTF-8"),
    ("pastas sem dir-stats", "folders without dir-stats"),
    (".AssetData sem dir-stats", ".AssetData without dir-stats"),
    ("CacheDelete sem serviços", "CacheDelete without services"),
    ("log do mobileassetd", "mobileassetd log"),
    ("snapshot sem XID", "snapshot without XID"),
    ("travessia falhou", "scan failed"),
    ("saiu com erro", "exited with an error"),
    ("falha interna", "internal failure"),
    ("sem travessia", "no scan"),
    ("interrompido", "interrupted"),
    ("timeout de", "timeout after"),
    ("sem entrada", "no entry"),
    (" falhou", " failed"),
    (" de ", " of "),
];

static PT: &[(&str, &str)] = &[
    ("unidade_gb", "GB = 10^9 bytes, como o Ajustes"),
    ("unidade_gib", "GiB = 2^30 bytes"),
    ("incompleto", "INCOMPLETO"),
    ("motivo_interrupted", "interrompido"),
    ("motivo_no_root", "sem sudo"),
    ("motivo_no_full_disk_access", "sem Acesso Total ao Disco"),
    ("motivo_stage_failed", "etapa com falha"),
    ("interrompido_nota", "Ctrl-C: a leitura parou; os números abaixo são só do que foi lido até ali."),
    ("falta_lsof", "arquivos apagados e abertos: só os processos do seu usuário"),
    ("falta_negadas", "{0} pastas sem permissão de leitura: entram por dir-stats quando o APFS guarda o número"),
    ("falta_negadas_1", "{0} pasta sem permissão de leitura: entra por dir-stats quando o APFS guarda o número"),
    ("rode_sudo", "Para a conta completa: sudo sdtally"),
    ("conceda_fda", "Conceda Acesso Total ao Disco ao seu terminal (Ajustes > Privacidade e Segurança) e rode de novo; {0} pastas foram negadas."),
    ("conceda_fda_1", "Conceda Acesso Total ao Disco ao seu terminal (Ajustes > Privacidade e Segurança) e rode de novo; {0} pasta foi negada."),
    ("fonte", "fonte"),
    ("tela", "Tela do Ajustes (reproduzida)"),
    ("tela_macos", "macOS (Σ CapacityInUse − Data)"),
    ("tela_ai", "Apple Intelligence (dentro de Dados do Sistema)"),
    ("tela_ds", "Dados do Sistema (log do serviço, {0})"),
    ("tela_ds_sem", "Dados do Sistema"),
    ("sem_registro", "sem registro"),
    ("tela_ds_instrucao", "Abra Ajustes do Sistema > Geral > Armazenamento, espere carregar e rode de novo (o log guarda o valor por menos de ~21 h)."),
    ("fonte_capacity", "diskutil"),
    ("fonte_log", "log show"),
    ("volume_data", "Volume Data"),
    ("mais_pesa", "O que mais pesa ({0} de {1} caminhos)"),
    ("mais_pesa_1", "O que mais pesa ({0} de {1} caminho)"),
    ("demais", "demais {0} caminhos medidos"),
    ("demais_1", "demais {0} caminho medido"),
    ("balde_demais", "(demais)"),
    ("item_snapshots", "Snapshots locais ({0})"),
    ("item_apagados", "Arquivos apagados ainda abertos"),
    ("item_xattr", "Atributos estendidos grandes (xattr)"),
    ("item_metadados", "Metadados do APFS"),
    ("fonte_travessia", "travessia"),
    ("fonte_xattr", "listxattr"),
    ("fonte_snapshots", "fsctl"),
    ("fonte_lsof", "lsof +L1"),
    ("fonte_lsof_proc", "lsof +L1 (quem mais segura: {0}, {1})"),
    ("fonte_dirstats", "dir-stats"),
    ("fonte_diferenca", "diferença ({0} do usado)"),
    ("fonte_diferenca_teto", "diferença ({0} do usado; inclui o que não foi medido)"),
    ("fonte_sobreposicao", "as partes somam {0} a mais que o usado: pastas protegidas contam o que já está nas legíveis"),
    ("fonte_cachedelete", "CacheDelete"),
    ("fonte_ai", "dir-stats, como o Ajustes; físico {0}"),
    ("estado_titulo", "Parcial ou não medido (o que falta fica bloqueado pelo macOS)"),
    ("termo_tree", "árvore de arquivos"),
    ("termo_large_xattrs", "xattrs grandes"),
    ("termo_snapshots", "snapshots"),
    ("termo_deleted_open", "apagados e abertos"),
    ("termo_protected_folders", "pastas protegidas (dir-stats)"),
    ("termo_apfs_metadata", "metadados do APFS"),
    ("parcial", "parcial"),
    ("nao_medido", "não medido"),
    ("nao_medido_curto", "não medido"),
    ("nao_disp", "não disponível nesta versão"),
    ("nao_disp_curto", "não disp."),
    ("sem_explicacao", "Bloqueado pelo macOS"),
    ("informativo", "Informativo (não somar: já está nas linhas acima ou é outra conta)"),
    ("purgeable", "O macOS libera sozinho (purgeable)"),
    ("nao_voltou", "Por que o espaço pode não voltar (só o que foi medido)"),
    ("clones", "Clones completos repetidos ({0})"),
    ("hardlinks", "Hardlinks repetidos ({0})"),
    ("enxertos", "Raízes de enxerto puladas: {0} (blocos já contados)"),
    ("dataless", "Só na nuvem (dataless): {0}, {1}"),
    ("fonte_dataless", "tamanho aparente; 0 no disco"),
    ("duplicados", "Cópias idênticas: {0} a mais"),
    ("fonte_conteudo", "conteúdo, byte a byte"),
    ("aparentes", "Arquivos que aparecem maiores do que ocupam"),
    ("esparso", "esparso, aparece como {0}"),
    ("comprimido", "comprimido, {0} sem compressão"),
    ("fonte_addedtime", "getattrlist"),
    ("fonte_apfs_uc", "IOKit APFS"),
    ("apfs_meta", "Metadados do APFS (árvores vivas do volume)"),
    ("apfs_meta_nota", "Nós omap, fsroot e extentref × {0} B. Já entra na conta acima (a travessia só soma arquivos). Não inclui as árvores dos snapshots."),
    ("trash_old", "Lixo: itens há mais de 30 dias ({0} de {1})"),
    ("trash_old_none", "Lixo: itens há mais de 30 dias"),
    ("trash_note", "De {0} {1} em ~/.Trash. A idade é a data em que o item entrou no Lixo, não a de modificação. Só o Lixo desta conta, neste volume; outros volumes não são percorridos."),
    ("trash_undated", "{0} itens sem data de entrada ({1}) ficam fora da conta de idade."),
    ("macos_volumes", "Volumes à parte, somados no macOS e não em Dados do Sistema: {0}"),
    ("papel_system", "Sistema"),
    ("papel_sem", "sem papel"),
    ("n_arquivos", "{0} arquivos"),
    ("n_arquivos_1", "{0} arquivo"),
    ("n_pastas", "{0} pastas"),
    ("n_pastas_1", "{0} pasta"),
    ("erros_etapas", "{0} etapas da coleta falharam (detalhes: sdtally --json, campo stage_errors)"),
    ("erros_etapas_1", "{0} etapa da coleta falhou (detalhes: sdtally --json, campo stage_errors)"),
    // drill
    ("ramo", "{0} (só este ramo, profundidade {1})"),
    ("col_privado", "libera se apagar"),
    ("soltos", "(arquivos nesta pasta)"),
    ("ramo_total", "Total lido"),
    ("ramo_negada_sem", "{0} pastas sem permissão e sem dir-stats: não medidas"),
    ("ramo_negada_sem_1", "{0} pasta sem permissão e sem dir-stats: não medida"),
    ("ramo_erros", "{0} entradas com erro de leitura"),
    ("ramo_erros_1", "{0} entrada com erro de leitura"),
    ("ramo_outro_volume", "{0} montagens de outros volumes não cruzadas"),
    ("ramo_outro_volume_1", "{0} montagem de outro volume não cruzada"),
    ("ramo_nota_privado", "Libera se apagar = privado do APFS: desconta clones e o que um snapshot ainda segura."),
    ("outro_volume", "{0} está em outro volume; o sdtally só mede o volume Data (/System/Volumes/Data)."),
    ("nao_e_pasta", "{0} não é uma pasta; use: sdtally why {0}"),
    // why
    ("why_tipo", "tipo"),
    ("why_arquivo", "arquivo"),
    ("why_pasta", "pasta"),
    ("why_link", "link simbólico"),
    ("why_outro", "outro"),
    ("why_logico", "lógico (tamanho aparente)"),
    ("why_alocado", "alocado"),
    ("why_privado", "privado (libera se apagar)"),
    ("why_tamanho", "tamanho"),
    ("why_pasta_tamanho", "não se aplica à pasta; o conteúdo está em dir-stats (abaixo) e em sdtally {0}"),
    ("why_ids", "fileid / cloneid"),
    ("why_ext", "ext flags"),
    ("why_flags", "flags"),
    ("why_links", "links"),
    ("why_dev", "dispositivo"),
    ("why_xattrs", "xattrs"),
    ("why_xattrs_val", "{0} ({1} bytes; {2} bytes em fluxo próprio, fora do alocado)"),
    ("why_dataless", "dataless (só na nuvem)"),
    ("why_comprimido", "comprimido (APFS)"),
    ("why_enxerto", "raiz de enxerto"),
    ("why_dirstats", "dir-stats (APFS)"),
    ("sim", "sim"),
    ("nao", "não"),
    ("ausente", "ausente"),
    ("erro_errno", "erro (errno {0})"),
    ("why_nota", "Nada do conteúdo foi lido: só metadados (getattrlist, lstat, listxattr)."),
    ("progresso", "sdtally: {0} arquivos lidos"),
    // argumentos e início
    ("arg_numero", "{0} precisa de um número > 0"),
    ("arg_lang", "--lang precisa de pt ou en"),
    ("arg_cor", "--color precisa de auto, always ou never"),
    ("arg_desconhecida", "opção desconhecida: {0} (veja --help)"),
    ("arg_uso", "uso: sdtally [<caminho> | why <caminho>] (veja --help)"),
    ("arg_quick", "--quick só vale com --json e sem caminho"),
    ("arg_watch", "--watch só vale com --quick --json, sem --since nem --parts"),
    ("arg_auto", "--auto não combina com outras opções"),
    ("arg_since", "--since só vale com --quick e pede os segundos desde a época da leitura anterior"),
    ("arg_parts", "--parts só vale com --quick e pede uma lista de meta, volumes, swap, uso, purgeable e tela separada por vírgula"),
    ("arg_catalog", "--catalog exige --json e não aceita caminho nem outro modo"),
    ("arg_ia", "--last, --history e --changes exigem --json; --mcp vai sem --json; nenhum aceita caminho, --quick ou outro deles"),
    ("arg_root", "não rode como root: --last, --history, --changes, --mcp e --verify nunca precisam de root"),
    ("arg_redact", "--redact precisa de names ou off e só vale com saída JSON de medição (--json, --quick, --last, --history, --changes, --mcp, --stdin)"),
    ("arg_stdin", "--stdin exige --redact names e não aceita caminho nem outro modo"),
    ("arg_verify", "--verify não aceita caminho, --quick nem outro modo"),
    ("verify_titulo", "Conferência com os comandos da Apple (lado a lado, agora)"),
    ("verify_colunas", "item · sdtally · Apple (antes / depois) · resultado"),
    ("verify_equal", "igual"),
    ("verify_within", "dentro (o disco mudou entre as leituras)"),
    ("verify_outside", "fora"),
    ("verify_mesma_fonte", "mesma fonte, não dá para conferir"),
    ("verify_sem_comando", "sem comando da Apple equivalente"),
    ("verify_rodape", "Só leitura. Nada foi gravado."),
    ("seguranca_falhou", "setiopolicy_np falhou (materialize={0}, atime={1}); abortando para não alterar nada."),
    ("sigaction_falhou", "sigaction errno {0}; Ctrl-C vai encerrar sem relatório parcial"),
    ("ajuda", "sdtally {0}: explica o \"Dados do Sistema\" do macOS com medição, sem estimativa.
Só leitura.

uso:
  sudo sdtally [--top N] [--json]
      a conta do volume Data (sem sudo sai INCOMPLETO)
  sdtally <caminho> [--top N] [--depth N]
      filhos de uma pasta por alocado único
  sdtally why <caminho>
      atributos brutos de um arquivo ou pasta
  sdtally --quick --json [--since SEGUNDOS_DA_EPOCA] [--parts LISTA]
      só o que muda a toda hora (tela do Ajustes, usado, livre, purgeable), ~1 s; com --since
      (carimbo da leitura anterior) lê o log da tela só dali em diante e só com o painel aberto;
      --parts meta,volumes,swap,uso,purgeable,tela lê só essas (as outras chaves somem do JSON)
  sdtally --quick --json --watch
      escuta o log da tela (log stream) e escreve uma linha JSON (tela e macOS) a cada tela nova
      do Ajustes; termina quando o stdin fecha
  sdtally --auto
      árvore viva: uma passada inteira e depois só as pastas que o FSEvents avisar; escreve uma
      linha JSON a cada mudança de 10 MB ou mais; como root (o helper do app) abre também as pastas
      protegidas; termina quando o stdin fecha
  sdtally --last --json | --history --json | --changes --json
      a medição completa mais nova que o app salvou | o resumo de todas | o que mudou entre as duas
      mais novas
  sdtally --mcp
      servidor MCP por stdio para agentes de IA (quick, last_measurement, history, what_changed); sem root;
      redige por padrão (--redact off desliga)
  sdtally --redact names --stdin
      redige o JSON de uma medição lido da entrada (até 20 MB)
  sdtally --catalog --json
      as fichas do catálogo e a conferência de cada fonte
  sdtally --verify [--json]
      confere usado, swap e snapshots com os comandos da Apple, agora; sem root

opções:
  --top N                    linhas do ranking (padrão 15)
  --depth N                  níveis abaixo do caminho (padrão 1)
  --json                     JSON em stdout (schema em docs/json-schema.md)
  --quick                    modo rápido, sem travessia (exige --json)
  --binary                   GiB (2^30) em vez de GB (10^9)
  --lang pt|en               idioma (padrão: LC_ALL, LC_MESSAGES ou LANG)
  --color auto|always|never  cor (auto: só em terminal e sem NO_COLOR)
  --redact names|off         troca usuário e pastas pessoais no JSON (padrão: só no --mcp)

Conceda Acesso Total ao Disco ao app de terminal. Nada é gravado em disco."),
];

static EN: &[(&str, &str)] = &[
    ("unidade_gb", "GB = 10^9 bytes, as in System Settings"),
    ("unidade_gib", "GiB = 2^30 bytes"),
    ("incompleto", "INCOMPLETE"),
    ("motivo_interrupted", "interrupted"),
    ("motivo_no_root", "no sudo"),
    ("motivo_no_full_disk_access", "no Full Disk Access"),
    ("motivo_stage_failed", "stage failed"),
    ("interrompido_nota", "Ctrl-C: reading stopped; the numbers below cover only what was read up to that point."),
    ("falta_lsof", "deleted-but-open files: only your own processes"),
    ("falta_negadas", "{0} folders without read permission: counted via dir-stats when APFS keeps the number"),
    ("falta_negadas_1", "{0} folder without read permission: counted via dir-stats when APFS keeps the number"),
    ("rode_sudo", "For the complete breakdown: sudo sdtally"),
    ("conceda_fda", "Grant Full Disk Access to your terminal (System Settings > Privacy & Security) and run again; {0} folders were denied."),
    ("conceda_fda_1", "Grant Full Disk Access to your terminal (System Settings > Privacy & Security) and run again; {0} folder was denied."),
    ("fonte", "source"),
    ("tela", "System Settings screen (reproduced)"),
    ("tela_macos", "macOS (Σ CapacityInUse − Data)"),
    ("tela_ai", "Apple Intelligence (inside System Data)"),
    ("tela_ds", "System Data (service log, {0})"),
    ("tela_ds_sem", "System Data"),
    ("sem_registro", "no record"),
    ("tela_ds_instrucao", "Open System Settings > General > Storage, wait for it to load and run again (the log keeps the value for less than ~21 h)."),
    ("fonte_capacity", "diskutil"),
    ("fonte_log", "log show"),
    ("volume_data", "Data volume"),
    ("mais_pesa", "What takes the most space ({0} of {1} paths)"),
    ("mais_pesa_1", "What takes the most space ({0} of {1} path)"),
    ("demais", "{0} other measured paths"),
    ("demais_1", "{0} other measured path"),
    ("balde_demais", "(others)"),
    ("item_snapshots", "Local snapshots ({0})"),
    ("item_apagados", "Deleted files still open"),
    ("item_xattr", "Large extended attributes (xattr)"),
    ("item_metadados", "APFS metadata"),
    ("fonte_travessia", "scan"),
    ("fonte_xattr", "listxattr"),
    ("fonte_snapshots", "fsctl"),
    ("fonte_lsof", "lsof +L1"),
    ("fonte_lsof_proc", "lsof +L1 (holding the most: {0}, {1})"),
    ("fonte_dirstats", "dir-stats"),
    ("fonte_diferenca", "difference ({0} of used)"),
    ("fonte_diferenca_teto", "difference ({0} of used; includes what was not measured)"),
    ("fonte_sobreposicao", "the parts add up to {0} more than used: protected folders count what readable ones already hold"),
    ("fonte_cachedelete", "CacheDelete"),
    ("fonte_ai", "dir-stats, as Settings does; physical {0}"),
    ("estado_titulo", "Partial or not measured (whatever is missing stays blocked by macOS)"),
    ("termo_tree", "file tree"),
    ("termo_large_xattrs", "large xattrs"),
    ("termo_snapshots", "snapshots"),
    ("termo_deleted_open", "deleted but open"),
    ("termo_protected_folders", "protected folders (dir-stats)"),
    ("termo_apfs_metadata", "APFS metadata"),
    ("parcial", "partial"),
    ("nao_medido", "not measured"),
    ("nao_medido_curto", "unmeasured"),
    ("nao_disp", "not available in this version"),
    ("nao_disp_curto", "n/a"),
    ("sem_explicacao", "Blocked by macOS"),
    ("informativo", "Informational (do not add up: already counted above or a different measure)"),
    ("purgeable", "macOS frees this on its own (purgeable)"),
    ("nao_voltou", "Why space may not come back (measured only)"),
    ("clones", "Repeated full clones ({0})"),
    ("hardlinks", "Repeated hardlinks ({0})"),
    ("enxertos", "Graft roots skipped: {0} (blocks already counted)"),
    ("dataless", "Cloud only (dataless): {0}, {1}"),
    ("fonte_dataless", "apparent size; 0 on disk"),
    ("duplicados", "Identical copies: {0} extra"),
    ("fonte_conteudo", "content, byte by byte"),
    ("aparentes", "Files that look larger than they take"),
    ("esparso", "sparse, appears as {0}"),
    ("comprimido", "compressed, {0} uncompressed"),
    ("fonte_addedtime", "getattrlist"),
    ("fonte_apfs_uc", "IOKit APFS"),
    ("apfs_meta", "APFS metadata (live trees of the volume)"),
    ("apfs_meta_nota", "Nodes of the omap, fsroot and extentref trees × {0} B. Already counted above (the scan only adds up files). Snapshot trees are not included."),
    ("trash_old", "Trash: items older than 30 days ({0} of {1})"),
    ("trash_old_none", "Trash: items older than 30 days"),
    ("trash_note", "Out of {0} {1} in ~/.Trash. Age is the date the item entered the Trash, not its modification date. Only this account's Trash on this volume; other volumes are not scanned."),
    ("trash_undated", "{0} items without an entry date ({1}) are left out of the age count."),
    ("macos_volumes", "Separate volumes, added into macOS and not into System Data: {0}"),
    ("papel_system", "System"),
    ("papel_sem", "no role"),
    ("n_arquivos", "{0} files"),
    ("n_arquivos_1", "{0} file"),
    ("n_pastas", "{0} folders"),
    ("n_pastas_1", "{0} folder"),
    ("erros_etapas", "{0} collection stages failed (details: sdtally --json, field stage_errors)"),
    ("erros_etapas_1", "{0} collection stage failed (details: sdtally --json, field stage_errors)"),
    // drill
    ("ramo", "{0} (this branch only, depth {1})"),
    ("col_privado", "freed if deleted"),
    ("soltos", "(files in this folder)"),
    ("ramo_total", "Total read"),
    ("ramo_negada_sem", "{0} folders without permission and without dir-stats: not measured"),
    ("ramo_negada_sem_1", "{0} folder without permission and without dir-stats: not measured"),
    ("ramo_erros", "{0} entries with read errors"),
    ("ramo_erros_1", "{0} entry with a read error"),
    ("ramo_outro_volume", "{0} mounts of other volumes not crossed"),
    ("ramo_outro_volume_1", "{0} mount of another volume not crossed"),
    ("ramo_nota_privado", "Freed if deleted = APFS private size: excludes clones and whatever a snapshot still holds."),
    ("outro_volume", "{0} is on another volume; sdtally only measures the Data volume (/System/Volumes/Data)."),
    ("nao_e_pasta", "{0} is not a folder; use: sdtally why {0}"),
    // why
    ("why_tipo", "type"),
    ("why_arquivo", "file"),
    ("why_pasta", "folder"),
    ("why_link", "symbolic link"),
    ("why_outro", "other"),
    ("why_logico", "logical (apparent size)"),
    ("why_alocado", "allocated"),
    ("why_privado", "private (freed if deleted)"),
    ("why_tamanho", "size"),
    ("why_pasta_tamanho", "does not apply to a folder; its contents are in dir-stats (below) and in sdtally {0}"),
    ("why_ids", "fileid / cloneid"),
    ("why_ext", "ext flags"),
    ("why_flags", "flags"),
    ("why_links", "links"),
    ("why_dev", "device"),
    ("why_xattrs", "xattrs"),
    ("why_xattrs_val", "{0} ({1} bytes; {2} bytes in a separate stream, outside allocated)"),
    ("why_dataless", "dataless (cloud only)"),
    ("why_comprimido", "compressed (APFS)"),
    ("why_enxerto", "graft root"),
    ("why_dirstats", "dir-stats (APFS)"),
    ("sim", "yes"),
    ("nao", "no"),
    ("ausente", "absent"),
    ("erro_errno", "error (errno {0})"),
    ("why_nota", "No content was read: metadata only (getattrlist, lstat, listxattr)."),
    ("progresso", "sdtally: {0} files read"),
    // argumentos e início
    ("arg_numero", "{0} needs a number > 0"),
    ("arg_lang", "--lang needs pt or en"),
    ("arg_cor", "--color needs auto, always or never"),
    ("arg_desconhecida", "unknown option: {0} (see --help)"),
    ("arg_uso", "usage: sdtally [<path> | why <path>] (see --help)"),
    ("arg_quick", "--quick only works with --json and no path"),
    ("arg_watch", "--watch only works with --quick --json, without --since or --parts"),
    ("arg_auto", "--auto takes no other options"),
    ("arg_since", "--since only works with --quick and takes the seconds since the epoch of the previous reading"),
    ("arg_parts", "--parts only works with --quick and takes a comma-separated list of meta, volumes, swap, uso, purgeable and tela"),
    ("arg_catalog", "--catalog requires --json and takes no path or other mode"),
    ("arg_ia", "--last, --history and --changes require --json; --mcp goes without --json; none takes a path, --quick or another of them"),
    ("arg_root", "do not run as root: --last, --history, --changes, --mcp and --verify never need root"),
    ("arg_redact", "--redact needs names or off and only works with measurement JSON output (--json, --quick, --last, --history, --changes, --mcp, --stdin)"),
    ("arg_stdin", "--stdin requires --redact names and takes no path or other mode"),
    ("arg_verify", "--verify takes no path, --quick or other mode"),
    ("verify_titulo", "Check against Apple's commands (side by side, now)"),
    ("verify_colunas", "item · sdtally · Apple (before / after) · result"),
    ("verify_equal", "equal"),
    ("verify_within", "within (the disk changed between readings)"),
    ("verify_outside", "outside"),
    ("verify_mesma_fonte", "same source, cannot be checked"),
    ("verify_sem_comando", "no equivalent Apple command"),
    ("verify_rodape", "Read-only. Nothing was written."),
    ("seguranca_falhou", "setiopolicy_np failed (materialize={0}, atime={1}); aborting so that nothing is changed."),
    ("sigaction_falhou", "sigaction errno {0}; Ctrl-C will exit without a partial report"),
    ("ajuda", "sdtally {0}: explains macOS \"System Data\" with measurements, no estimates.
Read-only.

usage:
  sudo sdtally [--top N] [--json]
      the Data volume breakdown (without sudo it is INCOMPLETE)
  sdtally <path> [--top N] [--depth N]
      children of a folder by unique allocated size
  sdtally why <path>
      raw attributes of a file or folder
  sdtally --quick --json [--since EPOCH_SECONDS] [--parts LIST]
      only what changes all the time (Settings screen, used, free, purgeable), ~1 s; with --since
      (stamp of the previous reading) reads the screen log only from there on, and only while the
      Settings pane is open; --parts meta,volumes,swap,uso,purgeable,tela reads only those (the
      other keys are left out of the JSON)
  sdtally --quick --json --watch
      listens to the screen log (log stream) and writes one JSON line (screen and macOS) for
      each new Settings screen; ends when stdin closes
  sdtally --auto
      live tree: one full pass, then only the folders FSEvents reports; writes one JSON line per
      change of 10 MB or more; as root (the app helper) it also opens protected folders;
      ends when stdin closes
  sdtally --last --json | --history --json | --changes --json
      the newest complete measurement saved by the app | a summary of all of them | what changed
      between the two newest
  sdtally --mcp
      MCP server over stdio for AI agents (quick, last_measurement, history, what_changed); no root;
      redacts by default (--redact off turns it off)
  sdtally --redact names --stdin
      redacts the JSON of a measurement read from stdin (up to 20 MB)
  sdtally --catalog --json
      the catalog cards and how each source was checked
  sdtally --verify [--json]
      checks used, swap and snapshots against Apple's commands, now; no root

options:
  --top N                    ranking lines (default 15)
  --depth N                  levels below the path (default 1)
  --json                     JSON on stdout (schema in docs/json-schema.md)
  --quick                    quick mode, no scan (requires --json)
  --binary                   GiB (2^30) instead of GB (10^9)
  --lang pt|en               language (default: LC_ALL, LC_MESSAGES or LANG)
  --color auto|always|never  color (auto: only on a terminal and without NO_COLOR)
  --redact names|off         replaces user and personal folders in the JSON (default: only with --mcp)

Grant Full Disk Access to your terminal app. Nothing is written to disk."),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn chaves(t: &'static [(&'static str, &'static str)]) -> BTreeSet<&'static str> {
        t.iter().map(|x| x.0).collect()
    }

    fn marcas(s: &str) -> Vec<String> {
        (0..10).map(|i| format!("{{{i}}}")).filter(|m| s.contains(m.as_str())).collect()
    }

    #[test]
    fn idiomas_completos() {
        assert_eq!(chaves(PT).len(), PT.len(), "chave repetida em pt");
        assert_eq!(chaves(EN).len(), EN.len(), "chave repetida em en");
        let (pt, en) = (chaves(PT), chaves(EN));
        assert_eq!(pt.difference(&en).collect::<Vec<_>>(), Vec::<&&str>::new(), "faltam em en");
        assert_eq!(en.difference(&pt).collect::<Vec<_>>(), Vec::<&&str>::new(), "faltam em pt");
        for (k, v) in PT {
            let e = Lang::En.t(k);
            assert!(!v.is_empty() && !e.is_empty(), "texto vazio: {k}");
            assert_eq!(marcas(v), marcas(&e), "marcas {{n}} diferentes em {k}");
        }
    }

    /// Toda chave literal usada no código tem texto (idiomas_completos só compara pt com en).
    #[test]
    fn chaves_usadas_existem() {
        let pt = chaves(PT);
        for f in [include_str!("texto.rs"), include_str!("mod.rs"), include_str!("../bin/sdtally.rs"), include_str!("../verify.rs")] {
            for pre in [".t(\"", ".f(\"", ".p(\"", ".pf(\""] {
                for (i, _) in f.match_indices(pre) {
                    let k = &f[i + pre.len()..];
                    let k = &k[..k.find('"').expect("aspas")];
                    assert!(pt.contains(k), "chave sem texto: {k}");
                }
            }
        }
    }

    #[test]
    fn formatos() {
        assert_eq!(Lang::Pt.int(1234567), "1.234.567");
        assert_eq!(Lang::En.int(1234567), "1,234,567");
        assert_eq!(Lang::En.int(12), "12");
        assert_eq!(Lang::Pt.f("demais", &["3"]), "demais 3 caminhos medidos");
        assert_eq!(Lang::Pt.p("demais", 1), "demais 1 caminho medido");
        assert_eq!(Lang::En.p("demais", 1200), "1,200 other measured paths");
        assert_eq!(Lang::En.p("n_pastas", 1), "1 folder");
        // argumento com "{1}" não é varrido de novo; marca sem argumento fica como está
        assert_eq!(Lang::Pt.f("ramo", &["/a{1}", "2"]), "/a{1} (só este ramo, profundidade 2)");
        assert_eq!(Lang::Pt.f("ramo", &["x"]), "x (só este ramo, profundidade {1})");
        assert_eq!(Lang::Pt.t("nao_existe"), "⟦nao_existe⟧");
        assert_eq!(Lang::En.motivo("/usr/sbin/diskutil: saiu com erro"), "/usr/sbin/diskutil: exited with an error");
        assert_eq!(Lang::En.motivo("getattrlist ATTR_VOL_SPACEUSED falhou (errno 5)"), "getattrlist ATTR_VOL_SPACEUSED failed (errno 5)");
        assert_eq!(Lang::de("pt"), Some(Lang::Pt));
        assert_eq!(Lang::de("fr"), None);
        assert_eq!(Lang::En.motivo("3 de 20 arquivos sem tamanho alocado legível"), "3 of 20 files without a readable allocated size");
        assert_eq!(
            Lang::En.motivo("3 de 20 arquivos sem tamanho alocado legível: datavault do sistema, mapeados fora das regiões, já fechados ou de processo inacessível (3929 MB lógicos pelo lsof, fora da soma)"),
            "3 of 20 files without a readable allocated size: system datavault, mapped outside regions, already closed, or owned by an unreadable process (3929 logical MB per lsof, left out of the sum)"
        );
        assert_eq!(Lang::Pt.motivo("interrompido"), "interrompido");
        // O caminho na frente do motivo não é traduzido.
        assert_eq!(Lang::En.motivo("/Volumes/Disco de Backup: saiu com erro"), "/Volumes/Disco de Backup: exited with an error");
        assert_eq!(Lang::En.motivo("/Users/fulano/Pasta de Fotos: timeout de 10 s"), "/Users/fulano/Pasta de Fotos: timeout after 10 s");
        assert_eq!(Lang::En.motivo("~/.Trash sem permissão (errno 1); falta Acesso Total ao Disco"), "~/.Trash: no permission (errno 1); Full Disk Access is missing");
        assert_eq!(Lang::En.motivo("12 pastas/entradas não lidas"), "12 folders/entries not read");
        assert_eq!(Lang::En.motivo("dir-stats 0 (não mantido)"), "dir-stats 0 (not maintained)");
    }
}
