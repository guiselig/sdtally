//! Descrição do disco inteiro pelo DiskArbitration (o que o `diskutil info` mostra em Internal e
//! TotalSize), no próprio processo: sem `diskutil` nem `plutil`. Só leitura, sem sessão agendada.

use std::ffi::{CString, c_char, c_void};

type Ref = *const c_void;

#[link(name = "DiskArbitration", kind = "framework")]
unsafe extern "C" {
    fn DASessionCreate(alocador: Ref) -> Ref;
    fn DADiskCreateFromBSDName(alocador: Ref, sessao: Ref, nome: *const c_char) -> Ref;
    fn DADiskCopyDescription(disco: Ref) -> Ref;
    static kDADiskDescriptionDeviceInternalKey: Ref;
    static kDADiskDescriptionMediaSizeKey: Ref;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDictionaryGetValue(d: Ref, chave: Ref) -> Ref;
    fn CFGetTypeID(r: Ref) -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(b: Ref) -> u8;
    fn CFNumberGetTypeID() -> usize;
    fn CFNumberGetValue(n: Ref, tipo: isize, saida: *mut c_void) -> bool;
    fn CFRelease(r: Ref);
}

const CF_NUMBER_SINT64: isize = 4;

/// Solta o objeto CF em qualquer saída.
struct Cf(Ref);

impl Drop for Cf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disco {
    /// `DeviceInternal`. O DiskArbitration só põe a chave nos discos internos; a ausência é externo
    /// (conferido com `diskutil info` em disk0, imagem de disco e disco de backup).
    pub interno: bool,
    /// `MediaSize` em bytes (o TotalSize do `diskutil info`).
    pub tamanho: Option<i64>,
}

/// Disco inteiro `bsd` ("disk0"). None: nome inválido ou disco desconhecido do DiskArbitration.
pub fn descrever(bsd: &str) -> Option<Disco> {
    let nome = CString::new(bsd).ok()?;
    unsafe {
        let sessao = Cf(DASessionCreate(std::ptr::null()));
        if sessao.0.is_null() {
            return None;
        }
        let disco = Cf(DADiskCreateFromBSDName(std::ptr::null(), sessao.0, nome.as_ptr()));
        if disco.0.is_null() {
            return None;
        }
        let desc = Cf(DADiskCopyDescription(disco.0));
        if desc.0.is_null() {
            return None;
        }
        let interno = CFDictionaryGetValue(desc.0, kDADiskDescriptionDeviceInternalKey);
        let interno = !interno.is_null() && CFGetTypeID(interno) == CFBooleanGetTypeID() && CFBooleanGetValue(interno) != 0;
        let tam = CFDictionaryGetValue(desc.0, kDADiskDescriptionMediaSizeKey);
        let tamanho = if !tam.is_null() && CFGetTypeID(tam) == CFNumberGetTypeID() {
            let mut v = 0i64;
            CFNumberGetValue(tam, CF_NUMBER_SINT64, &mut v as *mut i64 as *mut c_void).then_some(v)
        } else {
            None
        };
        Some(Disco { interno, tamanho })
    }
}
