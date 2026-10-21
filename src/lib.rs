//! sdtally: explica o "Dados do Sistema" do macOS só com medição (Apple Silicon, macOS 15+).
//! `unsafe` só em `sys`; o resto é Rust seguro, e o parser do getattrlistbulk é puro.
#![deny(unsafe_code)]

pub mod anon;
pub mod assinatura;
pub mod caps;
pub mod catalogo;
pub mod coleta;
pub mod conselho;
pub mod detalhe;
pub mod historico;
pub mod ledger;
pub mod linhas;
pub mod mcp;
pub mod owners;
pub mod parse;
pub mod plist;
pub mod redigir;
pub mod report;
pub mod sources;
#[allow(unsafe_code)]
pub mod sys;
pub mod traducao;
pub mod verify;
pub mod voltou;
pub mod walk;
