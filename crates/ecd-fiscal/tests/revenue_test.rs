use std::collections::HashSet;

use chrono::NaiveDate;
use ecd_domain::{RegI355, RegJ150};
use ecd_fiscal::{LinhaExtracaoInput, ReceitaExtractor, CATALOGO_24_TERMOS};
use rust_decimal::Decimal;

#[test]
fn test_catalogo_24_termos_completo() {
    assert_eq!(CATALOGO_24_TERMOS.len(), 24);

    let extractor = ReceitaExtractor::new();
    for termo in &CATALOGO_24_TERMOS {
        // Autoconsistência do partial_ratio consigo mesmo deve ser sempre 1.0 (100%)
        assert_eq!(
            ReceitaExtractor::partial_ratio(termo, termo),
            1.0,
            "Auto-similaridade deve ser 1.0 para: {}",
            termo
        );

        // avaliacao contra o catalogo deve identificar com score >= 0.80
        let (score, matched) = extractor
            .avaliar_linha(termo)
            .unwrap_or_else(|| panic!("Termo do catálogo não reconhecido: {}", termo));
        assert!(
            score >= 0.80,
            "Score deve ser >= 0.80 para termo do catálogo {}: obtido {}",
            termo,
            score
        );
        assert!(!matched.is_empty());
    }
}

#[test]
fn test_normalizacao_acentos_e_pontuacao() {
    let raw = "  RECEITA BRUTA DE GESTÃO PATRIMONIAL - LOCAÇÃO, SERVIÇOS & OUTROS!!  ";
    let norm = ReceitaExtractor::normalizar(raw);
    assert_eq!(
        norm,
        "receita bruta de gestao patrimonial locacao servicos outros"
    );
}

#[test]
fn test_rejeicao_12_filtros_negativos_sql() {
    let extractor = ReceitaExtractor::new();

    // 12 casos reais que devem ser sumariamente descartados
    let casos_negativos = [
        ("RECEITA NAO OPERACIONAL", " NAO "),
        ("RECEITA NÃO OPERACIONAL", " NÃO "),
        ("CUSTO DAS MERCADORIAS VENDIDAS", "CUST"),
        ("DEDUCOES DA RECEITA BRUTA", "DEDU"),
        ("IMPOSTOS SOBRE SERVIÇOS", "IMP"),
        ("DESPESAS OPERACIONAIS GERAIS", "DESPE"),
        ("CONTRAPARTIDA DE RECEITA", "CONTRA"),
        ("SERVIÇOS DE TERCEIROS", "TERCEI"),
        ("DESPESAS COM PESSOAL", "PESSO"),
        ("RECEITA DE VENDAS PJ EXTERNO", " PJ "),
        ("RECEITA LIQUIDA DE VENDAS", "LIQUID"),
        ("RECEITA LÍQUIDA OPERACIONAL", "LÍQUID"),
        ("RECEITA L/QUIDA", "L%QUID"),
        ("RECEITA ANTES DOS TRIBUTOS SOBRE O LUCRO", "ANTES"),
    ];

    for (desc, motivo) in &casos_negativos {
        assert!(
            ReceitaExtractor::contem_filtro_negativo(desc),
            "Filtro negativo '{}' deveria ter barrado: {}",
            motivo,
            desc
        );
        let elegivel = extractor.elegivel_receita_j150("C", Decimal::from(1000), "T", desc);
        assert!(
            !elegivel,
            "Não pode ser elegível em J150 devido ao filtro negativo {}: {}",
            motivo, desc
        );
    }
}

#[test]
fn test_elegibilidade_j150_regras_rigidas() {
    let extractor = ReceitaExtractor::new();

    // Válido
    assert!(extractor.elegivel_receita_j150(
        "C",
        Decimal::from(50000),
        "T",
        "RECEITA BRUTA DE VENDAS DE MERCADORIAS"
    ));

    // Débito não é receita
    assert!(!extractor.elegivel_receita_j150(
        "D",
        Decimal::from(50000),
        "T",
        "RECEITA BRUTA DE VENDAS DE MERCADORIAS"
    ));

    // Valor zero ou negativo
    assert!(!extractor.elegivel_receita_j150(
        "C",
        Decimal::ZERO,
        "T",
        "RECEITA BRUTA DE VENDAS DE MERCADORIAS"
    ));
    assert!(!extractor.elegivel_receita_j150(
        "C",
        Decimal::from(-100),
        "T",
        "RECEITA BRUTA DE VENDAS DE MERCADORIAS"
    ));

    // Não totalizador (detalhe 'D')
    assert!(!extractor.elegivel_receita_j150(
        "C",
        Decimal::from(50000),
        "D",
        "RECEITA BRUTA DE VENDAS DE MERCADORIAS"
    ));

    // Sem palavra positiva BRUTA ou RECEITA
    assert!(!extractor.elegivel_receita_j150(
        "C",
        Decimal::from(50000),
        "T",
        "FATURAMENTO TOTAL DO PERIODO"
    ));
}

#[test]
fn test_fuzzy_matching_partial_ratio_paridade_legado() {
    // Exact substring
    let score = ReceitaExtractor::partial_ratio("receita bruta", "receita bruta de vendas");
    assert_eq!(score, 1.0);

    // Minor typo / omission
    let score =
        ReceitaExtractor::partial_ratio("receita bruta operacionl", "receita bruta operacional");
    assert!(
        score >= 0.80,
        "Similaridade deveria ser >= 0.80, obtido: {}",
        score
    );

    // Avaliação sequencial: o primeiro termo do catálogo com score >= 0.80 é 'receita bruta venda' (score ~ 88.24%)
    let extractor = ReceitaExtractor::new();
    let aval = extractor.avaliar_linha("RECEITA BRUTA DE PRESTACAO DE SERVICOS CONTABEIS");
    assert!(aval.is_some());
    let (score, termo) = aval.unwrap();
    assert!(score >= 0.80);
    assert_eq!(termo, "receita bruta venda");

    // E na avaliação direta contra o termo longo específico, atinge score > 0.90
    let score_longo = ReceitaExtractor::partial_ratio(
        "receita bruta venda prestacao de servicos",
        "receita bruta de prestacao de servicos contabeis",
    );
    assert!(
        score_longo >= 0.90,
        "Score longo deveria ser >= 0.90, obtido: {}",
        score_longo
    );

    // Casos limítrofes exatos do RapidFuzz que antes falhavam com normalized_levenshtein
    let score_tot =
        ReceitaExtractor::partial_ratio("receita total bruta", "receita bruta total apurada");
    assert!(
        score_tot >= 0.80,
        "Paridade RapidFuzz: 'receita total bruta' vs 'receita bruta total apurada' deve ser >= 0.80, obtido: {}",
        score_tot
    );

    let score_loc = ReceitaExtractor::partial_ratio(
        "receita bruta locacao",
        "receita com locacao de bens imoveis proprios e de terceiros",
    );
    assert!(
        score_loc >= 0.80,
        "Paridade RapidFuzz: 'receita bruta locacao' deve ser >= 0.80, obtido: {}",
        score_loc
    );

    let score_prod =
        ReceitaExtractor::partial_ratio("vendas produtos", "venda de produtos industriais");
    assert!(
        score_prod >= 0.80,
        "Paridade RapidFuzz: 'vendas produtos' vs 'venda de produtos industriais' deve ser >= 0.80, obtido: {}",
        score_prod
    );
}

#[test]
fn test_extrair_receitas_paridade_python() {
    let extractor = ReceitaExtractor::new();

    let linhas = vec![
        LinhaExtracaoInput::new(
            "00000000000191",
            "3.1",
            "RECEITA BRUTA DE VENDAS",
            Decimal::from(100000),
        ),
        LinhaExtracaoInput::new(
            "00000000000191",
            "3.2",
            "RECEITA OPERACIONAL DE LOCACAO",
            Decimal::from(25000),
        ),
        LinhaExtracaoInput::new(
            "00000000000191",
            "3.3",
            "OUTRAS DESPESAS GERAIS",
            Decimal::from(5000),
        ),
    ];

    let extraidas = extractor.extrair_receitas(&linhas, None);
    assert_eq!(extraidas.len(), 2);
    assert_eq!(extraidas[0].cod_agl, "3.1");
    assert_eq!(extraidas[0].valor, Decimal::from(100000));
    assert_eq!(extraidas[1].cod_agl, "3.2");
    assert_eq!(extraidas[1].valor, Decimal::from(25000));
}

#[test]
fn test_filtra_receitas_j150_completo_com_substituicao() {
    let extractor = ReceitaExtractor::new();
    let cnpj = "12345678000195";

    let r_valido = RegJ150 {
        id: Some(1),
        nu_ordem: 1,
        cod_agl: Some("3.01".into()),
        ind_cod_agl: "T".into(),
        nivel_agl: 2,
        cod_agl_sup: None,
        descr_cod_agl: "RECEITA BRUTA DE VENDAS DE PRODUTOS".into(),
        vl_cta_ini: Decimal::ZERO,
        ind_dc_cta_ini: Some("C".into()),
        vl_cta_fin: Decimal::from(500000),
        ind_dc_cta_fin: "C".into(),
        ind_grp_dre: "R".into(),
        nota_exp_ref: None,
        cnpj: cnpj.into(),
        dt_ini: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        dt_fim: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        hash_arquivo: "HASH_ATIVO".into(),
    };

    let r_menor = RegJ150 {
        id: Some(2),
        nu_ordem: 2,
        cod_agl: Some("3.02".into()),
        ind_cod_agl: "T".into(),
        nivel_agl: 2,
        cod_agl_sup: None,
        descr_cod_agl: "RECEITA OPERACIONAL BRUTA".into(),
        vl_cta_ini: Decimal::ZERO,
        ind_dc_cta_ini: Some("C".into()),
        vl_cta_fin: Decimal::from(150000),
        ind_dc_cta_fin: "C".into(),
        ind_grp_dre: "R".into(),
        nota_exp_ref: None,
        cnpj: cnpj.into(),
        dt_ini: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        dt_fim: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        hash_arquivo: "HASH_ATIVO".into(),
    };

    let r_substituido = RegJ150 {
        id: Some(3),
        nu_ordem: 3,
        cod_agl: Some("3.01".into()),
        ind_cod_agl: "T".into(),
        nivel_agl: 2,
        cod_agl_sup: None,
        descr_cod_agl: "RECEITA BRUTA DE VENDAS DE PRODUTOS".into(),
        vl_cta_ini: Decimal::ZERO,
        ind_dc_cta_ini: Some("C".into()),
        vl_cta_fin: Decimal::from(900000),
        ind_dc_cta_fin: "C".into(),
        ind_grp_dre: "R".into(),
        nota_exp_ref: None,
        cnpj: cnpj.into(),
        dt_ini: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        dt_fim: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        hash_arquivo: "HASH_SUBSTITUIDO".into(),
    };

    let mut hashes_sub = HashSet::new();
    hashes_sub.insert("HASH_SUBSTITUIDO".to_string());

    let registros = vec![r_menor, r_valido, r_substituido];

    let filtrados = extractor.filtra_receitas_j150(cnpj, &registros, Some(&hashes_sub));

    assert_eq!(filtrados.len(), 2);
    // ORDER BY 4 DESC: primeiro deve ser 500.000, depois 150.000
    assert_eq!(filtrados[0].valor, Decimal::from(500000));
    assert_eq!(filtrados[1].valor, Decimal::from(150000));
}

#[test]
fn test_apuracao_consolidada_dre_e_i355() {
    let extractor = ReceitaExtractor::new();
    let cnpj = "98765432000100";

    let reg_j150 = vec![
        RegJ150 {
            id: Some(1),
            nu_ordem: 1,
            cod_agl: Some("3.1".into()),
            ind_cod_agl: "T".into(),
            nivel_agl: 1,
            cod_agl_sup: None,
            descr_cod_agl: "RECEITA BRUTA OPERACIONAL".into(),
            vl_cta_ini: Decimal::ZERO,
            ind_dc_cta_ini: Some("C".into()),
            vl_cta_fin: Decimal::from(1200000),
            ind_dc_cta_fin: "C".into(),
            ind_grp_dre: "R".into(),
            nota_exp_ref: None,
            cnpj: cnpj.into(),
            dt_ini: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            dt_fim: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
            hash_arquivo: "H1".into(),
        },
        RegJ150 {
            id: Some(2),
            nu_ordem: 2,
            cod_agl: Some("3.1.1".into()),
            ind_cod_agl: "D".into(),
            nivel_agl: 2,
            cod_agl_sup: Some("3.1".into()),
            descr_cod_agl: "RECEITA BRUTA DE SERVICOS".into(),
            vl_cta_ini: Decimal::ZERO,
            ind_dc_cta_ini: Some("C".into()),
            vl_cta_fin: Decimal::from(800000),
            ind_dc_cta_fin: "C".into(),
            ind_grp_dre: "R".into(),
            nota_exp_ref: None,
            cnpj: cnpj.into(),
            dt_ini: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            dt_fim: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
            hash_arquivo: "H1".into(),
        },
        RegJ150 {
            id: Some(3),
            nu_ordem: 3,
            cod_agl: Some("3.1.2".into()),
            ind_cod_agl: "D".into(),
            nivel_agl: 2,
            cod_agl_sup: Some("3.1".into()),
            descr_cod_agl: "RECEITA BRUTA DE LOCACAO".into(),
            vl_cta_ini: Decimal::ZERO,
            ind_dc_cta_ini: Some("C".into()),
            vl_cta_fin: Decimal::from(400000),
            ind_dc_cta_fin: "C".into(),
            ind_grp_dre: "R".into(),
            nota_exp_ref: None,
            cnpj: cnpj.into(),
            dt_ini: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            dt_fim: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
            hash_arquivo: "H1".into(),
        },
    ];

    let apuracao = extractor.apurar_consolidado(&reg_j150, &[]);
    assert_eq!(apuracao.receita_bruta, Decimal::from(1200000));
    assert_eq!(apuracao.receita_servico, Decimal::from(800000));
    assert_eq!(apuracao.outras_receitas, Decimal::from(400000));
    assert_eq!(apuracao.receita_acumulada, Decimal::from(1200000));
    assert_eq!(apuracao.linhas_identificadas.len(), 3);
}

#[test]
fn test_apuracao_i355_fallback_quando_j150_ausente() {
    let extractor = ReceitaExtractor::new();
    let cnpj = "11222333000144";

    let reg_i355 = vec![
        RegI355 {
            id: uuid::Uuid::new_v4(),
            cnpj: cnpj.into(),
            cod_cta: "4.1.1.01".into(),
            nome_cta_referencia: Some("RECEITA DE PRESTACAO DE SERVICOS".into()),
            cod_ccus: None,
            vl_cta: Decimal::from(300000),
            ind_dc: "C".into(),
            vl_cta_mf: None,
            ind_dc_mf: None,
            dt_res: NaiveDate::from_ymd_opt(2023, 6, 30).unwrap(),
            cod_agl: Some("4.1".into()),
            cod_cta_ref: Some("3.01.01.01.01.06".into()),
            nome_aglutinador: None,
            hash_arquivo: "H_I355".into(),
        },
        RegI355 {
            id: uuid::Uuid::new_v4(),
            cnpj: cnpj.into(),
            cod_cta: "4.1.2.01".into(),
            nome_cta_referencia: Some("RECEITA FINANCEIRA E OUTRAS".into()),
            cod_ccus: None,
            vl_cta: Decimal::from(50000),
            ind_dc: "C".into(),
            vl_cta_mf: None,
            ind_dc_mf: None,
            dt_res: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
            cod_agl: Some("4.2".into()),
            cod_cta_ref: Some("3.01.01.01.01.08".into()),
            nome_aglutinador: None,
            hash_arquivo: "H_I355".into(),
        },
    ];

    let apuracao = extractor.apurar_consolidado(&[], &reg_i355);
    assert_eq!(apuracao.receita_servico, Decimal::from(300000));
    assert_eq!(apuracao.outras_receitas, Decimal::from(50000));
    assert_eq!(apuracao.receita_bruta, Decimal::from(350000));
    assert_eq!(apuracao.receita_acumulada, Decimal::from(350000));
}

#[test]
fn test_casos_de_borda_entradas_vazias_e_nulas() {
    let extractor = ReceitaExtractor::new();

    // Strings vazias ou apenas espaços
    assert_eq!(ReceitaExtractor::partial_ratio("", ""), 0.0);
    assert_eq!(ReceitaExtractor::partial_ratio("receita", ""), 0.0);
    assert_eq!(ReceitaExtractor::partial_ratio("", "receita"), 0.0);
    assert_eq!(ReceitaExtractor::normalizar("   "), "");
    assert_eq!(ReceitaExtractor::normalizar("!@#$%^&*()"), "");
    assert!(extractor.avaliar_linha("").is_none());
    assert!(extractor.avaliar_linha("   ").is_none());
    assert!(extractor.avaliar_linha("!@#$").is_none());

    // Lista vazia para apuração
    let apuracao = extractor.apurar_consolidado(&[], &[]);
    assert_eq!(apuracao.receita_bruta, Decimal::ZERO);
    assert_eq!(apuracao.receita_servico, Decimal::ZERO);
    assert_eq!(apuracao.outras_receitas, Decimal::ZERO);
    assert_eq!(apuracao.receita_acumulada, Decimal::ZERO);
    assert!(apuracao.linhas_identificadas.is_empty());
}

#[test]
fn test_casos_de_borda_valores_monetarios_extremos() {
    let extractor = ReceitaExtractor::new();
    let cnpj = "55555555000155";

    // Valor astronômico (bilhões com centavos)
    let valor_bilhao = Decimal::from_str_exact("123456789012.75").unwrap();

    let reg_j150 = vec![RegJ150 {
        id: Some(10),
        nu_ordem: 1,
        cod_agl: Some("3.1".into()),
        ind_cod_agl: "T".into(),
        nivel_agl: 1,
        cod_agl_sup: None,
        descr_cod_agl: "RECEITA BRUTA DE VENDAS".into(),
        vl_cta_ini: Decimal::ZERO,
        ind_dc_cta_ini: Some("C".into()),
        vl_cta_fin: valor_bilhao,
        ind_dc_cta_fin: "C".into(),
        ind_grp_dre: "R".into(),
        nota_exp_ref: None,
        cnpj: cnpj.into(),
        dt_ini: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
        dt_fim: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        hash_arquivo: "HBIG".into(),
    }];

    let apuracao = extractor.apurar_consolidado(&reg_j150, &[]);
    assert_eq!(apuracao.receita_bruta, valor_bilhao);
    assert_eq!(apuracao.receita_acumulada, valor_bilhao);
}

#[test]
fn test_todos_os_12_filtros_negativos_individualmente_em_j150_e_i355() {
    let extractor = ReceitaExtractor::new();

    let filtros = [
        "RECEITA COM PARTE NAO OPERACIONAL",
        "RECEITA COM PARTE NÃO OPERACIONAL",
        "RECEITA BRUTA COM CUSTO ATRELADO",
        "RECEITA BRUTA COM DEDUCOES",
        "RECEITA BRUTA COM IMPOSTOS RETIDOS",
        "RECEITA BRUTA COM DESPESAS DIRETAS",
        "RECEITA BRUTA CONTRAPARTIDA",
        "RECEITA BRUTA SERVICOS DE TERCEIROS",
        "RECEITA BRUTA CUSTEIO DE PESSOAL",
        "RECEITA BRUTA CLIENTE PJ EXTERNO",
        "RECEITA BRUTA LIQUIDA",
        "RECEITA BRUTA ANTES DO IRPJ",
    ];

    for desc in &filtros {
        assert!(
            ReceitaExtractor::contem_filtro_negativo(desc),
            "Filtro negativo deve barrar: {}",
            desc
        );
        assert!(
            !extractor.elegivel_receita_j150("C", Decimal::from(100), "T", desc),
            "J150 não pode ser elegível para: {}",
            desc
        );
        assert!(
            !extractor.elegivel_receita_i355("C", Decimal::from(100), desc),
            "I355 não pode ser elegível para: {}",
            desc
        );
    }
}

#[test]
fn test_elegibilidade_i355_revenda_mercadorias_terceiros() {
    let extractor = ReceitaExtractor::new();

    // Venda de mercadorias de terceiros (comércio varejista) é receita e NÃO deve ser barrada
    let desc_revenda = "Venda de mercadorias adquiridas de terceiros no mercado interno";
    assert!(
        !ReceitaExtractor::contem_filtro_negativo(desc_revenda),
        "Revenda de mercadorias de terceiros não pode ser descartada como despesa"
    );
    assert!(
        extractor.elegivel_receita_i355("C", Decimal::from(150000), desc_revenda),
        "I355 deve aceitar receita de venda de mercadorias de terceiros"
    );

    // Já 'serviços de terceiros' é despesa e deve ser barrada
    let desc_despesa = "Serviços prestados por terceiros PJ";
    assert!(
        ReceitaExtractor::contem_filtro_negativo(desc_despesa),
        "Serviços de terceiros deve ser barrado pelo filtro negativo"
    );
    assert!(!extractor.elegivel_receita_i355("C", Decimal::from(150000), desc_despesa));
}

#[test]
fn test_consistencia_receita_bruta_soma_componentes() {
    let extractor = ReceitaExtractor::new();
    let cnpj = "11112222000133";

    // Cenário onde o totalizador informou 1.000.000 mas a soma de serviços (800k) e outras receitas (400k) é 1.200.000
    let reg_j150 = vec![
        RegJ150 {
            id: Some(1),
            nu_ordem: 1,
            cod_agl: Some("3.1".into()),
            ind_cod_agl: "T".into(),
            nivel_agl: 1,
            cod_agl_sup: None,
            descr_cod_agl: "RECEITA BRUTA TOTALIZADORA".into(),
            vl_cta_ini: Decimal::ZERO,
            ind_dc_cta_ini: Some("C".into()),
            vl_cta_fin: Decimal::from(1000000),
            ind_dc_cta_fin: "C".into(),
            ind_grp_dre: "R".into(),
            nota_exp_ref: None,
            cnpj: cnpj.into(),
            dt_ini: None,
            dt_fim: None,
            hash_arquivo: "H_CONSIST".into(),
        },
        RegJ150 {
            id: Some(2),
            nu_ordem: 2,
            cod_agl: Some("3.1.1".into()),
            ind_cod_agl: "D".into(),
            nivel_agl: 2,
            cod_agl_sup: Some("3.1".into()),
            descr_cod_agl: "RECEITA BRUTA DE SERVICOS".into(),
            vl_cta_ini: Decimal::ZERO,
            ind_dc_cta_ini: Some("C".into()),
            vl_cta_fin: Decimal::from(800000),
            ind_dc_cta_fin: "C".into(),
            ind_grp_dre: "R".into(),
            nota_exp_ref: None,
            cnpj: cnpj.into(),
            dt_ini: None,
            dt_fim: None,
            hash_arquivo: "H_CONSIST".into(),
        },
        RegJ150 {
            id: Some(3),
            nu_ordem: 3,
            cod_agl: Some("3.1.2".into()),
            ind_cod_agl: "D".into(),
            nivel_agl: 2,
            cod_agl_sup: Some("3.1".into()),
            descr_cod_agl: "RECEITA BRUTA DE LOCACAO".into(),
            vl_cta_ini: Decimal::ZERO,
            ind_dc_cta_ini: Some("C".into()),
            vl_cta_fin: Decimal::from(400000),
            ind_dc_cta_fin: "C".into(),
            ind_grp_dre: "R".into(),
            nota_exp_ref: None,
            cnpj: cnpj.into(),
            dt_ini: None,
            dt_fim: None,
            hash_arquivo: "H_CONSIST".into(),
        },
    ];

    let ap = extractor.apurar_j150(&reg_j150);
    // Consistência contábil: receita bruta não pode ser menor que a soma dos seus componentes
    assert_eq!(ap.receita_servico, Decimal::from(800000));
    assert_eq!(ap.outras_receitas, Decimal::from(400000));
    assert_eq!(ap.receita_bruta, Decimal::from(1200000));
    assert_eq!(ap.receita_acumulada, Decimal::from(1200000));
}

#[test]
fn test_preservacao_linhas_identificadas_fallback_consolidado() {
    let extractor = ReceitaExtractor::new();
    let cnpj = "44445555000166";

    // J150 tem apenas receita bruta geral
    let reg_j150 = vec![RegJ150 {
        id: Some(1),
        nu_ordem: 1,
        cod_agl: Some("3.1".into()),
        ind_cod_agl: "T".into(),
        nivel_agl: 1,
        cod_agl_sup: None,
        descr_cod_agl: "RECEITA BRUTA OPERACIONAL".into(),
        vl_cta_ini: Decimal::ZERO,
        ind_dc_cta_ini: Some("C".into()),
        vl_cta_fin: Decimal::from(500000),
        ind_dc_cta_fin: "C".into(),
        ind_grp_dre: "R".into(),
        nota_exp_ref: None,
        cnpj: cnpj.into(),
        dt_ini: None,
        dt_fim: None,
        hash_arquivo: "H_FALLBACK".into(),
    }];

    // I355 tem detalhamento de serviços
    let reg_i355 = vec![RegI355 {
        id: uuid::Uuid::new_v4(),
        cnpj: cnpj.into(),
        cod_cta: "4.1.01".into(),
        nome_cta_referencia: Some("RECEITA DE SERVICOS DE ENGENHARIA".into()),
        cod_ccus: None,
        vl_cta: Decimal::from(350000),
        ind_dc: "C".into(),
        vl_cta_mf: None,
        ind_dc_mf: None,
        dt_res: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
        cod_agl: Some("4.1".into()),
        cod_cta_ref: Some("3.01.01.01.01.06".into()),
        nome_aglutinador: None,
        hash_arquivo: "H_FALLBACK".into(),
    }];

    let ap = extractor.apurar_consolidado(&reg_j150, &reg_i355);
    assert_eq!(ap.receita_bruta, Decimal::from(500000));
    assert_eq!(ap.receita_servico, Decimal::from(350000));
    // As linhas identificadas devem conter tanto a linha do J150 quanto a linha de serviços do I355 incorporada
    assert_eq!(ap.linhas_identificadas.len(), 2);
    assert!(ap
        .linhas_identificadas
        .iter()
        .any(|l| l.descricao == "RECEITA DE SERVICOS DE ENGENHARIA"));
}

#[test]
fn test_benchmark_performance_microsegundos() {
    let extractor = ReceitaExtractor::new();
    let descricoes = [
        "RECEITA BRUTA DE VENDAS DE MERCADORIAS E PRODUTOS",
        "RECEITA DE PRESTACAO DE SERVICOS TECNICOS",
        "RECEITA OPERACIONAL COM LOCACAO DE BENS",
        "TOTAL DA RECEITA OPERACIONAL BRUTA",
        "RECEITAS OPERACIONAIS DE MENSALIDADES ESCOLARES",
        "OUTRAS RECEITAS OPERACIONAIS E PATRIMONIAIS",
        "FATURAMENTO BRUTO COM ALUGUEL DE IMOVEIS",
        "INGRESSOS E RECEITAS DECORRENTES DE TAXAS",
        "VENDAS DE MERCADORIAS ADQUIRIDAS DE TERCEIROS",
        "RECEITA BRUTA DE VENDAS NO MERCADO INTERNO E EXTERNO",
    ];

    let start = std::time::Instant::now();
    for _ in 0..100 {
        for desc in &descricoes {
            let _ = extractor.avaliar_linha(desc);
        }
    }
    let elapsed = start.elapsed();
    let max_allowed = if cfg!(debug_assertions) { 500 } else { 50 };
    assert!(
        elapsed.as_millis() < max_allowed,
        "1.000 avaliações de receita levaram {}ms (esperado < {}ms)",
        elapsed.as_millis(),
        max_allowed
    );
}
