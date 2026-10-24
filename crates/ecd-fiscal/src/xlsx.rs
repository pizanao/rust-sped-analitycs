use std::io::Cursor;
use std::path::Path;

use ecd_domain::{RegI355, RegJ150, Registro0000, RegistroI155, RegistroJ100};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use umya_spreadsheet::{new_file, Border, Spreadsheet, Worksheet};

use crate::error::FiscalError;

/// Cor corporativa azul escura para os cabeçalhos das tabelas SPED (ARGB com opacidade total FF)
pub const COR_CABECALHO_AZUL: &str = "FF1F4E78";
pub const COR_FONTE_BRANCA: &str = "FFFFFFFF";
pub const FORMATO_MOEDA: &str = "\"R$\" #,##0.00";
pub const FORMATO_DATA: &str = "DD/MM/YYYY";

#[derive(Clone, Debug, Default)]
pub struct ExportacaoSpedXlsx {
    pub cnpj: String,
    pub razao_social: String,
    pub bloco_0000: Vec<Registro0000>,
    pub bloco_j100: Vec<RegistroJ100>,
    pub bloco_j150: Vec<RegJ150>,
    pub bloco_i155: Vec<RegistroI155>,
    pub bloco_i355: Vec<RegI355>,
}

impl ExportacaoSpedXlsx {
    pub fn new(cnpj: impl Into<String>, razao_social: impl Into<String>) -> Self {
        Self {
            cnpj: cnpj.into(),
            razao_social: razao_social.into(),
            ..Default::default()
        }
    }
}

pub struct SpedXlsxGenerator;

impl SpedXlsxGenerator {
    /// Aplica formatação visual padrão aos cabeçalhos (azul corporativo + texto branco em negrito)
    fn formatar_cabecalho(sheet: &mut Worksheet, col: u32, row: u32, titulo: &str) {
        let cell = sheet.get_cell_mut((col, row));
        cell.set_value_string(titulo);

        let style = cell.get_style_mut();
        style.set_background_color_solid(COR_CABECALHO_AZUL);
        style
            .get_font_mut()
            .get_color_mut()
            .set_argb(COR_FONTE_BRANCA);
        style.get_font_mut().set_bold(true);
        style
            .get_borders_mut()
            .get_bottom_mut()
            .set_border_style(Border::BORDER_THIN);
        style
            .get_borders_mut()
            .get_top_mut()
            .set_border_style(Border::BORDER_THIN);
    }

    /// Formata uma célula com número monetário brasileiro (`R$ #,##0.00`)
    fn formatar_celula_moeda(sheet: &mut Worksheet, col: u32, row: u32, valor: Decimal) {
        let valor_f64 = valor.to_f64().unwrap_or(0.0);
        let cell = sheet.get_cell_mut((col, row));
        cell.set_value_number(valor_f64);
        cell.get_style_mut()
            .get_number_format_mut()
            .set_format_code(FORMATO_MOEDA);
    }

    /// Formata uma célula com data no formato `DD/MM/YYYY`
    fn formatar_celula_data(sheet: &mut Worksheet, col: u32, row: u32, data_str: &str) {
        let cell = sheet.get_cell_mut((col, row));
        cell.set_value_string(data_str);
        cell.get_style_mut()
            .get_number_format_mut()
            .set_format_code(FORMATO_DATA);
    }

    /// Ajusta a largura das colunas dinamicamente baseado no conteúdo inserido
    fn auto_ajustar_colunas(sheet: &mut Worksheet, total_colunas: u32, larguras_max: &[usize]) {
        for col_idx in 1..=total_colunas {
            let idx = (col_idx - 1) as usize;
            let largura = if idx < larguras_max.len() {
                (larguras_max[idx] + 3).clamp(12, 60) as f64
            } else {
                15.0
            };
            sheet
                .get_column_dimension_by_number_mut(&col_idx)
                .set_width(largura);
        }
    }

    /// Gera o relatório completo em formato `Spreadsheet` da crate `umya-spreadsheet`
    pub fn gerar_workbook(dados: &ExportacaoSpedXlsx) -> Result<Spreadsheet, FiscalError> {
        let mut book = new_file();

        // 1. Configurar aba 0000 (renomeando a aba inicial "Sheet1")
        let _ = book.set_sheet_name(0, "0000");
        {
            let sheet = book.get_sheet_by_name_mut("0000").unwrap();
            let cabecalhos = [
                "CNPJ",
                "Razão Social",
                "Data Inicial",
                "Data Final",
                "UF",
                "Inscrição Estadual",
                "Inscrição Municipal",
                "Cód. Município",
                "Sit. Especial",
                "Sit. Inicial Período",
                "NIRE",
                "Finalidade Escrituração",
                "Grande Porte",
                "Tipo ECD",
                "Hash Substituto",
                "Hash Arquivo",
            ];

            let mut max_lens = vec![0; cabecalhos.len()];
            for (i, &cab) in cabecalhos.iter().enumerate() {
                Self::formatar_cabecalho(sheet, (i + 1) as u32, 1, cab);
                max_lens[i] = cab.len();
            }

            for (row, r) in (2u32..).zip(dados.bloco_0000.iter()) {
                let dt_ini_str = r.dt_ini.format("%d/%m/%Y").to_string();
                let dt_fim_str = r.dt_fim.format("%d/%m/%Y").to_string();
                let ie_str = r.ie.as_deref().unwrap_or("");
                let im_str = r.im.as_deref().unwrap_or("");
                let cod_mun_str = r.cod_mun.map(|v| v.to_string()).unwrap_or_default();
                let sit_esp_str = r.ind_sit_esp.map(|v| v.to_string()).unwrap_or_default();
                let hash_sub_str = r.cod_hash_sub.as_deref().unwrap_or("");

                let valores = [
                    r.cnpj.clone(),
                    r.nome_razao.clone(),
                    dt_ini_str,
                    dt_fim_str,
                    r.uf.clone(),
                    ie_str.to_string(),
                    im_str.to_string(),
                    cod_mun_str,
                    sit_esp_str,
                    r.ind_sit_ini_per.to_string(),
                    r.ind_nire.to_string(),
                    r.ind_fin_esc.to_string(),
                    r.ind_grande_porte.to_string(),
                    r.tip_ecd.to_string(),
                    hash_sub_str.to_string(),
                    r.hash_arquivo_id.clone(),
                ];

                for (i, val) in valores.iter().enumerate() {
                    let col = (i + 1) as u32;
                    if i == 2 || i == 3 {
                        Self::formatar_celula_data(sheet, col, row, val);
                    } else {
                        sheet.get_cell_mut((col, row)).set_value_string(val);
                    }
                    if val.len() > max_lens[i] {
                        max_lens[i] = val.len();
                    }
                }
            }
            Self::auto_ajustar_colunas(sheet, cabecalhos.len() as u32, &max_lens);
        }

        // 2. Criar aba J100 (Balanço Patrimonial)
        let _ = book.new_sheet("J100");
        {
            let sheet = book.get_sheet_by_name_mut("J100").unwrap();
            let cabecalhos = [
                "CNPJ",
                "Cód. Aglutinação",
                "Ind. Cód. Agl.",
                "Nível",
                "Cód. Agl. Superior",
                "Grupo Balanço",
                "Descrição",
                "Saldo Inicial",
                "D/C Inicial",
                "Saldo Final",
                "D/C Final",
                "Nota Explicativa",
            ];

            let mut max_lens = vec![0; cabecalhos.len()];
            for (i, &cab) in cabecalhos.iter().enumerate() {
                Self::formatar_cabecalho(sheet, (i + 1) as u32, 1, cab);
                max_lens[i] = cab.len();
            }

            for (row, r) in (2u32..).zip(dados.bloco_j100.iter()) {
                let agl_sup = r.cod_agl_sup.as_deref().unwrap_or("");
                let nota_exp = r.nota_exp_ref.as_deref().unwrap_or("");

                sheet.get_cell_mut((1, row)).set_value_string(&r.cnpj);
                sheet.get_cell_mut((2, row)).set_value_string(&r.cod_agl);
                sheet
                    .get_cell_mut((3, row))
                    .set_value_string(&r.ind_cod_agl);
                sheet
                    .get_cell_mut((4, row))
                    .set_value_number(r.nivel_agl as f64);
                sheet.get_cell_mut((5, row)).set_value_string(agl_sup);
                sheet
                    .get_cell_mut((6, row))
                    .set_value_string(&r.ind_grp_bal);
                sheet
                    .get_cell_mut((7, row))
                    .set_value_string(&r.descr_cod_agl);
                Self::formatar_celula_moeda(sheet, 8, row, r.vl_cta_ini);
                sheet
                    .get_cell_mut((9, row))
                    .set_value_string(&r.ind_dc_cta_ini);
                Self::formatar_celula_moeda(sheet, 10, row, r.vl_cta_fin);
                sheet
                    .get_cell_mut((11, row))
                    .set_value_string(&r.ind_dc_cta_fin);
                sheet.get_cell_mut((12, row)).set_value_string(nota_exp);

                let lens = [
                    r.cnpj.len(),
                    r.cod_agl.len(),
                    r.ind_cod_agl.len(),
                    4,
                    agl_sup.len(),
                    r.ind_grp_bal.len(),
                    r.descr_cod_agl.len(),
                    15,
                    r.ind_dc_cta_ini.len(),
                    15,
                    r.ind_dc_cta_fin.len(),
                    nota_exp.len(),
                ];
                for (i, &l) in lens.iter().enumerate() {
                    if l > max_lens[i] {
                        max_lens[i] = l;
                    }
                }
            }
            Self::auto_ajustar_colunas(sheet, cabecalhos.len() as u32, &max_lens);
        }

        // 3. Criar aba J150 (Demonstração do Resultado do Exercício - DRE)
        let _ = book.new_sheet("J150");
        {
            let sheet = book.get_sheet_by_name_mut("J150").unwrap();
            let cabecalhos = [
                "CNPJ",
                "Nº Ordem",
                "Cód. Aglutinação",
                "Ind. Cód. Agl.",
                "Nível",
                "Cód. Agl. Superior",
                "Descrição",
                "Saldo Inicial",
                "D/C Inicial",
                "Saldo Final",
                "D/C Final",
                "Grupo DRE",
                "Data Inicial",
                "Data Final",
                "Hash Arquivo",
            ];

            let mut max_lens = vec![0; cabecalhos.len()];
            for (i, &cab) in cabecalhos.iter().enumerate() {
                Self::formatar_cabecalho(sheet, (i + 1) as u32, 1, cab);
                max_lens[i] = cab.len();
            }

            for (row, r) in (2u32..).zip(dados.bloco_j150.iter()) {
                let cod_agl = r.cod_agl.as_deref().unwrap_or("");
                let agl_sup = r.cod_agl_sup.as_deref().unwrap_or("");
                let dc_ini = r.ind_dc_cta_ini.as_deref().unwrap_or("");
                let dt_ini_str = r
                    .dt_ini
                    .map(|d| d.format("%d/%m/%Y").to_string())
                    .unwrap_or_default();
                let dt_fim_str = r
                    .dt_fim
                    .map(|d| d.format("%d/%m/%Y").to_string())
                    .unwrap_or_default();

                sheet.get_cell_mut((1, row)).set_value_string(&r.cnpj);
                sheet
                    .get_cell_mut((2, row))
                    .set_value_number(r.nu_ordem as f64);
                sheet.get_cell_mut((3, row)).set_value_string(cod_agl);
                sheet
                    .get_cell_mut((4, row))
                    .set_value_string(&r.ind_cod_agl);
                sheet
                    .get_cell_mut((5, row))
                    .set_value_number(r.nivel_agl as f64);
                sheet.get_cell_mut((6, row)).set_value_string(agl_sup);
                sheet
                    .get_cell_mut((7, row))
                    .set_value_string(&r.descr_cod_agl);
                Self::formatar_celula_moeda(sheet, 8, row, r.vl_cta_ini);
                sheet.get_cell_mut((9, row)).set_value_string(dc_ini);
                Self::formatar_celula_moeda(sheet, 10, row, r.vl_cta_fin);
                sheet
                    .get_cell_mut((11, row))
                    .set_value_string(&r.ind_dc_cta_fin);
                sheet
                    .get_cell_mut((12, row))
                    .set_value_string(&r.ind_grp_dre);
                Self::formatar_celula_data(sheet, 13, row, &dt_ini_str);
                Self::formatar_celula_data(sheet, 14, row, &dt_fim_str);
                sheet
                    .get_cell_mut((15, row))
                    .set_value_string(&r.hash_arquivo);

                let lens = [
                    r.cnpj.len(),
                    6,
                    cod_agl.len(),
                    r.ind_cod_agl.len(),
                    4,
                    agl_sup.len(),
                    r.descr_cod_agl.len(),
                    15,
                    dc_ini.len(),
                    15,
                    r.ind_dc_cta_fin.len(),
                    r.ind_grp_dre.len(),
                    dt_ini_str.len(),
                    dt_fim_str.len(),
                    r.hash_arquivo.len(),
                ];
                for (i, &l) in lens.iter().enumerate() {
                    if l > max_lens[i] {
                        max_lens[i] = l;
                    }
                }
            }
            Self::auto_ajustar_colunas(sheet, cabecalhos.len() as u32, &max_lens);
        }

        // 4. Criar aba I155 (Detalhe dos Saldos Periódicos)
        let _ = book.new_sheet("I155");
        {
            let sheet = book.get_sheet_by_name_mut("I155").unwrap();
            let cabecalhos = [
                "Cód. Conta",
                "Centro de Custos",
                "Saldo Inicial",
                "D/C Inicial",
                "Débito",
                "Crédito",
                "Saldo Final",
                "D/C Final",
            ];

            let mut max_lens = vec![0; cabecalhos.len()];
            for (i, &cab) in cabecalhos.iter().enumerate() {
                Self::formatar_cabecalho(sheet, (i + 1) as u32, 1, cab);
                max_lens[i] = cab.len();
            }

            for (row, r) in (2u32..).zip(dados.bloco_i155.iter()) {
                let ccus = r.cod_ccus.as_deref().unwrap_or("");

                sheet.get_cell_mut((1, row)).set_value_string(&r.cod_cta);
                sheet.get_cell_mut((2, row)).set_value_string(ccus);
                Self::formatar_celula_moeda(sheet, 3, row, r.vl_sld_ini);
                sheet.get_cell_mut((4, row)).set_value_string(&r.ind_dc_ini);
                Self::formatar_celula_moeda(sheet, 5, row, r.vl_deb);
                Self::formatar_celula_moeda(sheet, 6, row, r.vl_cred);
                Self::formatar_celula_moeda(sheet, 7, row, r.vl_sld_fim);
                sheet.get_cell_mut((8, row)).set_value_string(&r.ind_dc_fim);

                let lens = [
                    r.cod_cta.len(),
                    ccus.len(),
                    15,
                    r.ind_dc_ini.len(),
                    15,
                    15,
                    15,
                    r.ind_dc_fim.len(),
                ];
                for (i, &l) in lens.iter().enumerate() {
                    if l > max_lens[i] {
                        max_lens[i] = l;
                    }
                }
            }
            Self::auto_ajustar_colunas(sheet, cabecalhos.len() as u32, &max_lens);
        }

        // 5. Opcional: criar aba I355 se houver registros
        if !dados.bloco_i355.is_empty() {
            let _ = book.new_sheet("I355");
            let sheet = book.get_sheet_by_name_mut("I355").unwrap();
            let cabecalhos = [
                "CNPJ",
                "Cód. Conta",
                "Conta Referência",
                "Centro de Custos",
                "Valor",
                "Ind. D/C",
                "Data Encerramento",
                "Cód. Conta Ref.",
                "Nome Aglutinador",
            ];

            let mut max_lens = vec![0; cabecalhos.len()];
            for (i, &cab) in cabecalhos.iter().enumerate() {
                Self::formatar_cabecalho(sheet, (i + 1) as u32, 1, cab);
                max_lens[i] = cab.len();
            }

            for (row, r) in (2u32..).zip(dados.bloco_i355.iter()) {
                let nome_ref = r.nome_cta_referencia.as_deref().unwrap_or("");
                let ccus = r.cod_ccus.as_deref().unwrap_or("");
                let dt_res_str = r.dt_res.format("%d/%m/%Y").to_string();
                let cta_ref = r.cod_cta_ref.as_deref().unwrap_or("");
                let agl = r.nome_aglutinador.as_deref().unwrap_or("");

                sheet.get_cell_mut((1, row)).set_value_string(&r.cnpj);
                sheet.get_cell_mut((2, row)).set_value_string(&r.cod_cta);
                sheet.get_cell_mut((3, row)).set_value_string(nome_ref);
                sheet.get_cell_mut((4, row)).set_value_string(ccus);
                Self::formatar_celula_moeda(sheet, 5, row, r.vl_cta);
                sheet.get_cell_mut((6, row)).set_value_string(&r.ind_dc);
                Self::formatar_celula_data(sheet, 7, row, &dt_res_str);
                sheet.get_cell_mut((8, row)).set_value_string(cta_ref);
                sheet.get_cell_mut((9, row)).set_value_string(agl);

                let lens = [
                    r.cnpj.len(),
                    r.cod_cta.len(),
                    nome_ref.len(),
                    ccus.len(),
                    15,
                    r.ind_dc.len(),
                    dt_res_str.len(),
                    cta_ref.len(),
                    agl.len(),
                ];
                for (i, &l) in lens.iter().enumerate() {
                    if l > max_lens[i] {
                        max_lens[i] = l;
                    }
                }
            }
            Self::auto_ajustar_colunas(sheet, cabecalhos.len() as u32, &max_lens);
        }

        Ok(book)
    }

    /// Exporta os dados contábeis em um buffer de bytes XLSX em memória
    pub fn exportar_bytes(dados: &ExportacaoSpedXlsx) -> Result<Vec<u8>, FiscalError> {
        let book = Self::gerar_workbook(dados)?;
        let mut cursor = Cursor::new(Vec::new());
        umya_spreadsheet::writer::xlsx::write_writer(&book, &mut cursor)?;
        Ok(cursor.into_inner())
    }

    /// Grava a planilha formatada diretamente em um arquivo no caminho especificado
    pub fn exportar_arquivo(
        dados: &ExportacaoSpedXlsx,
        caminho: impl AsRef<Path>,
    ) -> Result<(), FiscalError> {
        let book = Self::gerar_workbook(dados)?;
        umya_spreadsheet::writer::xlsx::write(&book, caminho)?;
        Ok(())
    }
}
