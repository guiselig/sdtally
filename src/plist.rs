//! Leitor mínimo de plist (XML e binário `bplist00`) para as atribuições de `owners`: strings, listas
//! e dicionários; o resto vira `Outro`. Sem processo externo, sem `unsafe`. Entrada hostil: toda
//! posição é conferida, a profundidade e o número de nós têm teto, nada é gravado.

#[derive(Debug, Clone, PartialEq)]
pub enum V {
    Texto(String),
    Lista(Vec<V>),
    Dic(Vec<(String, V)>),
    Outro,
}

impl V {
    pub fn get(&self, k: &str) -> Option<&V> {
        match self {
            V::Dic(d) => d.iter().find(|(c, _)| c == k).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn texto(&self) -> Option<&str> {
        match self {
            V::Texto(s) => Some(s),
            _ => None,
        }
    }

    pub fn lista(&self) -> &[V] {
        match self {
            V::Lista(l) => l,
            _ => &[],
        }
    }
}

const PROF_MAX: u8 = 32;
/// Nós por arquivo: um bplist com referências repetidas não explode em tempo.
const NOS_MAX: usize = 200_000;

pub fn ler(b: &[u8]) -> Option<V> {
    if b.starts_with(b"bplist00") { binario(b) } else { xml(std::str::from_utf8(b).ok()?) }
}

// ---- XML ----

#[derive(Clone, Copy)]
enum Tok<'a> {
    Abre(&'a str),
    Fecha(&'a str),
    Vazio(&'a str),
    Texto(&'a str),
}

fn tokens(s: &str) -> Vec<Tok<'_>> {
    let mut v = Vec::new();
    let mut r = s;
    while !r.is_empty() {
        let Some(i) = r.find('<') else {
            v.push(Tok::Texto(r));
            break;
        };
        if i > 0 {
            v.push(Tok::Texto(&r[..i]));
        }
        r = &r[i..];
        // Comentário, declaração e instrução: pulados.
        let fim = if r.starts_with("<!--") { r.find("-->").map(|j| j + 3) } else { r.find('>').map(|j| j + 1) };
        let Some(fim) = fim else { break };
        let tag = &r[1..fim - 1];
        r = &r[fim..];
        if tag.starts_with(['!', '?']) {
            continue;
        }
        let nome = |t: &'_ str| -> usize { t.find(|c: char| c.is_whitespace() || c == '/').unwrap_or(t.len()) };
        if let Some(t) = tag.strip_prefix('/') {
            v.push(Tok::Fecha(&t[..nome(t)]));
        } else if let Some(t) = tag.strip_suffix('/') {
            v.push(Tok::Vazio(&t[..nome(t)]));
        } else {
            v.push(Tok::Abre(&tag[..nome(tag)]));
        }
    }
    v
}

fn desescapar(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut o = String::with_capacity(s.len());
    let mut r = s;
    while let Some(i) = r.find('&') {
        o.push_str(&r[..i]);
        r = &r[i..];
        let Some(j) = r.find(';').filter(|j| *j <= 10) else { break };
        match &r[1..j] {
            "amp" => o.push('&'),
            "lt" => o.push('<'),
            "gt" => o.push('>'),
            "quot" => o.push('"'),
            "apos" => o.push('\''),
            n => match n.strip_prefix('#').and_then(|d| match d.strip_prefix(['x', 'X']) {
                Some(h) => u32::from_str_radix(h, 16).ok(),
                None => d.parse().ok(),
            }) {
                Some(c) => o.push(char::from_u32(c).unwrap_or('\u{FFFD}')),
                None => o.push_str(&r[..=j]),
            },
        }
        r = &r[j + 1..];
    }
    o.push_str(r);
    o
}

struct Xml<'a> {
    t: Vec<Tok<'a>>,
    i: usize,
    nos: usize,
}

impl<'a> Xml<'a> {
    /// Próximo token (sem consumir), pulando o espaço entre tags.
    fn espiar(&mut self) -> Option<Tok<'a>> {
        while matches!(self.t.get(self.i), Some(Tok::Texto(x)) if x.trim().is_empty()) {
            self.i += 1;
        }
        self.t.get(self.i).copied()
    }

    fn prox(&mut self) -> Option<Tok<'a>> {
        let t = self.espiar()?;
        self.i += 1;
        Some(t)
    }

    /// Conteúdo de `<nome>…</nome>` (já aberto): texto cru, ou vazio.
    fn texto_ate(&mut self, nome: &str) -> Option<String> {
        let s = match self.t.get(self.i)? {
            Tok::Texto(x) => {
                self.i += 1;
                desescapar(x)
            }
            _ => String::new(),
        };
        matches!(self.t.get(self.i)?, Tok::Fecha(n) if *n == nome).then(|| self.i += 1)?;
        Some(s)
    }

    /// Pula até o fecha de `nome` (já aberto), contando aninhamento do mesmo nome.
    fn pular(&mut self, nome: &str) -> Option<()> {
        let mut n = 1;
        while n > 0 {
            match self.t.get(self.i)? {
                Tok::Abre(x) if *x == nome => n += 1,
                Tok::Fecha(x) if *x == nome => n -= 1,
                _ => {}
            }
            self.i += 1;
        }
        Some(())
    }

    fn valor(&mut self, prof: u8) -> Option<V> {
        self.nos += 1;
        if prof > PROF_MAX || self.nos > NOS_MAX {
            return None;
        }
        match self.prox()? {
            Tok::Abre("dict") => {
                let mut d = Vec::new();
                loop {
                    match self.prox()? {
                        Tok::Fecha("dict") => return Some(V::Dic(d)),
                        Tok::Abre("key") => {
                            let k = self.texto_ate("key")?;
                            d.push((k, self.valor(prof + 1)?));
                        }
                        _ => return None,
                    }
                }
            }
            Tok::Abre("array") => {
                let mut l = Vec::new();
                loop {
                    if matches!(self.espiar()?, Tok::Fecha("array")) {
                        self.i += 1;
                        return Some(V::Lista(l));
                    }
                    l.push(self.valor(prof + 1)?);
                }
            }
            Tok::Vazio("dict") => Some(V::Dic(Vec::new())),
            Tok::Vazio("array") => Some(V::Lista(Vec::new())),
            Tok::Vazio("string") => Some(V::Texto(String::new())),
            Tok::Abre("string") => self.texto_ate("string").map(V::Texto),
            Tok::Abre(n) => self.pular(n).map(|()| V::Outro),
            Tok::Vazio(_) => Some(V::Outro),
            _ => None,
        }
    }
}

fn xml(s: &str) -> Option<V> {
    let mut x = Xml { t: tokens(s), i: 0, nos: 0 };
    loop {
        match x.prox()? {
            Tok::Abre("plist") => return x.valor(0),
            Tok::Texto(_) => return None,
            _ => {}
        }
    }
}

// ---- binário ----

struct Bin<'a> {
    b: &'a [u8],
    osz: usize,
    rsz: usize,
    n: usize,
    tabela: usize,
    nos: usize,
}

fn inteiro(b: &[u8]) -> Option<usize> {
    if b.is_empty() || b.len() > 8 {
        return None;
    }
    usize::try_from(b.iter().fold(0u64, |a, x| a << 8 | *x as u64)).ok()
}

fn binario(b: &[u8]) -> Option<V> {
    let t = b.get(b.len().checked_sub(32)?..)?;
    let (osz, rsz) = (t[6] as usize, t[7] as usize);
    let (n, topo, tabela) = (inteiro(&t[8..16])?, inteiro(&t[16..24])?, inteiro(&t[24..32])?);
    if !(1..=8).contains(&osz) || !(1..=8).contains(&rsz) || n == 0 || topo >= n {
        return None;
    }
    // A tabela de offsets cabe antes do trailer.
    if tabela.checked_add(n.checked_mul(osz)?)? > b.len() - 32 {
        return None;
    }
    Bin { b, osz, rsz, n, tabela, nos: 0 }.obj(topo, 0)
}

impl Bin<'_> {
    /// (tamanho, onde começam os dados) do objeto em `off` com a nibble `lo`.
    fn tam(&self, off: usize, lo: u8) -> Option<(usize, usize)> {
        if lo < 0xF {
            return Some((lo as usize, off + 1));
        }
        let m = *self.b.get(off + 1)?;
        if m >> 4 != 1 {
            return None;
        }
        let k = 1usize << (m & 0xF);
        Some((inteiro(self.b.get(off + 2..off.checked_add(2 + k)?)?)?, off + 2 + k))
    }

    fn ref_em(&self, pos: usize) -> Option<usize> {
        inteiro(self.b.get(pos..pos.checked_add(self.rsz)?)?).filter(|r| *r < self.n)
    }

    fn obj(&mut self, r: usize, prof: u8) -> Option<V> {
        self.nos += 1;
        if prof > PROF_MAX || self.nos > NOS_MAX || r >= self.n {
            return None;
        }
        let p = self.tabela + r * self.osz;
        let off = inteiro(self.b.get(p..p + self.osz)?)?;
        let m = *self.b.get(off)?;
        let (hi, lo) = (m >> 4, m & 0xF);
        match hi {
            0x5 | 0x6 => {
                let (n, ini) = self.tam(off, lo)?;
                let larg = if hi == 0x5 { 1 } else { 2 };
                let d = self.b.get(ini..ini.checked_add(n.checked_mul(larg)?)?)?;
                Some(V::Texto(if hi == 0x5 {
                    String::from_utf8_lossy(d).into_owned()
                } else {
                    String::from_utf16_lossy(&d.chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>())
                }))
            }
            0xA => {
                let (n, ini) = self.tam(off, lo)?;
                // Cada referência ocupa `rsz` bytes: o tamanho declarado tem de caber no arquivo.
                (ini.checked_add(n.checked_mul(self.rsz)?)? <= self.b.len()).then_some(())?;
                (0..n).map(|i| self.ref_em(ini + i * self.rsz).and_then(|r| self.obj(r, prof + 1))).collect::<Option<Vec<_>>>().map(V::Lista)
            }
            0xD => {
                let (n, ini) = self.tam(off, lo)?;
                (ini.checked_add(n.checked_mul(self.rsz)?.checked_mul(2)?)? <= self.b.len()).then_some(())?;
                let mut d = Vec::new();
                for i in 0..n {
                    let k = self.ref_em(ini + i * self.rsz).and_then(|r| self.obj(r, prof + 1))?;
                    let v = self.ref_em(ini + (n + i) * self.rsz).and_then(|r| self.obj(r, prof + 1))?;
                    if let V::Texto(k) = k {
                        d.push((k, v));
                    }
                }
                Some(V::Dic(d))
            }
            _ => Some(V::Outro),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<!-- comentário -->
	<key>CFBundleIdentifier</key>
	<string>com.exemplo.App</string>
	<key>CFBundleName</key>
	<string>Hit&apos;n&apos;Mix &amp; &#233;</string>
	<key>Vazia</key>
	<string></string>
	<key>Outra</key>
	<string/>
	<key>LSUIElement</key>
	<true/>
	<key>Numero</key>
	<integer>3</integer>
	<key>Grupos</key>
	<array>
		<string>group.com.exemplo</string>
		<string>ABCDE12345.exemplo</string>
	</array>
	<key>Aninhado</key>
	<dict><key>x</key><array><dict><key>k</key><string>v</string></dict></array></dict>
</dict>
</plist>"#;

    #[test]
    fn xml_basico() {
        let v = ler(XML.as_bytes()).unwrap();
        assert_eq!(v.get("CFBundleIdentifier").and_then(V::texto), Some("com.exemplo.App"));
        assert_eq!(v.get("CFBundleName").and_then(V::texto), Some("Hit'n'Mix & é"));
        assert_eq!(v.get("Vazia").and_then(V::texto), Some(""));
        assert_eq!(v.get("Outra").and_then(V::texto), Some(""));
        assert_eq!(v.get("LSUIElement"), Some(&V::Outro));
        assert_eq!(v.get("Numero"), Some(&V::Outro));
        let g: Vec<_> = v.get("Grupos").unwrap().lista().iter().filter_map(V::texto).collect();
        assert_eq!(g, ["group.com.exemplo", "ABCDE12345.exemplo"]);
        assert_eq!(v.get("Aninhado").and_then(|a| a.get("x")).map(|l| l.lista().len()), Some(1));
    }

    #[test]
    fn xml_quebrado_vira_none() {
        assert!(ler(b"<plist><dict><key>a</key>").is_none());
        assert!(ler(b"nada").is_none());
        assert!(ler(b"").is_none());
        assert!(ler(&[0xff, 0xfe]).is_none());
        let fundo = format!("<plist>{}{}</plist>", "<array>".repeat(100), "</array>".repeat(100));
        assert!(ler(fundo.as_bytes()).is_none());
    }

    /// `{"CFBundleName": "Ana", "L": ["x"], "U": "é"}` montado à mão (plutil -convert binary1).
    fn bplist() -> Vec<u8> {
        let mut b = b"bplist00".to_vec();
        let mut offs = Vec::new();
        let mut obj = |b: &mut Vec<u8>, bytes: &[u8]| {
            offs.push(b.len());
            b.extend_from_slice(bytes);
        };
        // 0 dict, 1 chave, 2 valor, 3 chave L, 4 array, 5 "x", 6 chave U, 7 "é" (UTF-16)
        obj(&mut b, &[0xD3, 1, 3, 6, 2, 4, 7]);
        obj(&mut b, &[&[0x5C][..], b"CFBundleName"].concat());
        obj(&mut b, &[&[0x53][..], b"Ana"].concat());
        obj(&mut b, &[&[0x51][..], b"L"].concat());
        obj(&mut b, &[0xA1, 5]);
        obj(&mut b, &[&[0x51][..], b"x"].concat());
        obj(&mut b, &[&[0x51][..], b"U"].concat());
        obj(&mut b, &[0x61, 0x00, 0xE9]);
        let tabela = b.len();
        b.extend(offs.iter().map(|o| *o as u8));
        let mut t = [0u8; 32];
        t[6] = 1;
        t[7] = 1;
        t[15] = 8;
        t[31] = tabela as u8;
        b.extend_from_slice(&t);
        b
    }

    #[test]
    fn binario_basico() {
        let v = ler(&bplist()).unwrap();
        assert_eq!(v.get("CFBundleName").and_then(V::texto), Some("Ana"));
        assert_eq!(v.get("L").unwrap().lista(), [V::Texto("x".into())]);
        assert_eq!(v.get("U").and_then(V::texto), Some("é"));
    }

    #[test]
    fn binario_hostil() {
        let ok = bplist();
        // Qualquer corte ou byte trocado dá None ou um valor, nunca pânico.
        for n in 0..ok.len() {
            let _ = ler(&ok[..n]);
        }
        for i in 8..ok.len() {
            for x in [0u8, 0xFF, 0x7F] {
                let mut b = ok.clone();
                b[i] = x;
                let _ = ler(&b);
            }
        }
        // Dicionário que aponta para si mesmo: o teto de profundidade corta.
        let mut b = b"bplist00".to_vec();
        b.extend_from_slice(&[0xA1, 0]);
        let tabela = b.len();
        b.push(8);
        let mut t = [0u8; 32];
        t[6] = 1;
        t[7] = 1;
        t[15] = 1;
        t[31] = tabela as u8;
        b.extend_from_slice(&t);
        assert!(ler(&b).is_none());
    }
}
