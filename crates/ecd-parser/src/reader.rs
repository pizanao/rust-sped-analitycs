use std::fs::File;
use std::path::Path;
use std::str::FromStr;

use chrono::NaiveDate;
use memmap2::{Mmap, MmapOptions};
use rust_decimal::Decimal;

use crate::ParserError;

/// Representa um registro individual do SPED ECD fatiado em modo zero-copy (`&[u8]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpedRecord<'a> {
    pub bloco: &'a [u8],
    pub campos: Vec<&'a [u8]>,
}

impl<'a> SpedRecord<'a> {
    /// Constrói um `SpedRecord` a partir de uma linha bruta de bytes delimitada por `|`.
    pub fn from_line(raw_line: &'a [u8]) -> Option<Self> {
        let line = trim_ascii_bytes(raw_line);
        if line.starts_with(b"|") && line.ends_with(b"|") && line.len() > 2 {
            let inner = &line[1..line.len() - 1];
            let mut campos: Vec<&'a [u8]> = inner.split(|&b| b == b'|').collect();
            if let Some(first) = campos.first_mut() {
                let bloco = trim_ascii_bytes(first);
                if !bloco.is_empty() {
                    *first = bloco;
                    return Some(SpedRecord { bloco, campos });
                }
            }
        }
        None
    }

    /// Retorna o código do bloco (ex: `"0000"`, `"I050"`, `"J150"`) decodificado.
    pub fn bloco_str(&self) -> String {
        decode_field(self.bloco)
    }

    /// Retorna a fatia de bytes do campo no índice informado.
    pub fn campo_bytes(&self, index: usize) -> Option<&'a [u8]> {
        self.campos.get(index).copied()
    }

    /// Retorna o campo decodificado em `WINDOWS_1252` com espaços laterais removidos.
    pub fn campo_str(&self, index: usize) -> Option<String> {
        self.campos.get(index).map(|b| decode_field(b))
    }

    /// Retorna `Some(String)` se o campo existir e não for vazio após `trim()`, ou `None`.
    pub fn campo_opt_str(&self, index: usize) -> Option<String> {
        self.campos.get(index).and_then(|b| decode_opt_field(b))
    }
}

/// Wrapper seguro para mapeamento de arquivo SPED em memória (`memmap2::Mmap`),
/// tratando arquivos vazios (0 bytes) sem erro de sistema operacional (`EINVAL`).
pub struct SpedMmap {
    mmap: Option<Mmap>,
}

impl SpedMmap {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ParserError> {
        let file = File::open(path.as_ref())?;
        let metadata = file.metadata()?;
        if metadata.len() == 0 {
            return Ok(Self { mmap: None });
        }
        // SAFETY: O arquivo é aberto apenas para leitura em estágio controlado.
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        Ok(Self { mmap: Some(mmap) })
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.mmap.as_ref().map(|m| m.as_ref()).unwrap_or(&[])
    }

    pub fn reader(&self) -> SpedStreamReader<'_> {
        SpedStreamReader::from_bytes(self.as_bytes())
    }
}

/// Abre diretamente um `memmap2::Mmap` a partir de um caminho no sistema de arquivos.
pub fn open_mmap(path: impl AsRef<Path>) -> Result<Mmap, ParserError> {
    let file = File::open(path.as_ref())?;
    if file.metadata()?.len() == 0 {
        return Err(ParserError::MissingHeader);
    }
    // SAFETY: Mapeamento somente leitura do arquivo SPED em disco.
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    Ok(mmap)
}

/// Leitor streaming zero-copy sobre fatia de bytes ou `memmap2::Mmap`.
pub struct SpedStreamReader<'a> {
    data: &'a [u8],
}

impl<'a> SpedStreamReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self::from_bytes(data)
    }

    pub fn from_bytes(data: &'a [u8]) -> Self {
        // Remove UTF-8 BOM inicial caso presente
        let clean = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
        Self { data: clean }
    }

    pub fn from_mmap(mmap: &'a Mmap) -> Self {
        Self::from_bytes(mmap.as_ref())
    }

    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Itera sobre as linhas válidas do arquivo SPED fatiadas em campos delimitados por `|`.
    pub fn iter_records(&self) -> impl Iterator<Item = SpedRecord<'a>> + '_ {
        self.data
            .split(|&b| b == b'\n')
            .filter_map(SpedRecord::from_line)
    }

    /// Itera sobre as linhas válidas do arquivo SPED preservando o número da linha (1-indexed).
    pub fn iter_records_enumerated(&self) -> impl Iterator<Item = (usize, SpedRecord<'a>)> + '_ {
        self.data
            .split(|&b| b == b'\n')
            .enumerate()
            .filter_map(|(idx, raw_line)| SpedRecord::from_line(raw_line).map(|rec| (idx + 1, rec)))
    }
}

#[inline]
pub fn trim_ascii_bytes(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|&b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|&b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        .map(|pos| pos + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

/// Decodifica bytes em `WINDOWS_1252` (compatível com `ISO-8859-1` do SPED ECD).
pub fn decode_latin1(bytes: &[u8]) -> String {
    let (cow, _encoding, _had_errors) = encoding_rs::WINDOWS_1252.decode(bytes);
    cow.into_owned()
}

/// Decodifica bytes em `WINDOWS_1252` e aplica `.trim()`.
pub fn decode_field(bytes: &[u8]) -> String {
    let trimmed = trim_ascii_bytes(bytes);
    let (cow, _encoding, _had_errors) = encoding_rs::WINDOWS_1252.decode(trimmed);
    cow.trim().to_string()
}

/// Decodifica um campo opcional em `WINDOWS_1252`, retornando `None` se estiver vazio.
pub fn decode_opt_field(bytes: &[u8]) -> Option<String> {
    let s = decode_field(bytes);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Faz o parsing de datas do SPED nos formatos `DDMMAAAA` (padrão RFB), `AAAAMMDD` ou `AAAA-MM-DD`.
pub fn parse_sped_date(
    bytes: &[u8],
    line_num: usize,
    field_name: &str,
) -> Result<NaiveDate, ParserError> {
    let raw = decode_field(bytes);
    if raw.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Campo de data obrigatório '{field_name}' está vazio"),
        ));
    }

    if raw.len() == 8 && raw.bytes().all(|b| b.is_ascii_digit()) {
        let last4: u32 = raw[4..8].parse().unwrap_or(0);
        let first4: u32 = raw[0..4].parse().unwrap_or(0);

        // No formato oficial SPED DDMMAAAA, os 4 últimos dígitos representam o ano (ex: 1900..=2199)
        if (1900..=2199).contains(&last4) {
            if let Ok(date) = NaiveDate::parse_from_str(&raw, "%d%m%Y") {
                return Ok(date);
            }
        }
        // No formato alternativo AAAAMMDD, os 4 primeiros dígitos representam o ano
        if (1900..=2199).contains(&first4) {
            if let Ok(date) = NaiveDate::parse_from_str(&raw, "%Y%m%d") {
                return Ok(date);
            }
        }
        // Fallback caso o ano esteja fora de 1900..=2199
        if let Ok(date) = NaiveDate::parse_from_str(&raw, "%d%m%Y") {
            return Ok(date);
        }
        if let Ok(date) = NaiveDate::parse_from_str(&raw, "%Y%m%d") {
            return Ok(date);
        }
    } else if raw.len() == 10 {
        if let Ok(date) = NaiveDate::parse_from_str(&raw, "%Y-%m-%d") {
            return Ok(date);
        }
        if let Ok(date) = NaiveDate::parse_from_str(&raw, "%d/%m/%Y") {
            return Ok(date);
        }
    }

    Err(ParserError::InvalidLine(
        line_num,
        format!("Data inválida no campo '{field_name}': '{raw}'"),
    ))
}

/// Faz o parsing de um campo de data opcional do SPED (`None` se vazio).
pub fn parse_opt_sped_date(
    bytes: &[u8],
    line_num: usize,
    field_name: &str,
) -> Result<Option<NaiveDate>, ParserError> {
    let trimmed = trim_ascii_bytes(bytes);
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_sped_date(trimmed, line_num, field_name).map(Some)
}

/// Faz o parsing de valores numéricos decimais no padrão SPED (`1234,56`, `1.234,56` ou `1234.56`).
pub fn parse_sped_decimal(
    bytes: &[u8],
    line_num: usize,
    field_name: &str,
) -> Result<Decimal, ParserError> {
    let raw = decode_field(bytes);
    if raw.is_empty() {
        return Err(ParserError::InvalidLine(
            line_num,
            format!("Campo decimal obrigatório '{field_name}' está vazio"),
        ));
    }

    let normalized = if raw.contains('.') && raw.contains(',') {
        raw.replace('.', "").replace(',', ".")
    } else if raw.contains(',') {
        raw.replace(',', ".")
    } else {
        raw.clone()
    };

    Decimal::from_str(&normalized).map_err(|_| {
        ParserError::InvalidLine(
            line_num,
            format!("Valor decimal inválido no campo '{field_name}': '{raw}'"),
        )
    })
}

/// Faz o parsing de valores decimais retornando `Decimal::ZERO` caso o campo esteja em branco.
pub fn parse_sped_decimal_or_zero(
    bytes: &[u8],
    line_num: usize,
    field_name: &str,
) -> Result<Decimal, ParserError> {
    let trimmed = trim_ascii_bytes(bytes);
    if trimmed.is_empty() {
        return Ok(Decimal::ZERO);
    }
    parse_sped_decimal(trimmed, line_num, field_name)
}

/// Faz o parsing de valores decimais opcionais (`None` caso em branco).
pub fn parse_opt_sped_decimal(
    bytes: &[u8],
    line_num: usize,
    field_name: &str,
) -> Result<Option<Decimal>, ParserError> {
    let trimmed = trim_ascii_bytes(bytes);
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_sped_decimal(trimmed, line_num, field_name).map(Some)
}

/// Faz o parsing de um inteiro `i32` obrigatório.
pub fn parse_sped_i32(bytes: &[u8], line_num: usize, field_name: &str) -> Result<i32, ParserError> {
    let raw = decode_field(bytes);
    raw.parse::<i32>().map_err(|_| {
        ParserError::InvalidLine(
            line_num,
            format!("Inteiro i32 inválido no campo '{field_name}': '{raw}'"),
        )
    })
}

/// Faz o parsing de um inteiro `i32` retornando `default` se o campo estiver vazio.
pub fn parse_sped_i32_or(
    bytes: &[u8],
    default: i32,
    line_num: usize,
    field_name: &str,
) -> Result<i32, ParserError> {
    let trimmed = trim_ascii_bytes(bytes);
    if trimmed.is_empty() {
        return Ok(default);
    }
    parse_sped_i32(trimmed, line_num, field_name)
}

/// Faz o parsing de um inteiro `i32` opcional (`None` se vazio).
pub fn parse_opt_sped_i32(
    bytes: &[u8],
    line_num: usize,
    field_name: &str,
) -> Result<Option<i32>, ParserError> {
    let trimmed = trim_ascii_bytes(bytes);
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_sped_i32(trimmed, line_num, field_name).map(Some)
}

/// Faz o parsing de um inteiro `i64` obrigatório.
pub fn parse_sped_i64(bytes: &[u8], line_num: usize, field_name: &str) -> Result<i64, ParserError> {
    let raw = decode_field(bytes);
    raw.parse::<i64>().map_err(|_| {
        ParserError::InvalidLine(
            line_num,
            format!("Inteiro i64 inválido no campo '{field_name}': '{raw}'"),
        )
    })
}

/// Faz o parsing de um inteiro `i64` opcional (`None` se vazio).
pub fn parse_opt_sped_i64(
    bytes: &[u8],
    line_num: usize,
    field_name: &str,
) -> Result<Option<i64>, ParserError> {
    let trimmed = trim_ascii_bytes(bytes);
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_sped_i64(trimmed, line_num, field_name).map(Some)
}

/// Normaliza a descrição de conta contábil com paridade à função `process_cta` do legado (`apps/core/tratamento.py`).
pub fn process_cta(nome_conta: &str) -> String {
    let sem_c = nome_conta.replace("C/", "");
    sem_c
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ã' | 'â' | 'ä' | 'Á' | 'À' | 'Ã' | 'Â' | 'Ä' | 'ª' => 'A',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'ó' | 'ò' | 'õ' | 'ô' | 'ö' | 'Ó' | 'Ò' | 'Õ' | 'Ô' | 'Ö' | 'º' => 'O',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'ý' | 'ÿ' | 'Ý' | 'Ÿ' => 'Y',
            'ç' | 'Ç' => 'C',
            'ñ' | 'Ñ' => 'N',
            '¹' => '1',
            '²' => '2',
            '³' => '3',
            other => other.to_ascii_uppercase(),
        })
        .filter(|c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace())
        .collect::<String>()
        .trim()
        .to_string()
}
