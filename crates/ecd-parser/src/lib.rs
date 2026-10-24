//! # ecd-parser
//!
//! Leitor de alta performance para arquivos do SPED ECD utilizando memory-mapped I/O (memmap2),
//! decodificação de caracteres ISO-8859-1 e fatiamento zero-copy de bytes.

use memmap2::Mmap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Erro de I/O ao abrir arquivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("Arquivo SPED inválido ou corrompido na linha {0}: {1}")]
    InvalidLine(usize, String),
    #[error("Registro 0000 não localizado no início do arquivo")]
    MissingHeader,
}

pub struct SpedRecord<'a> {
    pub bloco: &'a [u8],
    pub campos: Vec<&'a [u8]>,
}

pub struct SpedStreamReader<'a> {
    data: &'a [u8],
}

impl<'a> SpedStreamReader<'a> {
    pub fn from_mmap(mmap: &'a Mmap) -> Self {
        Self { data: mmap.as_ref() }
    }

    /// Itera sobre as linhas do arquivo SPED fatiadas em campos delimitados por '|'
    pub fn iter_records(&self) -> impl Iterator<Item = SpedRecord<'a>> {
        self.data.split(|&b| b == b'\n').filter_map(|raw_line| {
            let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
            if line.starts_with(b"|") && line.ends_with(b"|") && line.len() > 2 {
                let inner = &line[1..line.len() - 1];
                let campos: Vec<&'a [u8]> = inner.split(|&b| b == b'|').collect();
                if let Some(bloco) = campos.first() {
                    return Some(SpedRecord { bloco, campos });
                }
            }
            None
        })
    }
}

pub fn decode_latin1(bytes: &[u8]) -> String {
    let (cow, _encoding, _had_errors) = encoding_rs::WINDOWS_1252.decode(bytes);
    cow.into_owned()
}
