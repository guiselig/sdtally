//! Lixo (`~/.Trash`) por idade: quanto está lá há mais de 30 dias, pela data em que cada item
//! entrou no Lixo (`ATTR_CMN_ADDEDTIME`: criado ou renomeado para dentro da pasta, getattrlist(2)),
//! nunca pela data de modificação. Só leitura. O tamanho é o alocado da travessia (por item de
//! primeiro nível). Só o Lixo do volume Data: o projeto não percorre `/Volumes/*/.Trashes`.

use crate::ledger::Medida;
use crate::{detalhe, sys, walk};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// "Há mais de" 30 dias, em segundos.
pub const LIMITE_S: i64 = 30 * 86_400;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lixo {
    pub itens: u64,
    pub bytes: i64,
    pub velhos_itens: u64,
    pub velhos_bytes: i64,
    /// Itens que o sistema de arquivos não deu a data: ficam fora dos dois grupos de idade.
    pub sem_data_itens: u64,
    pub sem_data_bytes: i64,
    /// Algum item ficou sem tamanho (pasta negada sem dir-stats, erro de leitura ou interrompido): os bytes são um piso.
    pub piso: bool,
}

impl Lixo {
    /// `Parcial` quando há item sem data ou tamanho piso; é o que o JSON chama de `partial`.
    pub fn medida_velhos(&self) -> Medida {
        match (self.sem_data_itens, self.piso) {
            (0, false) => Medida::Valor(self.velhos_bytes),
            (n, _) if n > 0 => Medida::Parcial(self.velhos_bytes, format!("{n} itens sem data de entrada no Lixo")),
            _ => Medida::Parcial(self.velhos_bytes, "algum item sem tamanho lido".into()),
        }
    }
}

/// Mede `pasta` (o `~/.Trash`) em `agora` (segundos desde a época). Err(motivo em pt).
pub fn medir(pasta: &Path, agora: i64, threads: usize, ctl: Arc<walk::Controle>) -> Result<Lixo, String> {
    let rd = match std::fs::read_dir(pasta) {
        Ok(r) => r,
        // Sem a pasta não há Lixo: é um zero medido, não uma falha.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Lixo::default()),
        Err(e) => {
            let n = e.raw_os_error().unwrap_or(0);
            return Err(if n == libc::EPERM || n == libc::EACCES { format!("~/.Trash sem permissão (errno {n}); falta Acesso Total ao Disco") } else { format!("~/.Trash não abriu (errno {n})") });
        }
    };
    let mut entradas: Vec<(String, bool, i64)> = Vec::new();
    for e in rd.flatten() {
        let Ok(m) = e.metadata() else { continue };
        use std::os::darwin::fs::MetadataExt as _;
        entradas.push((e.file_name().to_string_lossy().into_owned(), m.is_dir(), m.st_blocks() as i64 * 512));
    }
    // Uma travessia só (sem contar duas vezes hardlink e clone entre itens): o alocado de cada pasta
    // de primeiro nível. Pasta negada com dir-stats vem com o caminho relativo.
    let ramo = detalhe::ramo(pasta, 1, threads, ctl.clone())?;
    let mut por_pasta: HashMap<&str, i64> = HashMap::new();
    for f in &ramo.filhos {
        if let Some(topo) = f.nome.split('/').next().filter(|t| !t.is_empty()) {
            *por_pasta.entry(topo).or_default() += f.alocado;
        }
    }
    let mut r = Lixo { piso: ramo.interrompida || ramo.protegidas_sem > 0 || ramo.erros > 0, ..Lixo::default() };
    for (nome, pasta_, blocos) in &entradas {
        let bytes = if *pasta_ { por_pasta.get(nome.as_str()).copied().unwrap_or(0) } else { *blocos };
        r.itens += 1;
        r.bytes += bytes;
        match sys::apfs::added_time(&pasta.join(nome)) {
            Ok(Some(t)) if agora - t > LIMITE_S => {
                r.velhos_itens += 1;
                r.velhos_bytes += bytes;
            }
            Ok(Some(_)) => {}
            _ => {
                r.sem_data_itens += 1;
                r.sem_data_bytes += bytes;
            }
        }
    }
    if ctl.cancelado() {
        return Err("interrompido".into());
    }
    Ok(r)
}

pub fn agora() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}
