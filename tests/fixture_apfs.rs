//! Fixture APFS sem root (Frente E §3.1): imagem do hdiutil com arquivo comum, clone completo,
//! clone parcial, esparso, hardlink, comprimido e xattrs grandes. O esperado sai de st_blocks
//! medidos aqui mesmo, nunca de tamanho chutado, e a conta do volume é conferida contra o
//! ATTR_VOL_SPACEUSED da imagem: byte contado duas vezes (clone parcial, clone completo, hardlink)
//! faz a soma passar do que o volume usa. A imagem é desmontada e apagada no Drop.

use sdtally::report::i18n::Lang;
use sdtally::report::texto::{self, Estilo};
use sdtally::{anon::Ctx, detalhe, sys, walk};
use std::os::darwin::fs::MetadataExt as _;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MIB: i64 = 1 << 20;
const UF_COMPRESSED: u32 = 0x20;

struct Imagem {
    dmg: PathBuf,
    /// Pasta comum que contém o ponto de montagem: prova que a travessia não entra na imagem.
    pai: PathBuf,
    mnt: PathBuf,
}

impl Drop for Imagem {
    fn drop(&mut self) {
        // Sempre: desmonta à força e apaga só o que este teste criou.
        let _ = sys::run("/usr/bin/hdiutil", &["detach", "-force", s(&self.mnt)], 60);
        let _ = std::fs::remove_file(&self.dmg);
        let _ = std::fs::remove_file(self.dmg.with_extension("txt"));
        let _ = std::fs::remove_dir(&self.mnt);
        let _ = std::fs::remove_dir(&self.pai);
    }
}

fn s(p: &Path) -> &str {
    p.to_str().expect("caminho UTF-8")
}

fn sh(cmd: &str, args: &[&str]) {
    if let Err(e) = sys::run(cmd, args, 120) {
        panic!("{cmd} {args:?}: {e}");
    }
}

fn blocos(p: &Path) -> i64 {
    std::fs::symlink_metadata(p).expect("stat").blocks() as i64 * 512
}

fn walk(p: &Path) -> walk::Result {
    let ctx = Arc::new(Ctx::new("sdtfix-teste", "", &[]));
    walk::walk(s(p), 2, ctx).expect("walk")
}

/// Bytes pseudoaleatórios (xorshift): conteúdo incompressível e determinístico.
fn aleatorio(n: usize, mut x: u64) -> Vec<u8> {
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

fn montar() -> Imagem {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let id = std::process::id();
    let dmg = base.join(format!("sdtfix-{id}.dmg"));
    let pai = base.join(format!("sdtfix-{id}"));
    let mnt = pai.join("m");
    // hdiutil às vezes falha com "Resource busy" em runner (runner-images#7522): 3 tentativas,
    // com -ov (a tentativa que falhou pode ter deixado o .dmg) e pausa entre elas. O stderr vai
    // para a mensagem do assert. No 27 o `create -size` avisa que está obsoleto em favor de
    // `diskutil image create blank`, que o 15 não tem: trocar quando o 15 sair do suporte.
    let mut saidas = Vec::new();
    let criada = (0..3).any(|i| {
        if i > 0 {
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
        let r = sys::run_status(
            "/bin/sh",
            &["-c", "exec /usr/bin/hdiutil \"$@\" 2>&1", "sh", "create", "-ov", "-size", "300m", "-fs", "APFS", "-volname", "sdtfix", s(&dmg)],
            120,
        );
        let ok = matches!(r, Ok((true, _)));
        saidas.push(format!("{r:?}"));
        ok
    });
    assert!(criada, "hdiutil create falhou 3 vezes: {saidas:?}");
    std::fs::create_dir_all(&mnt).expect("mkdir mnt");
    let img = Imagem { dmg, pai, mnt };
    let pl = sys::plist_json("/usr/bin/hdiutil", &["attach", "-nobrowse", "-noverify", "-mountpoint", s(&img.mnt), "-plist", s(&img.dmg)], 120)
        .expect("hdiutil attach");
    let pm = pl["system-entities"].as_array().into_iter().flatten().find_map(|e| e["mount-point"].as_str()).expect("mount-point na saída do attach");
    assert_eq!(Path::new(pm).canonicalize().ok(), img.mnt.canonicalize().ok(), "ponto de montagem");
    img
}

#[test]
fn fixture_apfs_fecha_a_conta() {
    sys::politicas_seguranca().expect("setiopolicy_np");
    walk::MEASURE_XATTR.store(true, std::sync::atomic::Ordering::Relaxed);
    let img = montar();
    let m = img.mnt.canonicalize().expect("canonicalize");
    sh("/bin/sync", &[]);
    let usado_vazio = sys::apfs::vol_space_used(s(&m)).expect("ATTR_VOL_SPACEUSED") as i64;

    let t = m.join("t");
    let d = |n: &str| {
        let p = t.join(n);
        std::fs::create_dir_all(&p).expect("mkdir");
        p
    };
    let (comum, clones, parcial, esparso, hard, comp, xat) =
        (d("comum"), d("clones"), d("parcial"), d("esparso"), d("hardlink"), d("comprimido"), d("xattr"));

    std::fs::write(comum.join("c.bin"), aleatorio(3 * MIB as usize + 123, 1)).expect("comum");
    let orig = clones.join("orig.bin");
    std::fs::write(&orig, aleatorio(8 * MIB as usize, 2)).expect("orig");
    sh("/bin/cp", &["-c", s(&orig), s(&clones.join("clone.bin"))]);
    // Clone parcial: 1 MiB reescrito no meio; o resto continua compartilhado com orig.bin.
    let p = parcial.join("p.bin");
    sh("/bin/cp", &["-c", s(&orig), s(&p)]);
    sh("/bin/dd", &["if=/dev/urandom", &format!("of={}", s(&p)), "bs=1048576", "seek=3", "count=1", "conv=notrunc"]);
    // Esparso: 1 GiB aparente, 16 KiB escritos.
    let sp = esparso.join("s.img");
    sh("/usr/bin/truncate", &["-s", "1g", s(&sp)]);
    sh("/bin/dd", &["if=/dev/urandom", &format!("of={}", s(&sp)), "bs=4096", "seek=100000", "count=4", "conv=notrunc"]);
    let a = hard.join("a.bin");
    std::fs::write(&a, aleatorio(2 * MIB as usize, 3)).expect("hard");
    std::fs::hard_link(&a, hard.join("b.bin")).expect("ln");
    // Fonte fora da imagem: no mesmo volume o ditto do macOS 27 clona e ignora --hfsCompression.
    let txt = img.dmg.with_extension("txt");
    std::fs::write(&txt, "sdtally mede, não estima. ".repeat(160_000)).expect("texto");
    let cz = comp.join("c.txt");
    sh("/usr/bin/ditto", &["--hfsCompression", s(&txt), s(&cz)]);
    let tam_txt = blocos(&txt);
    let (x1, x2) = (xat.join("x1"), xat.join("x2"));
    std::fs::write(&x1, b"x1").expect("x1");
    std::fs::write(&x2, b"x2").expect("x2");
    // > 3804 B: fluxo próprio, fora do ALLOCSIZE (síntese §2.1). O pequeno fica embutido no
    // metadado: entra em xattr_all e não em xattr_stream (senão contaria duas vezes).
    sh("/usr/bin/xattr", &["-w", "org.sdtally.pequeno", &"p".repeat(100), s(&x1)]);
    sh("/usr/bin/xattr", &["-w", "org.sdtally.medio", &"m".repeat(4000), s(&x1)]);
    sh("/usr/bin/xattr", &["-w", "org.sdtally.grande", &"g".repeat(1_000_000), s(&x2)]);
    sh("/bin/sync", &[]);

    // ---- por caso ----
    let w = walk(&clones);
    assert_eq!((w.total.files, w.clone_dup_files), (1, 1), "clone completo deduplicado");
    assert_eq!(w.total.alloc, blocos(&orig));
    assert_eq!(w.clone_dup_bytes, blocos(&clones.join("clone.bin")));

    let w = walk(&hard);
    assert_eq!((w.total.files, w.link_dup_files), (1, 1), "hardlink contado uma vez");
    assert_eq!(w.total.alloc, blocos(&a));
    assert_eq!(w.link_dup_bytes, blocos(&a));

    let w = walk(&esparso);
    assert_eq!(std::fs::metadata(&sp).expect("stat").len(), 1 << 30);
    assert_eq!(w.total.alloc, blocos(&sp));
    assert!(w.total.alloc <= MIB, "esparso alocado {} > 1 MiB", w.total.alloc);

    let w = walk(&comp);
    assert_ne!(std::fs::metadata(&cz).expect("stat").st_flags() & UF_COMPRESSED, 0, "ditto não comprimiu");
    assert_eq!(w.total.alloc, blocos(&cz));
    assert!(w.total.alloc < tam_txt / 4, "comprimido {} vs original {tam_txt}", w.total.alloc);

    let w = walk(&xat);
    assert_eq!(w.total.xattr_stream, 4000 + 1_000_000);
    // xattr_all tem também o com.apple.provenance que o 27 põe em arquivo novo (11 B cada).
    assert!(w.total.xattr_all - w.total.xattr_stream >= 100, "xattr pequeno fora de xattr_all: {}", w.total.xattr_all);
    assert_eq!(w.total.alloc, blocos(&x1) + blocos(&x2));

    let wp = walk(&parcial);
    assert_eq!(wp.total.alloc, blocos(&p), "sozinho, o clone parcial não divide com ninguém");
    assert_eq!(wp.shared_dup_files, 0);
    // O que p divide com orig.bin: 8 MiB menos o 1 MiB reescrito (por construção, não pelo PRIVATESIZE).
    let compartilhado = blocos(&orig) - MIB;
    assert_eq!(compartilhado, 7 * MIB);

    // ---- árvore toda: soma de st_blocks por inode único, clone completo uma vez e os blocos que o
    // clone parcial divide com o original também uma vez ----
    let unicos = [&comum.join("c.bin"), &orig, &p, &sp, &a, &cz, &x1, &x2];
    let esperado: i64 = unicos.iter().map(|f| blocos(f)).sum::<i64>() - compartilhado;
    let wt = walk(&t);
    assert_eq!(wt.total.alloc, esperado, "alocado único de t/ (soma de st_blocks − blocos divididos)");
    assert_eq!(wt.total.files, unicos.len() as u64);
    assert_eq!((wt.shared_dup_files, wt.shared_dup_bytes, wt.sem_mapa), (1, compartilhado, 0));
    assert!(wt.errors.is_empty(), "erros na travessia: {:?}", wt.errors);

    // ---- a conta do volume: tudo o que a travessia soma tem de caber no que o volume usa ----
    let usado = sys::apfs::vol_space_used(s(&m)).expect("ATTR_VOL_SPACEUSED") as i64;
    let wr = walk(&m);
    let soma = wr.total.alloc + wr.total.xattr_stream;
    let resto = usado - soma;
    eprintln!(
        "fixture: usado vazio {usado_vazio}; usado {usado}; árvore {}; xattr {}; resto {resto}; resto − usado vazio {}; blocos divididos {}; erros da raiz {:?}",
        wr.total.alloc,
        wr.total.xattr_stream,
        resto - usado_vazio,
        wr.shared_dup_bytes,
        wr.errors
    );
    // Sem conta fechada à força: o resto é metadado do APFS (volume vazio + árvores dos arquivos
    // novos) e o arredondamento dos xattrs em blocos de 4 KiB. Abaixo do volume vazio é byte contado
    // a mais (com o clone parcial contado inteiro, ficava 7 MiB abaixo); além de 1 MiB, byte que a
    // travessia não viu.
    // Folga de 4 blocos: o ATTR_VOL_SPACEUSED do volume vazio varia 4 KiB entre imagens.
    assert!(resto + 4 * 4096 >= usado_vazio, "conta passa do que o volume usa: resto {resto} < volume vazio {usado_vazio}");
    assert!(resto - usado_vazio <= MIB, "resto além do volume vazio: {}", resto - usado_vazio);

    // ---- drill (`sdtally <caminho>`) e why, na mesma imagem ----
    let r = detalhe::ramo(&t, 1, 2, Arc::new(walk::Controle::new(&sdtally::sys::INTERROMPIDO))).expect("ramo");
    let filho = |n: &str| r.filhos.iter().find(|f| f.nome == n).unwrap_or_else(|| panic!("filho {n}: {:?}", r.filhos));
    assert_eq!(r.filhos.len(), 7, "{:?}", r.filhos);
    assert_eq!(filho("clones").alocado, blocos(&orig), "clone completo uma vez no drill");
    assert_eq!(filho("hardlink").alocado, blocos(&a), "hardlink uma vez no drill");
    // O original (FILEID menor) fica com os blocos divididos; o clone parcial, só com o que é seu.
    assert_eq!(filho("parcial").alocado, blocos(&p) - compartilhado, "clone parcial sem os blocos do original");
    assert_eq!(filho("esparso").alocado, blocos(&sp));
    assert!(r.filhos.iter().all(|f| f.privado.is_some() && !f.protegida));
    assert!(r.filhos.windows(2).all(|w| w[0].alocado >= w[1].alocado), "ordem por alocado");
    assert_eq!((r.total.alloc, r.total.files, r.interrompida), (esperado, unicos.len() as u64, false));
    // Parcial: o privado desconta a parte dividida com orig.bin.
    assert_eq!(filho("parcial").privado, Some(wp.total.private));
    let r2 = detalhe::ramo(&t, 2, 2, Arc::new(walk::Controle::new(&sdtally::sys::INTERROMPIDO))).expect("ramo 2");
    // Arquivos estão um nível abaixo de t/: com --depth 2 os baldes são os mesmos.
    let nomes = |r: &detalhe::Ramo| r.filhos.iter().map(|f| f.nome.clone()).collect::<std::collections::BTreeSet<_>>();
    assert_eq!((nomes(&r2), r2.total.alloc), (nomes(&r), esperado));

    let w = detalhe::why(&sp).expect("why esparso");
    let at = w.attrs.as_ref().expect("attrs");
    assert_eq!((w.tipo, at.logico, at.alocado.map(|x| x as i64)), (detalhe::Tipo::Arquivo, Some(1 << 30), Some(blocos(&sp))));
    assert!(detalhe::ext_flags_nomes(at.ext_flags.unwrap_or(0)).contains(&"IS_SPARSE"), "{:?}", at.ext_flags);
    let w = detalhe::why(&clones.join("clone.bin")).expect("why clone");
    let at = w.attrs.as_ref().expect("attrs");
    assert!(detalhe::ext_flags_nomes(at.ext_flags.unwrap_or(0)).contains(&"SHARES_ALL_BLOCKS"));
    assert_eq!(at.privado, Some(0), "clone completo não libera nada");
    let w = detalhe::why(&a).expect("why hardlink");
    assert_eq!(w.attrs.as_ref().expect("attrs").links, Some(2));
    let w = detalhe::why(&cz).expect("why comprimido");
    assert!(detalhe::flags_nomes(w.st.4).contains(&"UF_COMPRESSED"));
    let w = detalhe::why(&x2).expect("why xattr");
    assert!(w.xattrs.as_ref().expect("xattrs").contains(&1_000_000));
    let w = detalhe::why(&t).expect("why pasta");
    assert_eq!((w.tipo, w.enxerto), (detalhe::Tipo::Pasta, Some(Ok(false))));
    assert!(w.dirstats.is_some());

    // Render de drill e why nos dois idiomas: sem chave faltando, sem cor, até 80 colunas.
    let wc = detalhe::why(&clones.join("clone.bin")).expect("why clone");
    for lang in [Lang::Pt, Lang::En] {
        let e = Estilo { lang, binario: false, cor: false, top: 15 };
        for s in [texto::ramo(&r, "", &e), texto::why(&w, "", &e), texto::why(&wc, "", &e)] {
            assert!(!s.contains('⟦') && !s.contains('\x1b'), "{s}");
            // Só uma palavra maior que 80 (o caminho do fixture) pode passar: não há onde quebrar.
            assert!(s.lines().all(|l| l.chars().count() <= 80 || !l.trim().contains(' ')), "linha larga:\n{s}");
        }
        let s = texto::ramo(&r, "", &e);
        assert!(!s.contains('≥') || s.lines().filter(|l| l.contains('≥')).count() == 1, "≥ só no privado total:\n{s}");
    }
    // Trava de volume: a imagem não é o Data; o próprio ponto de montagem é aceito.
    let mnt = sys::montado_em(&m).expect("statfs");
    assert!(!detalhe::mesmo_volume(&t, sdtally::sources::DATA).expect("mesmo_volume"));
    assert!(detalhe::mesmo_volume(&t, &mnt).expect("mesmo_volume"));

    // ---- montagem: o DEVID da entrada é o da pasta coberta; a trava tem de valer na pasta aberta ----
    std::fs::write(m.join("raiz.bin"), aleatorio(MIB as usize, 4)).expect("raiz.bin");
    let pai = img.pai.canonicalize().expect("canonicalize pai");
    let wm = walk(&pai);
    assert_eq!((wm.total.files, wm.total.alloc, wm.dirs, wm.other_dev), (0, 0, 1, 1), "travessia entrou na imagem montada");
    assert!(wm.denied.is_empty());
    drop(img);
}
