//! As 15 categorias do Ajustes > Armazenamento, com as regras da Frente P (lidas nos binários).
//! Cada função devolve a medida e um detalhe em JSON (só números e caminhos fixos; nada pessoal).

use crate::ffi::{self, Id, NIL, Pool, msg, tam};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::c_void;
use std::os::unix::fs::MetadataExt;
use std::sync::Mutex;

pub enum M {
    Valor(i64),
    /// Piso: só a parte medida; o texto diz o que ficou de fora.
    Parcial(i64, String),
    NaoMedida(String),
}

impl M {
    pub fn bytes(&self) -> Option<i64> {
        match self {
            M::Valor(v) | M::Parcial(v, _) => Some(*v),
            M::NaoMedida(_) => None,
        }
    }
    pub fn estado(&self) -> String {
        match self {
            M::Valor(_) => "medida".into(),
            M::Parcial(_, m) => format!("parcial ({m})"),
            M::NaoMedida(m) => format!("não medida ({m})"),
        }
    }
}

pub type R = Result<(M, Value), String>;

fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}
fn h(rel: &str) -> String {
    format!("{}/{rel}", home())
}
fn soma(ps: &[String]) -> i64 {
    ps.iter().filter_map(|p| tam(p, &[])).filter(|v| *v > 0).sum()
}
/// Filhos de uma pasta (sem "." e ".."); `pula_ponto` tira os ocultos. Pasta ilegível = vazio,
/// como o contentsOfDirectoryAtPath da Apple.
fn filhos(d: &str, pula_ponto: bool) -> Vec<String> {
    std::fs::read_dir(d).into_iter().flatten().flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !(pula_ponto && n.starts_with('.')))
        .map(|n| format!("{d}/{n}"))
        .collect()
}
fn vol(p: &str) -> Id {
    ffi::res(ffi::url(p), "NSURLVolumeIdentifierKey")
}

// ---- caminhos fixos ----

pub fn developer() -> R {
    ffi::stm()?;
    let mut ps: Vec<String> = ["Library/Developer/CoreSimulator/Caches", "Library/Developer/Xcode/DerivedData",
        "Library/Developer/Xcode/Installs", "Library/Developer/Xcode/Archives"].iter().map(|r| h(r)).collect();
    ps.push("/Library/Developer/CommandLineTools".into());
    // "Simulator Support" (/Library/Developer/CoreSimulator inteiro) só existe no binário do 27;
    // no 15 e no 26 o log = CLT ao byte mesmo com 38–55 GB em CoreSimulator/Caches (CI, 01/10).
    let maior: u32 = ffi::sysctl_str("kern.osproductversion").and_then(|v| v.split('.').next()?.parse().ok()).unwrap_or(0);
    if maior >= 27 {
        ps.push("/Library/Developer/CoreSimulator".into());
    }
    for os in ["iOS", "watchOS", "tvOS", "macOS", "visionOS"] {
        ps.extend(filhos(&h(&format!("Library/Developer/Xcode/{os} DeviceSupport")), true));
    }
    ps.extend(filhos("/AppleInternal/Developer/KDKs", true));
    // Fora do painel (sandbox da extensão), só informativo.
    let xc = tam(&h("Library/Caches/com.apple.dt.Xcode"), &[]);
    // Por item e, em CoreSimulator, por filho: nos runners 15/26 o painel deu ~CLT só e o frenteq
    // +38–55 GB (01/10); suspeita: runtimes montados em CoreSimulator/Volumes.
    let por: BTreeMap<String, i64> = ps.iter().filter_map(|p| tam(p, &[]).filter(|b| *b > 0).map(|b| (p.clone(), b))).collect();
    let cs: BTreeMap<String, Option<i64>> = filhos("/Library/Developer/CoreSimulator", false).into_iter()
        .map(|p| { let b = tam(&p, &[]); (p, b) }).collect();
    Ok((M::Valor(por.values().sum()), json!({"itens": ps.len(), "por_item": por, "coresimulator": cs, "fora_do_painel_caches_xcode": xc})))
}

pub fn otherusers() -> R {
    ffi::stm()?;
    let t = soma(&["/Users/Shared/".into(), "/Users/Deleted Users/".into()]);
    let hm = home();
    let outros: Vec<String> = filhos("/Users", true).into_iter()
        .filter(|u| *u != hm && !["/Users/Shared", "/Users/Deleted Users", "/Users/Guest"].contains(&u.as_str()))
        .collect();
    let m = if outros.is_empty() {
        M::Valor(t)
    } else {
        M::Parcial(t, format!("{} outra(s) home(s) exigem root (SMUtilities_Client)", outros.len()))
    };
    Ok((m, json!({"outras_homes": outros.len()})))
}

pub fn trash() -> R {
    ffi::stm()?;
    let _p = Pool::new();
    let hv = vol(&home());
    let mut t = 0;
    for r in [h(".Trash"), format!("/.Trashes/{}", unsafe { libc::getuid() })] {
        for p in filhos(&r, true) {
            if let Some(b) = tam(&p, &[]).filter(|b| *b > 0)
                && ffi::iguais(vol(&p), hv)
            {
                t += b;
            }
        }
    }
    Ok((M::Valor(t), Value::Null))
}

pub fn ios() -> R {
    ffi::stm()?;
    let mut ps: Vec<String> = filhos(&h("Library/Application Support/MobileSync/Backup"), true)
        .into_iter().filter(|p| !p.ends_with("/Logs")).collect();
    let mut dirs: Vec<String> = ["K36BKF7T3D", "4WXS7A4F54"].iter()
        .map(|g| h(&format!("Library/Group Containers/{g}.group.com.apple.configurator/Library/Caches/Firmware"))).collect();
    dirs.extend(["iPhone", "iPad", "iPod", "Apple TV"].iter().map(|d| h(&format!("Library/iTunes/{d} Software Updates"))));
    for d in dirs {
        ps.extend(filhos(&d, true).into_iter().filter(|p| p.ends_with(".ipsw")));
    }
    Ok((M::Valor(soma(&ps)), json!({"itens": ps.len()})))
}

pub fn garageband() -> R {
    ffi::stm()?;
    let mut ps = vec!["/Library/Application Support/GarageBand/Instrument Library".to_string()];
    for l in ["Sampler Instruments", "EXS Factory Samples", "Alchemy Samples", "Ultrabeat Samples", "Channel Strip Settings", "Plug-In Settings"] {
        ps.push(format!("/Library/Application Support/Logic/{l}"));
    }
    ps.extend(["/Library/Audio/Impulse Responses/Apple/".into(), "/Library/Audio/Apple Loops/Apple".into(),
               "/Library/Application Support/GarageBand/Learn to Play/Basic Lessons".into()]);
    Ok((M::Valor(soma(&ps)), Value::Null))
}

/// ~/Library/Mail − purgeable do com.apple.MailCacheDelete (CacheDelete, só consulta, urgência 3).
pub fn mail() -> R {
    ffi::stm()?;
    let pasta = tam(&h("Library/Mail"), &[]);
    let _p = Pool::new();
    let cd = ffi::dlopen("/System/Library/PrivateFrameworks/CacheDelete.framework/Versions/A/CacheDelete")
        .and_then(|x| ffi::dlsym(x, "CacheDeleteCopyItemizedPurgeableSpaceWithInfo"))
        .ok_or("CacheDelete: símbolo ausente")?;
    let f: unsafe extern "C" fn(Id) -> Id = unsafe { std::mem::transmute(cd) };
    let q = msg!(Id; ffi::cls("NSMutableDictionary"), "dictionary");
    let num = |v: i64| msg!(Id; ffi::cls("NSNumber"), "numberWithLongLong:", v => i64);
    let put = |k: &str, v: Id| msg!((); q, "setObject:forKey:", v => Id, ffi::ns(k) => Id);
    put("CACHE_DELETE_VOLUME", ffi::ns(&home()));
    put("CACHE_DELETE_SERVICES", ffi::nsarray(&[ffi::ns("com.apple.MailCacheDelete")]));
    put("CACHE_DELETE_URGENCY", num(3));
    put("CACHE_DELETE_NO_CACHE", msg!(Id; ffi::cls("NSNumber"), "numberWithBool:", false => bool));
    let r = unsafe { f(q) };
    let pg = ffi::i64v(ffi::get(r, "com.apple.MailCacheDelete"));
    if !r.is_null() {
        msg!((); r, "release");
    }
    let (Some(pasta), Some(pg)) = (pasta, pg) else {
        return Ok((M::NaoMedida("pasta ausente ou CacheDelete sem resposta".into()), Value::Null));
    };
    let d = json!({"pasta": pasta, "purgeable_mail": pg});
    // Negativo = "desconhecido" na extensão.
    Ok((if pasta - pg < 0 { M::NaoMedida("negativo: desconhecido".into()) } else { M::Valor(pasta - pg) }, d))
}

// ---- Aplicativos e Documentos ----

/// Apps que a consulta do Spotlight achou sob a home (Documentos os pula). Guardado da passagem
/// de Aplicativos.
static APPS_HOME: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// Mesma consulta do NSMetadataQuery da extensão, pela API C (MDQuery síncrono).
fn spotlight(escopos: &[&str]) -> Result<Vec<String>, String> {
    type Create = unsafe extern "C" fn(*const c_void, Id, Id, Id) -> Id;
    type Scope = unsafe extern "C" fn(Id, Id, u32);
    type Exec = unsafe extern "C" fn(Id, usize) -> bool;
    type Count = unsafe extern "C" fn(Id) -> isize;
    type At = unsafe extern "C" fn(Id, isize) -> Id;
    type Attr = unsafe extern "C" fn(Id, Id) -> Id;
    let s = |n: &str| ffi::dlsym(libc::RTLD_DEFAULT, n).ok_or(format!("{n} ausente"));
    let (create, scope, exec, count, at, attr): (Create, Scope, Exec, Count, At, Attr) = unsafe {
        (std::mem::transmute(s("MDQueryCreate")?), std::mem::transmute(s("MDQuerySetSearchScope")?),
         std::mem::transmute(s("MDQueryExecute")?), std::mem::transmute(s("MDQueryGetResultCount")?),
         std::mem::transmute(s("MDQueryGetResultAtIndex")?), std::mem::transmute(s("MDItemCopyAttribute")?))
    };
    let q = unsafe { create(std::ptr::null(), ffi::ns("kMDItemContentType == 'com.apple.application-bundle'"), NIL, NIL) };
    if q.is_null() {
        return Err("MDQueryCreate falhou".into());
    }
    let esc: Vec<Id> = escopos.iter().map(|e| ffi::ns(e)).collect();
    unsafe { scope(q, ffi::nsarray(&esc), 0) };
    // kMDQuerySynchronous = 1
    if !unsafe { exec(q, 1) } {
        msg!((); q, "release");
        return Err("MDQueryExecute falhou".into());
    }
    let kp = ffi::konst("kMDItemPath");
    let mut r = Vec::new();
    for i in 0..unsafe { count(q) } {
        let p = unsafe { attr(at(q, i), kp) };
        if let Some(s) = ffi::rs(p) {
            r.push(s);
        }
        if !p.is_null() {
            msg!((); p, "release");
        }
    }
    msg!((); q, "release");
    Ok(r)
}

/// bundle id → ~/Library/Containers/<x>/Data, pelo plist do containermanagerd.
fn mapa_containers() -> HashMap<String, String> {
    let mut m = HashMap::new();
    for c in filhos(&h("Library/Containers"), true) {
        let _p = Pool::new();
        let d = msg!(Id; ffi::cls("NSDictionary"), "dictionaryWithContentsOfFile:",
                     ffi::ns(&format!("{c}/.com.apple.containermanagerd.metadata.plist")) => Id);
        if let Some(id) = ffi::rs(ffi::get(d, "MCMMetadataIdentifier")) {
            m.insert(id, format!("{c}/Data"));
        }
    }
    m
}

/// _FolderThatContainsInApplications: tira "Applications" e "System" dos componentes; com 2+
/// sobrando, "/Applications/<primeiro>".
fn pasta_em_applications(p: &str) -> Option<String> {
    let c: Vec<&str> = p.split('/').filter(|x| !x.is_empty() && *x != "Applications" && *x != "System").collect();
    (c.len() >= 2).then(|| format!("/Applications/{}", c[0]))
}

struct Item {
    p: String,
    nome: String,
    bid: String,
    a: i64,
    d: i64,
    vivo: bool,
}

pub fn apps() -> R {
    ffi::stm()?;
    let _pool = Pool::new();
    let hm = home();
    let fora: HashSet<&str> = ["com.apple.Music", "com.apple.iBooksX", "com.apple.mail", "com.apple.TV",
        "com.apple.podcasts", "com.apple.Photos", "com.apple.MobileSMS"].into();
    let pasta_pai: HashSet<&str> = ["net.maxon.cinema4d", "com.sidefx.HoudiniFX"].into(); // USE_PARENT_FOLDER_SIZE
    let todos = spotlight(&["/Applications/", "/System/Applications", &hm])?;
    let cont = mapa_containers();
    let (lib, lixo) = (format!("{hm}/Library/"), format!("{hm}/.Trash/"));
    let mut itens: Vec<Item> = Vec::new();
    let mut grupos: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let (mut em_home, mut pulados, mut restritos) = (Vec::new(), 0, 0i64);
    // FRENTEQ_POR_ITEM=1 (só no CI: nomes de apps): cada app medido ou pulado e por quê.
    let por_item = std::env::var_os("FRENTEQ_POR_ITEM").is_some();
    let mut lista: Vec<Value> = Vec::new();
    let mut pula = |v: Value| if por_item { lista.push(v) };
    for p in &todos {
        let _p = Pool::new();
        // -[STMApplicationSizer shouldIgnoreItemAtPath:]; stat segue symlink, como _PathIsRestricted.
        // 0x80000 = SF_RESTRICTED (sys/stat.h), ausente no crate libc.
        let restrito = ffi::stat(p).is_some_and(|s| s.st_flags & 0x0008_0000 != 0);
        let pasta = pasta_em_applications(p);
        if (restrito && pasta.is_some()) || p.starts_with(&lixo) || p.starts_with("/.Trashes/") || p.starts_with(&lib) {
            pula(json!([p, "restrito em /Applications, lixo ou ~/Library"]));
            pulados += 1;
            continue;
        }
        let b = msg!(Id; ffi::cls("NSBundle"), "bundleWithPath:", ffi::ns(p) => Id);
        let bid = if b.is_null() { None } else { ffi::rs(msg!(Id; b, "bundleIdentifier")) };
        if bid.as_deref().is_some_and(|x| fora.contains(x)) {
            pula(json!([p, "categoria própria"]));
            pulados += 1;
            continue;
        }
        // _IsNestedAppPath: algum ancestral .app que é aplicação
        let mut q = p.as_str();
        let mut aninhado = false;
        while let Some(i) = q.rfind('/').filter(|i| *i > 0) {
            q = &p[..i];
            if q.ends_with(".app") && ffi::i64v(ffi::res(ffi::url(q), "NSURLIsApplicationKey")).unwrap_or(0) != 0 {
                aninhado = true;
            }
        }
        if aninhado {
            pula(json!([p, "aninhado"]));
            pulados += 1;
            continue;
        }
        let bid = bid.unwrap_or_default();
        let dc = if bid.starts_with("com.apple.Safari.WebApp.") {
            Some(h(&format!("Library/Containers/com.apple.Safari.WebApp/Data/Library/Containers/{bid}")))
        } else {
            cont.get(&bid).cloned()
        }
        .filter(|d| ffi::existe(d));
        // Restrito sem container sai da lista, mas o bundle entra no total (Frente P §2.1).
        if dc.is_none() && restrito {
            let b = tam(p, &[]).unwrap_or(0).max(0);
            restritos += b;
            pula(json!([p, format!("restrito sem container: {b} no total")]));
            pulados += 1;
            continue;
        }
        let nome = p.rsplit('/').next().unwrap_or("").trim_end_matches(".app").to_string();
        let a = tam(p, &[]).unwrap_or(0).max(0);
        let d = dc.as_deref().and_then(|d| tam(d, &[])).unwrap_or(0).max(0);
        if let Some(pa) = pasta {
            grupos.entry(pa).or_default().push(itens.len());
        }
        if p.starts_with(&format!("{hm}/")) {
            em_home.push(p.clone());
        }
        itens.push(Item { p: p.clone(), nome, bid, a, d, vivo: true });
    }
    // Pasta em /Applications com o nome do app (ou override): o item vira a pasta inteira e os
    // outros apps da pasta saem.
    for (pasta, ix) in &grupos {
        let ult = pasta.rsplit('/').next().unwrap_or("");
        let Some(&dono) = ix.iter().find(|&&i| itens[i].nome == ult || pasta_pai.contains(itens[i].bid.as_str())) else { continue };
        itens[dono].a = tam(pasta, &[]).unwrap_or(0).max(0);
        itens[dono].p = pasta.clone();
        for &i in ix {
            if i != dono {
                itens[i].vivo = false;
            }
        }
    }
    let hv = vol(&hm);
    let (mut sa, mut sd, mut n) = (0i64, 0i64, 0);
    for it in itens.iter().filter(|i| i.vivo) {
        let _p = Pool::new();
        if !ffi::iguais(vol(&it.p), hv) {
            pula(json!([it.p, "outro volume"]));
            pulados += 1;
            continue;
        }
        pula(json!([it.p, it.a, it.d]));
        n += 1;
        sa += it.a;
        sd += it.d;
    }
    let asup = tam(&h("Library/Application Support"), &[]).unwrap_or(0).max(0);
    let ms = tam(&h("Library/Application Support/MobileSync"), &[]).unwrap_or(0).max(0);
    *APPS_HOME.lock().unwrap() = Some(em_home);
    let mut d = json!({"spotlight": todos.len(), "itens": n, "pulados": pulados, "bundles": sa, "containers": sd,
                   "restritos_sem_container": restritos, "application_support": asup, "mobilesync_subtraido": ms});
    if por_item {
        d["por_item"] = Value::Array(lista);
    }
    Ok((M::Valor(sa + sd + restritos + asup - ms), d))
}

/// "Mesa e Documentos" do iCloud ligado: medido pela chave NSURLIsUbiquitousItemKey das pastas
/// (a Frente P assumia ligado pelo log de 28/09).
fn mesa_docs_icloud() -> (bool, bool) {
    let _p = Pool::new();
    let u = |r: &str| ffi::i64v(ffi::res(ffi::url(&h(r)), "NSURLIsUbiquitousItemKey")).unwrap_or(0) != 0;
    (u("Desktop"), u("Documents"))
}

pub fn documents() -> R {
    ffi::stm()?;
    if APPS_HOME.lock().unwrap().is_none() {
        apps()?;
    }
    let apps_home = APPS_HOME.lock().unwrap().clone().unwrap_or_default();
    let mut ex: Vec<String> = ["Applications", "Library", "Pictures/Photos Library.photoslibrary", "Music/iTunes/iTunes Media",
        "Music/Music/Media", "Movies/Apple TV/Media", "Movies/TV/Media", "Movies/TV/TV Library",
        // _APPPATHS_ de Música, TV e Lixo que caem na home
        "Music/Music/Music Library.musiclibrary", "Music/Music/Previous Libraries.localized", "Music/Music/Media.localized",
        "Movies/TV/Media.localized", "Movies/TV/Previous Libraries.localized", "Movies/TV/TV Library.tvlibrary", ".Trash"]
        .iter().map(|r| h(r)).collect();
    let (desk, docs) = mesa_docs_icloud();
    if desk {
        ex.push(h("Desktop"));
    }
    if docs {
        ex.push(h("Documents"));
    }
    for a in apps_home {
        if !ex.iter().any(|e| a.starts_with(&format!("{e}/"))) {
            ex.push(a);
        }
    }
    let vivos: Vec<String> = ex.into_iter().filter(|e| ffi::existe(e)).collect();
    let v = tam(&home(), &vivos).ok_or("home ausente")?;
    Ok((M::Valor(v), json!({"excluidos": vivos.len(), "desktop_icloud": desk, "documents_icloud": docs})))
}

// ---- bibliotecas de mídia ----

const CHAVE_MUSICDB: &[u8; 16] = b"BHUILuilfghuila3";

fn u32le(d: &[u8], o: usize) -> Option<usize> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?) as usize)
}

/// Library.musicdb/.tvdb: "hfma" + AES-128-ECB nos primeiros max_crypt bytes + zlib.
fn musicdb(path: &str) -> Result<Vec<u8>, String> {
    let d = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if d.get(..4) != Some(b"hfma") {
        return Err("não é hfma".into());
    }
    let (hl, mc) = (u32le(&d, 4).ok_or("cabeçalho")?, u32le(&d, 0x54).ok_or("cabeçalho")?);
    let body = d.get(hl..).ok_or("cabeçalho")?;
    let n = mc.min(body.len()) / 16 * 16;
    let mut z = ffi::aes128_ecb_dec(CHAVE_MUSICDB, &body[..n])?;
    z.extend_from_slice(&body[n..]);
    ffi::inflate(&z)
}

/// (FileSize, caminho local) de cada registro "itma"; porte direto do musica.py.
fn faixas(d: &[u8]) -> Result<Vec<(Option<u64>, Option<String>)>, String> {
    let mut out: Vec<(Option<u64>, Option<String>)> = Vec::new();
    let mut cur = false;
    let mut i = 0;
    while i + 8 < d.len() {
        let tag = &d[i..i + 4];
        let l1 = u32le(d, i + 4).ok_or("fim")?;
        if tag == b"hsma" {
            i += l1;
            continue;
        }
        if tag == b"boma" {
            let tot = u32le(d, i + 8).ok_or("fim")?;
            let st = u32le(d, i + 12).ok_or("fim")?;
            if cur && let Some(c) = out.last_mut() {
                if st == 1 && c.0.is_none() && tot >= 324 {
                    c.0 = Some(u64::from_le_bytes(d.get(i + 316..i + 324).ok_or("fim")?.try_into().unwrap()));
                } else if st == 11 {
                    let raw = d.get(i + 36..i + tot).ok_or("fim")?;
                    let u = String::from_utf8_lossy(raw.split(|b| *b == 0).next().unwrap_or(&[])).into_owned();
                    c.1 = u.starts_with("file://").then(|| unquote(&u)[7..].trim_end_matches('/').to_string());
                }
            }
            if tot == 0 {
                return Err(format!("boma vazio em {i}"));
            }
            i += tot;
            continue;
        }
        cur = tag == b"itma";
        if cur {
            out.push((None, None));
        }
        if l1 < 8 {
            return Err(format!("registro inválido em {i}"));
        }
        i += l1;
    }
    Ok(out)
}

fn unquote(s: &str) -> String {
    let b = s.as_bytes();
    let mut o = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(v) = s.get(i + 1..i + 3).and_then(|x| u8::from_str_radix(x, 16).ok())
        {
            o.push(v);
            i += 3;
            continue;
        }
        o.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&o).into_owned()
}

/// Música/TV: Σ FileSize gravado na biblioteca, só faixas com arquivo local no volume da home.
pub fn midia(tv: bool) -> R {
    let db = if tv { h("Movies/TV/TV Library.tvlibrary/Library.tvdb") } else { h("Music/Music/Music Library.musiclibrary/Library.musicdb") };
    if !ffi::existe(&db) {
        // Pacote da biblioteca sem o banco: formato mudou (outra versão), não é 0.
        if db.rsplit_once('/').is_some_and(|(pasta, _)| ffi::existe(pasta)) {
            return Ok((M::NaoMedida("biblioteca sem o banco esperado".into()), Value::Null));
        }
        // Sem biblioteca o app não tem o que contar: é 0 medido, não falta de medida.
        return Ok((M::Valor(0), json!({"biblioteca": "ausente"})));
    }
    let home_dev = std::fs::metadata(home()).map_err(|e| e.to_string())?.dev();
    let (mut t, mut ok, mut sem, mut aus, mut outro) = (0u64, 0, 0, 0, 0);
    for (sz, p) in faixas(&musicdb(&db)?)? {
        let Some(p) = p else {
            sem += 1;
            continue;
        };
        match std::fs::metadata(&p) {
            Err(_) => aus += 1,
            Ok(m) if m.dev() != home_dev => outro += 1,
            Ok(_) => {
                ok += 1;
                t += sz.unwrap_or(0);
            }
        }
    }
    Ok((M::Valor(t as i64), json!({"faixas": ok, "sem_local": sem, "ausentes": aus, "outro_volume": outro})))
}

static FOTOS: Mutex<Option<[u64; 5]>> = Mutex::new(None);

extern "C" fn fotos_cb(_b: *mut c_void, a: u64, b: u64, c: u64, d: u64, e: Id) {
    *FOTOS.lock().unwrap() = Some([a, b, c, d, !e.is_null() as u64]);
}

/// +[PLFileUtilities calculateTotalSizeOfFilesAtPath:calculatePurgeable:YES allocatedSize:NO
/// result:] (PhotoLibraryServicesCore, privado); a extensão mostra total − purgeable (r2 − r4).
pub fn fotos() -> R {
    let lib = h("Pictures/Photos Library.photoslibrary");
    if !ffi::existe(&lib) {
        return Ok((M::Valor(0), json!({"biblioteca": "ausente"})));
    }
    ffi::dlopen("/System/Library/PrivateFrameworks/PhotoLibraryServicesCore.framework/Versions/A/PhotoLibraryServicesCore")
        .ok_or("dlopen PhotoLibraryServicesCore falhou")?;
    let c = ffi::cls("PLFileUtilities");
    if c.is_null() {
        return Err("classe PLFileUtilities ausente".into());
    }
    let _p = Pool::new();
    *FOTOS.lock().unwrap() = None;
    let b = ffi::bloco(fotos_cb as *const c_void);
    // O método é void (encoding v40@0:8@16B24B28@?32 no 27): o resultado e o NSError vêm só no bloco.
    msg!((); c, "calculateTotalSizeOfFilesAtPath:calculatePurgeable:allocatedSize:result:",
                  ffi::ns(&lib) => Id, true => bool, false => bool, &b as *const ffi::Bloco => *const ffi::Bloco);
    let r = FOTOS.lock().unwrap().take().ok_or("bloco não foi chamado")?;
    if r[4] != 0 {
        return Err("PLFileUtilities devolveu NSError".into());
    }
    Ok((M::Valor(r[1] as i64 - r[3] as i64), json!({"r1": r[0], "r2": r[1], "r3": r[2], "r4": r[3]})))
}

/// Livros: o BKAgent recusa quem não é o Books ("Couldn't communicate with a helper", 29/09),
/// então vem direto do banco que ele mantém, no lugar e só leitura. Filtro e max do binário.
pub fn livros() -> R {
    // O sufixo do nome muda com o esquema: pega o BKLibrary-*.sqlite que houver.
    let pasta = h("Library/Containers/com.apple.iBooksX/Data/Documents/BKLibrary");
    let Some(db) = filhos(&pasta, true).into_iter().find(|p| p.rsplit('/').next().is_some_and(|n| n.starts_with("BKLibrary-") && n.ends_with(".sqlite"))) else {
        if ffi::existe(&pasta) {
            return Ok((M::NaoMedida("pasta BKLibrary sem banco".into()), Value::Null));
        }
        return Ok((M::Valor(0), json!({"banco": "ausente"})));
    };
    let rows = ffi::sql(&db, "SELECT ZPATH, ZISSAMPLE, ZTITLE, ZASSETID, ZSTOREID, ZFILESIZE FROM ZBKLIBRARYASSET")?;
    let _p = Pool::new();
    let fm = msg!(Id; ffi::cls("NSFileManager"), "defaultManager");
    type Create = unsafe extern "C" fn(*const c_void, Id) -> Id;
    type Attr = unsafe extern "C" fn(Id, Id) -> Id;
    let create: Create = unsafe { std::mem::transmute(ffi::dlsym(libc::RTLD_DEFAULT, "MDItemCreate").ok_or("MDItemCreate")?) };
    let attr: Attr = unsafe { std::mem::transmute(ffi::dlsym(libc::RTLD_DEFAULT, "MDItemCopyAttribute").ok_or("MDItemCopyAttribute")?) };
    let (mut t, mut n, mut fora) = (0i64, 0, 0);
    for r in &rows {
        let (path, name) = (r[0].t(), r[2].t());
        let fora_motivo = path.is_empty()
            || r[1].i().unwrap_or(0) != 0
            || name.is_empty()
            || (r[3].t().is_empty() && r[4].t().is_empty())
            || msg!(bool; fm, "isUbiquitousItemAtURL:", ffi::url(&path) => Id);
        if fora_motivo {
            fora += 1;
            continue;
        }
        let mut sz = r[5].i().unwrap_or(0);
        let it = unsafe { create(std::ptr::null(), ffi::ns(&path)) };
        if !it.is_null() {
            let fs = unsafe { attr(it, ffi::konst("kMDItemFSSize")) };
            sz = sz.max(ffi::i64v(fs).unwrap_or(0));
            if !fs.is_null() {
                msg!((); fs, "release");
            }
            msg!((); it, "release");
        }
        n += 1;
        t += sz;
    }
    Ok((M::Valor(t), json!({"contados": n, "descartados": fora, "fonte_lista": "BKLibrary (BKAgent recusa)"})))
}

pub fn podcasts() -> R {
    let g = h("Library/Group Containers/243LU875E5.groups.com.apple.podcasts");
    let db = format!("{g}/Documents/MTLibrary.sqlite");
    let rows = match ffi::sql(&db, "SELECT m.ZLOCALURL FROM ZMTEPISODE e JOIN ZMTMEDIAENCLOSURE m ON m.Z_PK = e.ZCURRENTMEDIAENCLOSURE \
                                    WHERE m.ZLOCALURL IS NOT NULL AND m.ZLOCALURL > ''") {
        Ok(r) => r,
        // Banco que não abre: a extensão usa o tamanho do cache do grupo.
        Err(e) => {
            ffi::stm()?;
            return Ok((M::Valor(tam(&format!("{g}/Library/Cache"), &[]).unwrap_or(0)), json!({"fallback_cache": e})));
        }
    };
    ffi::stm()?;
    let (mut t, mut n) = (0, 0);
    for r in &rows {
        let u = r[0].t();
        let p = match u.strip_prefix("file://") {
            Some(x) => unquote(&x[x.find('/').unwrap_or(0)..]),
            None => u.clone(),
        };
        if let Some(v) = tam(&p, &[]) {
            n += 1;
            t += v;
        }
    }
    Ok((M::Valor(t), json!({"com_localurl": rows.len(), "com_arquivo": n})))
}

pub fn messages() -> R {
    let rows = ffi::sql(&h("Library/Messages/chat.db"),
                        "SELECT SUM(total_bytes) FROM attachment WHERE hide_attachment=0 AND transfer_state = 5")?;
    // SUM sem linhas = NULL = nenhum anexo baixado: 0 medido.
    Ok((M::Valor(rows.first().and_then(|r| r[0].i()).unwrap_or(0)), Value::Null))
}

// ---- iCloud Drive ----

/// Soma alocada (st_blocks × 512) dos arquivos sob `raiz`, sem cruzar volume, sem descer em
/// dataless, repetindo em EINTR; `f` é chamada em cada entrada (para o graveyard).
fn andar(raiz: &str, f: &mut dyn FnMut(&str)) -> i64 {
    let Some(r) = ffi::lstat(raiz) else { return 0 };
    let dev = r.st_dev;
    let (mut t, mut pilha) = (0i64, vec![raiz.to_string()]);
    while let Some(d) = pilha.pop() {
        let rd = loop {
            match std::fs::read_dir(&d) {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                x => break x,
            }
        };
        for e in rd.into_iter().flatten().flatten() {
            let p = format!("{d}/{}", e.file_name().to_string_lossy());
            f(&p);
            let Some(st) = ffi::lstat(&p) else { continue };
            if st.st_dev != dev {
                continue;
            }
            match st.st_mode & libc::S_IFMT {
                libc::S_IFDIR if st.st_flags & 0x4000_0000 == 0 => pilha.push(p), // SF_DATALESS
                libc::S_IFREG => t += st.st_blocks * 512,
                _ => {}
            }
        }
    }
    t
}

/// A (FPFS não despejável, fileproviderd) + B (bird). A é medido; a máscara "= 0" e a fórmula de B
/// são grau B na Frente P, então a categoria sai parcial com A e B vai no detalhe.
pub fn icloud() -> R {
    // Sem nenhum banco FPFS não há iCloud Drive: a tela não mostra a categoria e o
    // fileproviderctl só estoura o tempo (runners do CI, 01/10).
    if !filhos(&h("Library/Application Support/FileProvider"), true).iter().any(|d| std::path::Path::new(&format!("{d}/database/db")).exists()) {
        return Ok((M::Valor(0), json!({"sem_banco_fpfs": true})));
    }
    let out =ffi::run("/usr/bin/fileproviderctl", &["dump", "-l", "com.apple.CloudDocs.iCloudDriveFileProvider"], 120)?;
    let out = String::from_utf8_lossy(&out);
    let uuid = out.lines().find(|l| l.contains("+ database:") && l.contains("UUID="))
        .and_then(|l| l.split("UUID=").nth(1)?.split_whitespace().next().map(str::to_string))
        .ok_or("UUID do banco FPFS não achado")?;
    let mut a = None;
    for dir in filhos(&h("Library/Application Support/FileProvider"), true) {
        let db = format!("{dir}/database/db");
        let Ok(u) = ffi::sql(&db, "SELECT db_uuid FROM engine_state") else { continue };
        if u.first().map(|r| r[0].t().to_uppercase()) != Some(uuid.to_uppercase()) {
            continue;
        }
        let r = ffi::sql(&db, "SELECT SUM(metadata_physical_size) FROM FS_snapshot WHERE metadata_is_evictable = 0 \
                               AND NOT (metadata_is_dataless AND metadata_is_package)")?;
        a = Some(r.first().and_then(|r| r[0].i()).unwrap_or(0));
        break;
    }
    let a = a.ok_or(format!("banco FPFS {uuid} não achado"))?;
    let ses = h("Library/Application Support/CloudDocs/session");
    let b = ffi::dirstats(&ses).and_then(|tot| {
        let mut g = 0i64;
        andar(&ses, &mut |p| {
            if let Some(v) = ffi::getxattr(p, "com.apple.bird.graveyard.size#N") {
                let s = String::from_utf8_lossy(&v);
                g += s.trim().parse::<i64>().unwrap_or_else(|_| {
                    let mut b8 = [0u8; 8];
                    b8[..v.len().min(8)].copy_from_slice(&v[..v.len().min(8)]);
                    i64::from_le_bytes(b8)
                });
            }
        });
        let (x, u) = (andar(&format!("{ses}/x"), &mut |_| {}), andar(&format!("{ses}/u"), &mut |_| {}));
        let fl = |n: &str| {
            let db = format!("{ses}/db/{n}");
            let c = ffi::sql(&db, "PRAGMA freelist_count").ok()?.first()?[0].i()?;
            let s = ffi::sql(&db, "PRAGMA page_size").ok()?.first()?[0].i()?;
            Some(c * s)
        };
        // Banco que não abre = B não medido (antes virava freelist 0).
        let fl = (fl("client.db")? + fl("server.db")? - (1 << 20)).max(0);
        Some(json!({"b": (tot - (g + x + u + fl)).max(0), "dirstats": tot, "graveyard": g, "x": x, "u": u, "freelist": fl}))
    });
    let det = json!({"a_fpfs_nao_despejavel": a, "b_bird_hipotese": b, "a_mais_b": b.as_ref().and_then(|v| v["b"].as_i64()).map(|v| v + a)});
    Ok((M::Parcial(a, "só a parte A (FPFS); a parte B do bird é hipótese da Frente P e fica no detalhe".into()), det))
}

// ---- volume ----

/// purgeable = ImportantUsage − livre, os dois do volume da home (é o que o painel usa).
pub fn purgeable() -> Option<i64> {
    let _p = Pool::new();
    let u = ffi::url(&home());
    let imp = ffi::i64v(ffi::res(u, "NSURLVolumeAvailableCapacityForImportantUsageKey"))?;
    let livre = ffi::i64v(ffi::res(u, "NSURLVolumeAvailableCapacityKey"))?;
    Some(imp - livre)
}

/// CapacityInUse do volume montado em /System/Volumes/Data (diskutil apfs list -plist).
pub fn usado_data() -> Result<i64, String> {
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c"/System/Volumes/Data".as_ptr(), &mut st) } != 0 {
        return Err("statfs Data".into());
    }
    let dev = unsafe { std::ffi::CStr::from_ptr(st.f_mntfromname.as_ptr()) }.to_string_lossy().trim_start_matches("/dev/").to_string();
    let x = ffi::run("/usr/sbin/diskutil", &["apfs", "list", "-plist"], 60)?;
    let _p = Pool::new();
    let pl = ffi::plist(&x);
    for c in ffi::itens(ffi::get(pl, "Containers")) {
        for v in ffi::itens(ffi::get(c, "Volumes")) {
            if ffi::rs(ffi::get(v, "DeviceIdentifier")).as_deref() == Some(dev.as_str()) {
                return ffi::i64v(ffi::get(v, "CapacityInUse")).ok_or("sem CapacityInUse".into());
            }
        }
    }
    Err(format!("volume {dev} não achado no diskutil"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applications() {
        assert_eq!(pasta_em_applications("/Applications/Foo.app"), None);
        assert_eq!(pasta_em_applications("/Applications/Adobe/Photoshop.app").as_deref(), Some("/Applications/Adobe"));
        assert_eq!(pasta_em_applications("/System/Applications/Utilities/X.app").as_deref(), Some("/Applications/Utilities"));
    }

    #[test]
    fn faixas_sinteticas() {
        // itma + boma 1 (FileSize no offset 316) + boma 11 (URL)
        let mut d = Vec::new();
        d.extend(b"itma");
        d.extend(12u32.to_le_bytes());
        d.extend([0u8; 4]);
        let mut b1 = vec![0u8; 324];
        b1[..4].copy_from_slice(b"boma");
        b1[8..12].copy_from_slice(&324u32.to_le_bytes());
        b1[12..16].copy_from_slice(&1u32.to_le_bytes());
        b1[316..324].copy_from_slice(&4242u64.to_le_bytes());
        d.extend(&b1);
        let url = b"file:///x/a%20b.m4a\0";
        let mut b2 = vec![0u8; 36];
        b2[..4].copy_from_slice(b"boma");
        b2[8..12].copy_from_slice(&((36 + url.len()) as u32).to_le_bytes());
        b2[12..16].copy_from_slice(&11u32.to_le_bytes());
        b2.extend(url);
        d.extend(&b2);
        let f = faixas(&d).unwrap();
        assert_eq!(f, vec![(Some(4242), Some("/x/a b.m4a".to_string()))]);
    }
}
