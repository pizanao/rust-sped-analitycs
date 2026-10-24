//! # ecd-parser
//!
//! Leitor de alta performance para arquivos do SPED ECD utilizando memory-mapped I/O (`memmap2`),
//! decodificação de caracteres `WINDOWS_1252` (`ISO-8859-1`) e fatiamento zero-copy de bytes.

use std::path::Path;

use chrono::{DateTime, NaiveDate, Utc};
use ecd_domain::{
    RegI050, RegI355, RegJ150, Registro0000, RegistroI010, RegistroI030, RegistroI050,
    RegistroI051, RegistroI052, RegistroI053, RegistroI150, RegistroI155, RegistroJ005,
    RegistroJ100, RegistroJ210, RegistroJ215,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod reader;
pub mod records;

pub use reader::*;
pub use records::*;

/// Tamanho padrão de lote (5.000 linhas) para flush em batch inserts no PostgreSQL.
pub const DEFAULT_BATCH_SIZE: usize = 5_000;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Erro de I/O ao abrir arquivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("Arquivo SPED inválido ou corrompido na linha {0}: {1}")]
    InvalidLine(usize, String),
    #[error("Registro 0000 não localizado no início do arquivo")]
    MissingHeader,
}

/// Resultado estruturado de uma leitura completa em streaming de um arquivo SPED ECD.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpedDocument {
    pub reg_0000: Registro0000,
    pub reg_i010: Vec<RegistroI010>,
    pub reg_i030: Vec<RegistroI030>,
    pub reg_i050: Vec<RegistroI050>,
    pub reg_i051: Vec<RegistroI051>,
    pub reg_i052: Vec<RegistroI052>,
    pub reg_i053: Vec<RegistroI053>,
    pub plano_contas: Vec<RegI050>,
    pub reg_i150: Vec<RegistroI150>,
    pub reg_i155: Vec<RegistroI155>,
    pub reg_i355: Vec<RegI355>,
    pub reg_j005: Vec<RegistroJ005>,
    pub reg_j100: Vec<RegistroJ100>,
    pub reg_j150: Vec<RegJ150>,
    pub reg_j210: Vec<RegistroJ210>,
    pub reg_j215: Vec<RegistroJ215>,
    pub total_linhas: usize,
}

impl SpedDocument {
    /// Abre e processa um arquivo SPED ECD a partir do disco usando `memmap2` em passagem única.
    pub fn parse_file(
        path: impl AsRef<Path>,
        hash_arquivo: &str,
        data_entrega: DateTime<Utc>,
    ) -> Result<Self, ParserError> {
        let mmap = SpedMmap::open(path)?;
        let reader = mmap.reader();
        Self::parse_from_reader(&reader, hash_arquivo, data_entrega)
    }

    /// Processa todos os blocos contábeis do SPED ECD a partir de um `SpedStreamReader`.
    pub fn parse_from_reader(
        reader: &SpedStreamReader<'_>,
        hash_arquivo: &str,
        data_entrega: DateTime<Utc>,
    ) -> Result<Self, ParserError> {
        let mut reg_0000: Option<Registro0000> = None;
        let mut consolidator: Option<PlanoContasConsolidator> = None;

        let mut reg_i010 = Vec::new();
        let mut reg_i030 = Vec::new();
        let mut reg_i050 = Vec::new();
        let mut reg_i051 = Vec::new();
        let mut reg_i052 = Vec::new();
        let mut reg_i053 = Vec::new();
        let mut reg_i150 = Vec::new();
        let mut reg_i155 = Vec::new();
        let mut reg_i355 = Vec::new();
        let mut reg_j005 = Vec::new();
        let mut reg_j100 = Vec::new();
        let mut reg_j150 = Vec::new();
        let mut reg_j210 = Vec::new();
        let mut reg_j215 = Vec::new();

        let mut current_i050_idx: Option<i64> = None;
        let mut current_i150_idx: Option<i64> = None;
        let mut current_i350_dt: Option<NaiveDate> = None;
        let mut current_j005_idx: Option<i64> = None;
        let mut current_j005_dates: Option<(NaiveDate, NaiveDate)> = None;
        let mut total_linhas = 0usize;

        for (line_num, record) in reader.iter_records_enumerated() {
            total_linhas += 1;

            if reg_0000.is_none() {
                if record.bloco == b"0000" {
                    let header = decode_0000_at_line(&record, hash_arquivo, line_num)?;
                    consolidator = Some(PlanoContasConsolidator::new(
                        header.cnpj.clone(),
                        hash_arquivo.to_string(),
                        data_entrega,
                    ));
                    reg_0000 = Some(header);
                    continue;
                } else {
                    return Err(ParserError::MissingHeader);
                }
            }

            let header = reg_0000.as_mut().expect("reg_0000 já verificado");
            let cnpj = header.cnpj.clone();
            let default_dates = (header.dt_ini, header.dt_fim);

            match record.bloco {
                b"I010" => {
                    let r = decode_i010_at_line(&record, 0, line_num)?;
                    reg_i010.push(r);
                }
                b"I030" => {
                    let r =
                        decode_i030_at_line(&record, 0, Some(hash_arquivo.to_string()), line_num)?;
                    reg_i030.push(r);
                }
                b"I050" => {
                    let mut r = decode_i050_at_line(&record, 0, line_num)?;
                    let next_id = (reg_i050.len() as i64) + 1;
                    r.id = Some(next_id);
                    if let Some(cons) = consolidator.as_mut() {
                        cons.push_i050(r.clone());
                    }
                    if r.ind_cta == "S" {
                        current_i050_idx = None;
                    } else {
                        current_i050_idx = Some(next_id);
                    }
                    reg_i050.push(r);
                }
                b"I051" => {
                    if let Some(i050_id) = current_i050_idx {
                        let r = decode_i051_at_line(&record, 0, i050_id, line_num)?;
                        if let Some(cons) = consolidator.as_mut() {
                            cons.push_i051(r.clone());
                        }
                        reg_i051.push(r);
                    }
                }
                b"I052" => {
                    if let Some(i050_id) = current_i050_idx {
                        let r = decode_i052_at_line(&record, 0, Some(i050_id), line_num)?;
                        if let Some(cons) = consolidator.as_mut() {
                            cons.push_i052(r.clone());
                        }
                        reg_i052.push(r);
                    }
                }
                b"I053" => {
                    let r = decode_i053_at_line(&record, 0, line_num)?;
                    reg_i053.push(r);
                }
                b"I150" => {
                    let mut r = decode_i150_at_line(&record, 0, line_num)?;
                    let next_id = (reg_i150.len() as i64) + 1;
                    r.id = Some(next_id);
                    current_i150_idx = Some(next_id);
                    reg_i150.push(r);
                }
                b"I155" => {
                    if let Some(i150_id) = current_i150_idx {
                        let r = decode_i155_at_line(&record, 0, i150_id, line_num)?;
                        reg_i155.push(r);
                    }
                }
                b"I350" => {
                    let dt = decode_i350_at_line(&record, line_num)?;
                    current_i350_dt = Some(dt);
                }
                b"I355" => {
                    let dt_res = current_i350_dt.unwrap_or(default_dates.1);
                    let r = decode_i355_at_line(&record, &cnpj, hash_arquivo, dt_res, line_num)?;
                    reg_i355.push(r);
                }
                b"I990" => {
                    header.i990 = decode_i990_at_line(&record, line_num)?;
                }
                b"J005" => {
                    let mut r = decode_j005_at_line(
                        &record,
                        0,
                        &cnpj,
                        Some(hash_arquivo.to_string()),
                        line_num,
                    )?;
                    let next_id = (reg_j005.len() as i64) + 1;
                    r.id = Some(next_id);
                    current_j005_idx = Some(next_id);
                    current_j005_dates = Some((r.dt_ini, r.dt_fim));
                    reg_j005.push(r);
                }
                b"J100" => {
                    if let Some(j005_id) = current_j005_idx {
                        let r = decode_j100_at_line(
                            &record,
                            0,
                            j005_id,
                            &cnpj,
                            Some(hash_arquivo.to_string()),
                            line_num,
                        )?;
                        reg_j100.push(r);
                    }
                }
                b"J150" => {
                    let (dt_ini, dt_fim) = current_j005_dates.unwrap_or(default_dates);
                    let mut r = decode_j150_at_line(
                        &record,
                        &cnpj,
                        hash_arquivo,
                        Some(dt_ini),
                        Some(dt_fim),
                        line_num,
                    )?;
                    if r.nu_ordem == 0 {
                        r.nu_ordem = (reg_j150.len() as i64) + 1;
                    }
                    reg_j150.push(r);
                }
                b"J210" => {
                    let r = decode_j210_at_line(
                        &record,
                        Some(0),
                        &cnpj,
                        Some(hash_arquivo.to_string()),
                        line_num,
                    )?;
                    reg_j210.push(r);
                }
                b"J215" => {
                    let r = decode_j215_at_line(
                        &record,
                        Some(0),
                        &cnpj,
                        Some(hash_arquivo.to_string()),
                        line_num,
                    )?;
                    reg_j215.push(r);
                }
                b"J800" | b"J900" | b"J930" => {
                    current_j005_idx = None;
                    current_i150_idx = None;
                }
                b"J990" => {
                    header.j990 = decode_j990_at_line(&record, line_num)?;
                }
                _ => {}
            }
        }

        let reg_0000 = reg_0000.ok_or(ParserError::MissingHeader)?;
        let plano_contas = consolidator.map(|c| c.finish()).unwrap_or_default();

        for item in &mut reg_i355 {
            enrich_i355(item, &plano_contas, &reg_j150);
        }

        Ok(Self {
            reg_0000,
            reg_i010,
            reg_i030,
            reg_i050,
            reg_i051,
            reg_i052,
            reg_i053,
            plano_contas,
            reg_i150,
            reg_i155,
            reg_i355,
            reg_j005,
            reg_j100,
            reg_j150,
            reg_j210,
            reg_j215,
            total_linhas,
        })
    }
}
