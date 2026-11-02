//! Synthetic RFB CNPJ open data fixture generator.
//! Produces minimal, valid, RFC/RFB-compliant 37 `.zip` archives encoded in Windows-1252.

use anyhow::{Context, Result};
use encoding_rs::WINDOWS_1252;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Canonical list of all 37 official RFB CNPJ open data zip files.
pub const CANONICAL_ZIP_FILES: [&str; 37] = [
    // 6 Domain lookup tables
    "Cnaes.zip",
    "Motivos.zip",
    "Municipios.zip",
    "Naturezas.zip",
    "Paises.zip",
    "Qualificacoes.zip",
    // 1 Special table
    "Simples.zip",
    // 10 Enterprises partitions
    "Empresas0.zip",
    "Empresas1.zip",
    "Empresas2.zip",
    "Empresas3.zip",
    "Empresas4.zip",
    "Empresas5.zip",
    "Empresas6.zip",
    "Empresas7.zip",
    "Empresas8.zip",
    "Empresas9.zip",
    // 10 Establishments partitions
    "Estabelecimentos0.zip",
    "Estabelecimentos1.zip",
    "Estabelecimentos2.zip",
    "Estabelecimentos3.zip",
    "Estabelecimentos4.zip",
    "Estabelecimentos5.zip",
    "Estabelecimentos6.zip",
    "Estabelecimentos7.zip",
    "Estabelecimentos8.zip",
    "Estabelecimentos9.zip",
    // 10 Partners partitions
    "Socios0.zip",
    "Socios1.zip",
    "Socios2.zip",
    "Socios3.zip",
    "Socios4.zip",
    "Socios5.zip",
    "Socios6.zip",
    "Socios7.zip",
    "Socios8.zip",
    "Socios9.zip",
];

/// Summary of generated synthetic dataset for test assertions.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SyntheticDatasetSummary {
    pub output_dir: PathBuf,
    pub total_zip_files: usize,
    pub empresas_count: usize,
    pub estabelecimentos_count: usize,
    pub filiais_count: usize,
    pub socios_count: usize,
    pub cnaes_count: usize,
    pub motivos_count: usize,
    pub municipios_count: usize,
    pub naturezas_count: usize,
    pub paises_count: usize,
    pub quals_count: usize,
    pub simples_count: usize,
    pub reference_code: String,
}

/// Options for synthetic data generation.
#[derive(Debug, Clone)]
pub struct GeneratorOptions {
    /// Date token embedded in internal filenames (e.g., "D61024" for 2026-10-24).
    /// Generates anoMes = "202610" >= "202608", triggering the Aug/2026 layout change.
    pub reference_code: String,
}

impl Default for GeneratorOptions {
    fn default() -> Self {
        Self {
            reference_code: "D61024".to_string(),
        }
    }
}

/// Creates a single `.zip` archive containing an internal file with Windows-1252 encoded content.
pub fn create_zip_with_content(
    zip_path: &Path,
    internal_name: &str,
    content_utf8: &str,
) -> Result<()> {
    let file = File::create(zip_path)
        .with_context(|| format!("Failed to create zip file at {:?}", zip_path))?;
    let mut zip = ZipWriter::new(file);

    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    zip.start_file(internal_name, options)
        .with_context(|| format!("Failed to start zip entry {:?}", internal_name))?;

    // Encode string to Windows-1252 bytes
    let (encoded_bytes, _, malformed) = WINDOWS_1252.encode(content_utf8);
    if malformed {
        anyhow::bail!("String contained characters outside Windows-1252 range");
    }

    zip.write_all(&encoded_bytes)
        .with_context(|| format!("Failed to write content to zip entry {:?}", internal_name))?;
    zip.finish()
        .with_context(|| format!("Failed to finalize zip file {:?}", zip_path))?;

    Ok(())
}

/// Generates the complete, minimal valid 37 `.zip` files into `output_dir`.
pub fn generate_synthetic_rfb_dataset(
    output_dir: &Path,
    options: Option<GeneratorOptions>,
) -> Result<SyntheticDatasetSummary> {
    let opts = options.unwrap_or_default();
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create target directory {:?}", output_dir))?;

    let ref_code = &opts.reference_code;

    // 1. CNAEs (Cnaes.zip -> *.CNAECSV)
    let cnaes_csv = "\
6201501;Desenvolvimento de programas de computador sob encomenda\r\n\
6202300;Desenvolvimento e licenciamento de programas de computador customizáveis\r\n\
6920601;Atividades de contabilidade\r\n\
4711301;Comércio varejista de mercadorias em geral com predominância de produtos alimentícios\r\n\
7020400;Atividades de consultoria em gestão empresarial\r\n\
";
    create_zip_with_content(
        &output_dir.join("Cnaes.zip"),
        &format!("F.K03200$Z.{}.CNAECSV", ref_code),
        cnaes_csv,
    )?;
    let cnaes_count = 5;

    // 2. Motivos (Motivos.zip -> *.MOTICSV)
    let motivos_csv = "\
00;SEM MOTIVO\r\n\
01;EXTINCAO POR ENCERRAMENTO LIQUIDACAO VOLUNTARIA\r\n\
63;OMISSAO DE DECLARACOES\r\n\
";
    create_zip_with_content(
        &output_dir.join("Motivos.zip"),
        &format!("F.K03200$Z.{}.MOTICSV", ref_code),
        motivos_csv,
    )?;
    let motivos_count = 3;

    // 3. Municípios (Municipios.zip -> *.MUNICCSV)
    let municipios_csv = "\
9051;CAMPO GRANDE\r\n\
7107;SAO PAULO\r\n\
6001;RIO DE JANEIRO\r\n\
0001;BRASILIA\r\n\
";
    create_zip_with_content(
        &output_dir.join("Municipios.zip"),
        &format!("F.K03200$Z.{}.MUNICCSV", ref_code),
        municipios_csv,
    )?;
    let municipios_count = 4;

    // 4. Naturezas Jurídicas (Naturezas.zip -> *.NATJUCSV)
    let naturezas_csv = "\
2062;Sociedade Empresária Limitada\r\n\
2054;Sociedade Anônima Fechada\r\n\
2135;Empresário Individual\r\n\
1015;Órgão Público do Poder Executivo Federal\r\n\
";
    create_zip_with_content(
        &output_dir.join("Naturezas.zip"),
        &format!("F.K03200$Z.{}.NATJUCSV", ref_code),
        naturezas_csv,
    )?;
    let naturezas_count = 4;

    // 5. Países (Paises.zip -> *.PAISCSV)
    let paises_csv = "\
105;BRASIL\r\n\
249;ESTADOS UNIDOS\r\n\
063;ARGENTINA\r\n\
";
    create_zip_with_content(
        &output_dir.join("Paises.zip"),
        &format!("F.K03200$Z.{}.PAISCSV", ref_code),
        paises_csv,
    )?;
    let paises_count = 3;

    // 6. Qualificações de Sócios (Qualificacoes.zip -> *.QUALSCSV)
    let quals_csv = "\
10;Diretor\r\n\
16;Presidente\r\n\
22;Sócio\r\n\
49;Sócio-Administrador\r\n\
05;Administrador\r\n\
";
    create_zip_with_content(
        &output_dir.join("Qualificacoes.zip"),
        &format!("F.K03200$Z.{}.QUALSCSV", ref_code),
        quals_csv,
    )?;
    let quals_count = 5;

    // 7. Simples Nacional (Simples.zip -> *.SIMPLES.CSV)
    // Columns: cnpj_basico;opcao_simples;data_opcao_simples;data_exclusao_simples;opcao_mei;data_opcao_mei;data_exclusao_mei
    let simples_csv = "\
11111111;S;20210101;;N;;\r\n\
22222222;N;;;N;;\r\n\
33333333;N;;;N;;\r\n\
44444444;N;;;N;;\r\n\
55555555;S;20220101;;S;20220101;\r\n\
";
    create_zip_with_content(
        &output_dir.join("Simples.zip"),
        &format!("F.K03200$Z.{}.SIMPLES.CSV", ref_code),
        simples_csv,
    )?;
    let simples_count = 5;

    // 8. Empresas (Empresas0.zip .. Empresas9.zip -> *.EMPRECSV)
    // Columns: cnpj_basico;razao_social;natureza_juridica;qualificacao_responsavel;capital_social_str;porte_empresa;ente_federativo_responsavel
    let mut total_empresas = 0;

    // Partition 0: Primary test companies (with Brazilian accents & comma decimal floats)
    let emp0_csv = "\
11111111;ALFA TECNOLOGIA LTDA;2062;49;150000,50;01;\r\n\
22222222;BETA COMÉRCIO E INDÚSTRIA S/A;2054;16;2500000,00;05;\r\n\
33333333;GAMA PARTICIPAÇÕES LTDA;2062;49;50000,00;03;\r\n\
44444444;DELTA SERVIÇOS TÉCNICOS S/A;2054;10;1000000,00;05;\r\n\
55555555;ÉPSILON INOVAÇÃO & CONSULTORIA EIRELI;2135;05;80000,75;01;\r\n\
";
    create_zip_with_content(
        &output_dir.join("Empresas0.zip"),
        &format!("F.K03200$Z.{}.EMPRECSV", ref_code),
        emp0_csv,
    )?;
    total_empresas += 5;

    // Partitions 1..9: Single valid company per partition to ensure non-empty partitions
    for part in 1..=9 {
        let radical = format!("9000000{}", part);
        let emp_csv = format!(
            "{};EMPRESA PARTIÇÃO {} LTDA;2062;49;10000,00;01;\r\n",
            radical, part
        );
        create_zip_with_content(
            &output_dir.join(format!("Empresas{}.zip", part)),
            &format!("F.K03200$Z.{}.EMPRECSV", ref_code),
            &emp_csv,
        )?;
        total_empresas += 1;
    }

    // 9. Estabelecimentos (Estabelecimentos0.zip .. Estabelecimentos9.zip -> *.ESTABELE)
    // 30 Columns:
    // 0: cnpj_basico, 1: cnpj_ordem, 2: cnpj_dv, 3: matriz_filial (1=Matriz, 2=Filial),
    // 4: nome_fantasia, 5: situacao_cadastral, 6: data_situacao_cadastral, 7: motivo_situacao_cadastral,
    // 8: nome_cidade_exterior, 9: pais, 10: data_inicio_atividades, 11: cnae_fiscal,
    // 12: cnae_fiscal_secundaria, 13: tipo_logradouro, 14: logradouro, 15: numero,
    // 16: complemento, 17: bairro, 18: cep, 19: uf, 20: municipio, 21: ddd1, 22: telefone1,
    // 23: ddd2, 24: telefone2, 25: ddd_fax, 26: fax, 27: correio_eletronico,
    // 28: situacao_especial, 29: data_situacao_especial
    let mut total_estabelecimentos = 0;
    let mut total_filiais = 0;

    let estab0_csv = "\
11111111;0001;91;1;ALFA TECH;02;20210101;00;;105;20200101;6201501;6202300;RUA;DOS DESENVOLVEDORES;100;SALA 1;CENTRO;79002000;MS;9051;67;33334444;;;;;contato@alfatech.com.br;;\r\n\
22222222;0001;82;1;BETA STORE;02;20210101;00;;105;20200101;4711301;;AVENIDA;PAULISTA;1000;;BELA VISTA;01310100;SP;7107;11;30004000;;;;;sac@betastore.com.br;;\r\n\
33333333;0001;73;1;GAMA MATRIZ;02;20210101;00;;105;20200101;7020400;;RUA;RIO BRANCO;500;;CENTRO;20040001;RJ;6001;21;22223333;;;;;diretoria@gamapart.com.br;;\r\n\
33333333;0002;54;2;GAMA FILIAL SP;02;20210601;00;;105;20210601;7020400;;AVENIDA;FARIA LIMA;200;SALA 20;PINHEIROS;05426000;SP;7107;11;38889999;;;;;filialsp@gamapart.com.br;;\r\n\
33333333;0003;35;2;;02;20220101;00;;105;20220101;7020400;;RUA;AFONSO PENA;300;;CENTRO;79002001;MS;9051;67;33221100;;;;;filialms@gamapart.com.br;;\r\n\
44444444;0001;64;1;DELTA SERVIÇOS;02;20210101;00;;105;20200101;6202300;;AVENIDA;AFONSO PENA;2500;;JARDIM DOS ESTADOS;79020001;MS;9051;67;33445566;;;;;contato@deltaserv.com.br;;\r\n\
55555555;0001;55;1;ÉPSILON LAB;02;20220101;00;;105;20220101;6201501;;RUA;14 DE JULHO;80;;CENTRO;79004000;MS;9051;67;33556677;;;;;lab@epsiloninovacao.com.br;;\r\n\
";
    create_zip_with_content(
        &output_dir.join("Estabelecimentos0.zip"),
        &format!("F.K03200$Z.{}.ESTABELE", ref_code),
        estab0_csv,
    )?;
    total_estabelecimentos += 7;
    total_filiais += 2; // 33333333000254 and 33333333000335

    for part in 1..=9 {
        let radical = format!("9000000{}", part);
        let estab_csv = format!(
            "{};0001;0{};1;FANTASIA {};02;20210101;00;;105;20200101;6201501;;RUA X;1;;CENTRO;79000000;MS;9051;67;30000000;;;;;;;\r\n",
            radical, part, part
        );
        create_zip_with_content(
            &output_dir.join(format!("Estabelecimentos{}.zip", part)),
            &format!("F.K03200$Z.{}.ESTABELE", ref_code),
            &estab_csv,
        )?;
        total_estabelecimentos += 1;
    }

    // 10. Sócios (Socios0.zip .. Socios9.zip -> *.SOCIOCSV)
    // 11 Columns:
    // 0: cnpj_basico
    // 1: identificador_de_socio ('1'=PJ, '2'=PF, '3'=Estrangeiro)
    // 2: nome_socio
    // 3: cnpj_cpf_socio (In Aug/2026 format, PJ partner is 8-digit radical!)
    // 4: qualificacao_socio
    // 5: data_entrada_sociedade
    // 6: pais
    // 7: representante_legal
    // 8: nome_representante
    // 9: qualificacao_representante_legal
    // 10: faixa_etaria
    let mut total_socios = 0;

    // Socios Partition 0:
    // Includes:
    // 1) PF -> PJ (Rule 2): JOAO DA SILVA (***123456**) in Company 1 (11111111)
    // 2) PJ -> PJ (Rule 1 & Aug/2026 quirk): Company 1 radical "11111111" as partner in Company 2 (22222222), with rep ROBERTO (***555444**) -> Rule 5
    // 3) PJ -> PJ (Rule 1 & Aug/2026 quirk): Company 2 radical "22222222" as partner in Company 3 (33333333)
    // 4) PE -> PJ (Rule 3): GLOBAL VENTURES LLC (empty CPF) in Company 4 (44444444), with rep MARIA (***987654**) -> Rule 4
    // 5) PF -> PJ (Rule 2): ANA CLARA (***789012**) in Company 5 (55555555)
    // 6) PF -> PF (Rule 6): PEDRO MENOR (***333222**) in Company 5, with rep PAULO PAI (***111222**)
    // 7) Boundary/dummy rep test: CARLOS TESTE (***444555**) in Company 1 with dummy rep '***000000**' (must not generate rep edge)
    let socios0_csv = "\
11111111;2;JOAO DA SILVA;***123456**;49;20200101;105;;;;5\r\n\
22222222;1;ALFA TECNOLOGIA LTDA;11111111;22;20200601;105;***555444**;ROBERTO REPRESENTANTE PJ;05;0\r\n\
33333333;1;BETA COMÉRCIO E INDÚSTRIA S/A;22222222;22;20210101;105;;;;0\r\n\
44444444;3;GLOBAL VENTURES LLC;;22;20210301;249;***987654**;MARIA REPRESENTANTE;05;0\r\n\
55555555;2;ANA CLARA MENEZES;***789012**;05;20220101;105;;;;4\r\n\
55555555;2;PEDRO MENOR;***333222**;22;20220101;105;***111222**;PAULO PAI REPRESENTANTE;05;1\r\n\
11111111;2;CARLOS TESTE COM DUMMY REP;***444555**;22;20200101;105;***000000**;DUMMY NOME;05;4\r\n\
";
    create_zip_with_content(
        &output_dir.join("Socios0.zip"),
        &format!("F.K03200$Z.{}.SOCIOCSV", ref_code),
        socios0_csv,
    )?;
    total_socios += 7;

    for part in 1..=9 {
        let radical = format!("9000000{}", part);
        let socio_csv = format!(
            "{};2;SÓCIO PARTIÇÃO {};***00000{}**;49;20200101;105;;;;5\r\n",
            radical, part, part
        );
        create_zip_with_content(
            &output_dir.join(format!("Socios{}.zip", part)),
            &format!("F.K03200$Z.{}.SOCIOCSV", ref_code),
            &socio_csv,
        )?;
        total_socios += 1;
    }

    Ok(SyntheticDatasetSummary {
        output_dir: output_dir.to_path_buf(),
        total_zip_files: 37,
        empresas_count: total_empresas,
        estabelecimentos_count: total_estabelecimentos,
        filiais_count: total_filiais,
        socios_count: total_socios,
        cnaes_count,
        motivos_count,
        municipios_count,
        naturezas_count,
        paises_count,
        quals_count,
        simples_count,
        reference_code: ref_code.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_generate_synthetic_rfb_dataset_completeness() {
        let temp = tempdir().expect("tempdir");
        let summary = generate_synthetic_rfb_dataset(temp.path(), None).expect("generator");

        assert_eq!(summary.total_zip_files, 37);
        assert_eq!(summary.empresas_count, 14); // 5 + 9
        assert_eq!(summary.estabelecimentos_count, 16); // 7 + 9
        assert_eq!(summary.filiais_count, 2);
        assert_eq!(summary.socios_count, 16); // 7 + 9

        // Verify each canonical zip file actually exists
        for name in CANONICAL_ZIP_FILES {
            let p = temp.path().join(name);
            assert!(p.exists(), "Missing canonical zip: {}", name);
            assert!(
                p.metadata().expect("meta").len() > 0,
                "Empty zip file: {}",
                name
            );
        }
    }
}
