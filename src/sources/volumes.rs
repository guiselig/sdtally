//! Volumes APFS (diskutil apfs list -plist) e o topo da conta (ATTR_VOL_SPACEUSED do Data).

use super::DATA;
use crate::ledger::Medida;
use crate::sys;
use serde_json::{Value, json};
use std::collections::HashMap;

pub struct Volumes {
    pub json: Value,
    /// macOS como o Ajustes calcula: Σ CapacityInUse dos volumes do disco interno − Data (§1c).
    pub macos: Medida,
    pub macos_det: Vec<Value>,
    pub disco_tamanho: Value,
}

fn whole_disk(dev: &str) -> String {
    // disk0s2 → disk0
    match dev.rfind('s') {
        Some(i) if i > 4 && dev[i + 1..].chars().all(|c| c.is_ascii_digit()) && !dev[i + 1..].is_empty() => dev[..i].to_string(),
        _ => dev.to_string(),
    }
}

pub fn coletar(err: &mut Vec<Value>) -> Volumes {
    // Disco inteiro (Internal e tamanho) pelo DiskArbitration: o `diskutil info` custava um processo
    // e um plutil por disco a cada leitura rápida.
    let mut discos: HashMap<String, Option<sys::disco::Disco>> = HashMap::new();
    let mut disco = |d: &str| *discos.entry(d.to_string()).or_insert_with(|| sys::disco::descrever(d));
    let mut r = Volumes { json: Value::Null, macos: Medida::NaoMedida(String::new()), macos_det: Vec::new(), disco_tamanho: Value::Null };
    let apfs = match sys::plist_json("/usr/sbin/diskutil", &["apfs", "list", "-plist"], 60) {
        Ok(v) => v,
        Err(e) => {
            r.macos = Medida::NaoMedida(e.clone());
            err.push(json!(e));
            return r;
        }
    };
    let data_dev = sys::dispositivo_de(DATA).unwrap_or_default();
    let mut out = Vec::new();
    let conts = apfs["Containers"].as_array().cloned().unwrap_or_default();
    let data_cont = conts
        .iter()
        .find(|c| c["Volumes"].as_array().is_some_and(|v| v.iter().any(|v| v["DeviceIdentifier"].as_str() == Some(&data_dev))));
    let data_disk = data_cont.map(|c| whole_disk(c["DesignatedPhysicalStore"].as_str().unwrap_or("")));
    if let Some(d) = &data_disk {
        r.disco_tamanho = disco(d).and_then(|x| x.tamanho).map_or(Value::Null, |t| json!(t));
    }
    let mut macos = Some(0i64);
    for c in &conts {
        let store = c["DesignatedPhysicalStore"].as_str().unwrap_or("");
        let wd = whole_disk(store);
        let interno = disco(&wd).map(|d| d.interno);
        // Container ISC (xART, Hardware, iSCPreboot): o Ajustes não mostra ("Volume should not be
        // shown in the UI") e a linha macOS da tela fica sem ele (02/10, Mac do dono: 38,73 GB =
        // fórmula − 16,58 MB do ISC). Não existe nas VMs do CI.
        let isc = c["Volumes"].as_array().is_some_and(|vs| {
            vs.iter().any(|v| v["Roles"].as_array().is_some_and(|r| r.iter().any(|x| x == "xART" || x == "Hardware")))
        });
        let mut vols = Vec::new();
        for v in c["Volumes"].as_array().cloned().unwrap_or_default() {
            let roles: Vec<String> = v["Roles"].as_array().map(|r| r.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            // Ausente fica null: somar 0 no lugar daria um macOS menor que o real.
            let used = v["CapacityInUse"].as_i64();
            let is_data = v["DeviceIdentifier"].as_str() == Some(&data_dev);
            let no_macos = Some(&wd) == data_disk.as_ref() && !isc && !roles.iter().any(|r| r == "Data") && !is_data;
            if no_macos {
                macos = macos.zip(used).map(|(a, b)| a + b);
                r.macos_det.push(json!({"container": c["ContainerReference"], "roles": roles, "capacity_in_use": used}));
            }
            vols.push(json!({
                "roles": roles,
                "sem_role": roles.is_empty(),
                "e_o_data_de_boot": is_data,
                "no_macos_da_tela": no_macos,
                "capacity_in_use": used,
                // O app lê o usado dos volumes montados por ele (getattrlist) e acha cada um por aqui.
                "dispositivo": v["DeviceIdentifier"],
                "criptografado": v["Encryption"].as_bool(),
                "filevault": v["FileVault"].as_bool(),
            }));
        }
        out.push(json!({
            "container": c["ContainerReference"],
            "disco_fisico": wd,
            "isc_fora_do_macos": isc,
            "tipo": match interno { Some(true) => "interno", Some(false) => "externo", None => "desconhecido" },
            "tamanho": c["CapacityCeiling"],
            "livre": c["CapacityFree"],
            "volumes": vols,
        }));
    }
    r.macos = match (data_disk.is_some(), macos) {
        (true, Some(m)) => Medida::Valor(m),
        (true, None) => Medida::NaoMedida("CapacityInUse ausente em volume do disco interno".into()),
        (false, _) => {
            err.push(json!("container do volume Data não encontrado"));
            Medida::NaoMedida("container do volume Data não encontrado".into())
        }
    };
    r.json = Value::Array(out);
    r
}

/// (usado do Data, JSON). O usado vem de ATTR_VOL_SPACEUSED; statfs dá o container.
pub fn volume_data(err: &mut Vec<Value>) -> (Medida, Value) {
    let usado = sys::apfs::vol_space_used(DATA).map(|x| x as i64);
    let sf = sys::statfs(DATA);
    if let Err(e) = usado {
        err.push(json!(format!("getattrlist ATTR_VOL_SPACEUSED falhou (errno {e})")));
    }
    let j = json!({
        "usado_attr_vol_spaceused": usado.ok(),
        "container_total_statfs": sf.as_ref().ok().map(|x| x.0),
        "container_livre_statfs": sf.as_ref().ok().map(|x| x.1),
        "fonte": "getattrlist(ATTR_VOL_SPACEUSED) no ponto de montagem; statfs dá o container",
    });
    (usado.map_or_else(|e| Medida::NaoMedida(format!("ATTR_VOL_SPACEUSED errno {e}")), Medida::Valor), j)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O DiskArbitration dá o mesmo Internal e TotalSize que o `diskutil info` do disco do Data
    /// (a leitura rápida trocou o `diskutil info` por ele).
    #[test]
    fn disco_do_data_igual_ao_diskutil_info() {
        let dev = sys::dispositivo_de(DATA).expect("statfs do Data");
        let inteiro = whole_disk(&dev);
        let d = sys::disco::descrever(&inteiro).expect("DiskArbitration");
        let u = sys::plist_json("/usr/sbin/diskutil", &["info", "-plist", &inteiro], 30).expect("diskutil info");
        assert_eq!(Some(d.interno), u["Internal"].as_bool(), "{inteiro}");
        assert_eq!(d.tamanho, u["TotalSize"].as_i64(), "{inteiro}");
    }

    #[test]
    fn disco_inexistente_e_none() {
        assert_eq!(sys::disco::descrever("disk999999"), None);
        assert_eq!(sys::disco::descrever("com\0nul"), None);
    }

    #[test]
    fn disco_inteiro_do_dispositivo() {
        assert_eq!(whole_disk("disk0s2"), "disk0");
        assert_eq!(whole_disk("disk12"), "disk12");
    }
}
