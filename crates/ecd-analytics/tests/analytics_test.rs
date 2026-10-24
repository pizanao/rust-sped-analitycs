use std::time::Duration;

use chrono::NaiveDate;
use ecd_analytics::{
    calcular_percentual, EmpresaRegistro, IndicadoresAggregator, IndicadoresCache,
    IndicadoresCacheConfig, IndicadoresFiltro, TipoIndicador,
};
use rust_decimal::Decimal;

#[test]
fn test_calculo_percentuais_e_arredondamento() {
    assert_eq!(calcular_percentual(0, 100), 0.0);
    assert_eq!(calcular_percentual(50, 0), 0.0);
    assert_eq!(calcular_percentual(0, 0), 0.0);
    assert_eq!(calcular_percentual(-5, 100), 0.0);

    // Divisões comuns com dízimas periódicas arredondadas a 2 casas
    assert_eq!(calcular_percentual(1, 3), 33.33);
    assert_eq!(calcular_percentual(2, 3), 66.67);
    assert_eq!(calcular_percentual(1, 6), 16.67);
    assert_eq!(calcular_percentual(100, 100), 100.0);
}

#[test]
fn test_agregacao_porte_empresarial() {
    let aggregator = IndicadoresAggregator::new();

    let empresas = vec![
        EmpresaRegistro::new().with_porte("01"),
        EmpresaRegistro::new().with_porte("01"),
        EmpresaRegistro::new().with_porte("01"),
        EmpresaRegistro::new().with_porte("03"),
        EmpresaRegistro::new().with_porte("05"),
        EmpresaRegistro::new().with_porte("00"),
    ];

    let resumo = aggregator.processar(&empresas);

    assert_eq!(resumo.total_empresas, 6);
    assert_eq!(resumo.porte.len(), 4);

    // O primeiro deve ser "01" com 3 empresas (50.0%)
    let top_porte = &resumo.porte[0];
    assert_eq!(top_porte.codigo.as_deref(), Some("01"));
    assert_eq!(top_porte.quantidade, 3);
    assert_eq!(top_porte.percentual, 50.0);
    assert!(top_porte.categoria.contains("Micro empresa"));

    // O segundo deve ter 1 empresa (16.67%)
    let segundo = &resumo.porte[1];
    assert_eq!(segundo.quantidade, 1);
    assert_eq!(segundo.percentual, 16.67);
}

#[test]
fn test_agregacao_cnae_top15_e_normalizacao() {
    let aggregator = IndicadoresAggregator::new().with_cnae_limit(2);

    let empresas = vec![
        EmpresaRegistro::new().with_cnae("47.11-3-02"), // 4711302
        EmpresaRegistro::new().with_cnae("4711302"),
        EmpresaRegistro::new().with_cnae("4711302"),
        EmpresaRegistro::new().with_cnae("6201501"), // Desenvolvimento de software
        EmpresaRegistro::new().with_cnae("6201501"),
        EmpresaRegistro::new().with_cnae("5611201"), // Restaurantes
    ];

    let resumo = aggregator.processar(&empresas);

    // Limitado a top 2 conforme configurado
    assert_eq!(resumo.cnae_top15.len(), 2);

    let top1 = &resumo.cnae_top15[0];
    assert_eq!(top1.codigo.as_deref(), Some("4711302"));
    assert_eq!(top1.quantidade, 3);
    assert_eq!(top1.percentual, 50.0);
    assert!(top1.categoria.contains("hipermercados"));

    let top2 = &resumo.cnae_top15[1];
    assert_eq!(top2.codigo.as_deref(), Some("6201501"));
    assert_eq!(top2.quantidade, 2);
    assert_eq!(top2.percentual, 33.33);
    assert!(top2.categoria.contains("programas de computador"));
}

#[test]
fn test_agregacao_regime_tributario() {
    let aggregator = IndicadoresAggregator::new();

    let mut emp_presumido = EmpresaRegistro::new();
    emp_presumido.regime_tributario = Some("Lucro Presumido".to_string());

    let mut emp_real = EmpresaRegistro::new();
    emp_real.regime_tributario = Some("Lucro Real".to_string());

    let mut emp_simples_expl = EmpresaRegistro::new();
    emp_simples_expl.opcao_simples = Some("S".to_string());

    let empresas = vec![
        EmpresaRegistro::new().with_mei("S"),
        EmpresaRegistro::new().with_mei("S"),
        EmpresaRegistro::new().with_mei("N"), // Fallback para Simples compatível com legado
        emp_simples_expl,
        emp_presumido,
        emp_real,
    ];

    let resumo = aggregator.processar(&empresas);

    let map_regimes: std::collections::HashMap<_, _> = resumo
        .regime_tributario
        .iter()
        .map(|r| (r.categoria.as_str(), r.quantidade))
        .collect();

    assert_eq!(map_regimes.get("MEI"), Some(&2));
    assert_eq!(map_regimes.get("Simples"), Some(&2));
    assert_eq!(map_regimes.get("Lucro Presumido"), Some(&1));
    assert_eq!(map_regimes.get("Lucro Real"), Some(&1));
}

#[test]
fn test_agregacao_capital_social_faixas_e_media() {
    let aggregator = IndicadoresAggregator::new();

    let empresas = vec![
        EmpresaRegistro::new().with_capital(Decimal::from(0)), // Faixa 0
        EmpresaRegistro::new(),                                // Sem capital => Faixa 0
        EmpresaRegistro::new().with_capital(Decimal::from(500)), // Faixa 1 (Até 1.000)
        EmpresaRegistro::new().with_capital(Decimal::from(5_000)), // Faixa 2 (1.001 a 10.000)
        EmpresaRegistro::new().with_capital(Decimal::from(50_000)), // Faixa 3 (10.001 a 100.000)
        EmpresaRegistro::new().with_capital(Decimal::from(500_000)), // Faixa 4 (100.001 a 1.000.000)
        EmpresaRegistro::new().with_capital(Decimal::from(2_000_000)), // Faixa 5 (> 1.000.000)
    ];

    let resumo = aggregator.processar(&empresas);

    assert_eq!(resumo.total_empresas, 7);

    // Valida presença das faixas
    let faixa0 = resumo
        .capital_social
        .iter()
        .find(|i| i.codigo.as_deref() == Some("0"));
    assert!(faixa0.is_some());
    assert_eq!(faixa0.unwrap().quantidade, 2);

    let faixa1 = resumo
        .capital_social
        .iter()
        .find(|i| i.codigo.as_deref() == Some("1"));
    assert!(faixa1.is_some());
    assert_eq!(faixa1.unwrap().quantidade, 1);
    assert_eq!(faixa1.unwrap().valor_medio, Some(500.0));

    let faixa5 = resumo
        .capital_social
        .iter()
        .find(|i| i.codigo.as_deref() == Some("5"));
    assert!(faixa5.is_some());
    assert_eq!(faixa5.unwrap().quantidade, 1);
    assert_eq!(faixa5.unwrap().valor_medio, Some(2_000_000.0));

    // Média geral = (500 + 5000 + 50000 + 500000 + 2000000) / 7 = 2555500 / 7 ≈ 365071.43
    assert_eq!(resumo.capital_social_medio, Some(365_071.43));
}

#[test]
fn test_agregacao_situacao_cadastral_e_filtros() {
    let aggregator = IndicadoresAggregator::new();

    let mut emp_ativa = EmpresaRegistro::new()
        .with_uf("MS")
        .with_municipio("Campo Grande");
    emp_ativa.situacao_cadastral = Some("02".to_string());

    let mut emp_baixada = EmpresaRegistro::new()
        .with_uf("MS")
        .with_municipio("Campo Grande");
    emp_baixada.situacao_cadastral = Some("08".to_string());

    let mut emp_sp = EmpresaRegistro::new()
        .with_uf("SP")
        .with_municipio("São Paulo");
    emp_sp.situacao_cadastral = Some("02".to_string());

    let empresas = vec![emp_ativa, emp_baixada, emp_sp];

    // Por padrão ignora empresas baixadas ("08")
    let resumo_padrao = aggregator.processar(&empresas);
    assert_eq!(resumo_padrao.total_empresas, 2);

    // Filtro por UF = MS
    let filtro_ms = IndicadoresFiltro::new().with_uf("MS");
    let resumo_ms = aggregator.processar_com_filtro(&empresas, &filtro_ms);
    assert_eq!(resumo_ms.total_empresas, 1);
    assert_eq!(resumo_ms.uf.len(), 1);
    assert_eq!(resumo_ms.uf[0].categoria, "MS");

    // Incluindo baixadas explicitamente
    let filtro_com_baixadas = IndicadoresFiltro::new().com_baixadas();
    let resumo_todas = aggregator.processar_com_filtro(&empresas, &filtro_com_baixadas);
    assert_eq!(resumo_todas.total_empresas, 3);
}

#[cfg(feature = "sqlite")]
#[test]
fn test_agregacao_direta_sqlite() -> Result<(), Box<dyn std::error::Error>> {
    let conn = rusqlite::Connection::open_in_memory()?;

    // Cria schema idêntico ao banco SQLite RFB / Turmalina
    conn.execute_batch(
        r#"
        CREATE TABLE estabelecimento (
            cnpj TEXT PRIMARY KEY,
            cnpj_basico TEXT,
            porte_empresa TEXT,
            situacao_cadastral TEXT,
            cnae_fiscal TEXT,
            uf TEXT,
            municipio TEXT
        );
        CREATE TABLE empresas (
            cnpj_basico TEXT PRIMARY KEY,
            porte_empresa TEXT,
            natureza_juridica TEXT,
            capital_social REAL
        );
        CREATE TABLE simples (
            cnpj_basico TEXT PRIMARY KEY,
            opcao_simples TEXT,
            opcao_mei TEXT
        );

        INSERT INTO estabelecimento VALUES 
            ('00000000000191', '00000000', '01', '02', '6201501', 'MS', 'Campo Grande'),
            ('00000001000100', '00000001', '03', '02', '4711302', 'MS', 'Dourados'),
            ('00000002000101', '00000002', '01', '08', '6201501', 'MS', 'Campo Grande');
        INSERT INTO empresas VALUES 
            ('00000000', '01', '2062', 50000.0),
            ('00000001', '03', '2135', 100000.0),
            ('00000002', '01', '2062', 10000.0);
        INSERT INTO simples VALUES 
            ('00000000', 'S', 'S'),
            ('00000001', 'S', 'N'),
            ('00000002', 'N', 'N');
        "#,
    )?;

    let aggregator = IndicadoresAggregator::new();
    let resumo = aggregator.processar_sqlite(&conn, None)?;

    // Empresa baixada ('08') deve ser filtrada por padrão
    assert_eq!(resumo.total_empresas, 2);
    assert_eq!(resumo.uf[0].categoria, "MS");
    assert_eq!(resumo.uf[0].quantidade, 2);

    Ok(())
}

#[tokio::test]
async fn test_cache_moka_hit_miss_e_invalidation() {
    let cache = IndicadoresCache::with_ttl(Duration::from_secs(60));
    let data_ref = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
    let chave = IndicadoresCache::gerar_chave_resumo(Some(data_ref), None);

    // 1. Cache Miss inicial
    assert!(cache.get_resumo(&chave).await.is_none());

    // 2. Criação e inserção de dados
    let aggregator = IndicadoresAggregator::new();
    let empresas = vec![EmpresaRegistro::new().with_porte("01")];
    let resumo = aggregator.processar(&empresas);

    cache.insert_resumo(chave.clone(), resumo.clone()).await;

    // 3. Cache Hit
    let cached = cache.get_resumo(&chave).await;
    assert!(cached.is_some());
    let hit = cached.unwrap();
    assert_eq!(hit.total_empresas, 1);
    assert_eq!(hit.porte[0].codigo.as_deref(), Some("01"));

    // 4. Invalidação Manual
    cache.invalidate(&chave).await;
    assert!(cache.get_resumo(&chave).await.is_none());
}

#[tokio::test]
async fn test_cache_moka_get_or_compute() {
    let cache = IndicadoresCache::default();
    let chave = "indicadores_teste_computar";

    let mut contador_computacoes = 0;

    // Primeira chamada computa
    let r1 = cache
        .get_or_compute_resumo(chave, || async {
            contador_computacoes += 1;
            let agg = IndicadoresAggregator::new();
            Ok::<_, String>(agg.processar(&[EmpresaRegistro::new().with_porte("01")]))
        })
        .await
        .unwrap();

    assert_eq!(contador_computacoes, 1);
    assert_eq!(r1.total_empresas, 1);

    // Segunda chamada deve vir do cache sem executar closure
    let r2 = cache
        .get_or_compute_resumo(chave, || async {
            contador_computacoes += 1;
            Ok::<_, String>(IndicadoresAggregator::new().processar(&[]))
        })
        .await
        .unwrap();

    assert_eq!(contador_computacoes, 1);
    assert_eq!(r2.total_empresas, 1);
}

#[tokio::test]
async fn test_cache_moka_ttl_expiracao() {
    // TTL ultracurto de 30 milissegundos para teste determinístico de expiração
    let cache = IndicadoresCache::new(IndicadoresCacheConfig {
        ttl: Duration::from_millis(30),
        max_capacity: 100,
    });

    let chave = "chave_temporaria_ttl".to_string();
    let agg = IndicadoresAggregator::new();
    let resumo = agg.processar(&[EmpresaRegistro::new()]);

    cache.insert_resumo(chave.clone(), resumo).await;
    assert!(cache.get_resumo(&chave).await.is_some());

    // Aguarda expiração do TTL
    tokio::time::sleep(Duration::from_millis(50)).await;
    cache.run_pending_tasks().await;

    // Deve estar expirado agora
    assert!(cache.get_resumo(&chave).await.is_none());
}

#[test]
fn test_geracao_chave_e_tipo_indicador() {
    let data = NaiveDate::from_ymd_opt(2026, 10, 24);

    assert_eq!(
        IndicadoresCache::gerar_chave_tipo(TipoIndicador::Porte, data),
        "indicadores_porte_2026-10-24"
    );
    assert_eq!(
        IndicadoresCache::gerar_chave_tipo(TipoIndicador::Cnae, None),
        "indicadores_cnae_atual"
    );
    assert_eq!(
        IndicadoresCache::gerar_chave_resumo(data, Some("MS")),
        "indicadores_resumo_2026-10-24_MS"
    );
}

#[test]
fn test_determinismo_hash_dados() {
    let agg = IndicadoresAggregator::new();
    let emp1 = vec![EmpresaRegistro::new().with_porte("01")];
    let emp2 = vec![EmpresaRegistro::new().with_porte("01")];

    let r1 = agg.processar(&emp1);
    let r2 = agg.processar(&emp2);

    assert!(r1.hash_dados.is_some());
    assert_eq!(r1.hash_dados, r2.hash_dados);
}

#[test]
fn test_agregacao_colecao_vazia() {
    let agg = IndicadoresAggregator::new();
    let resumo = agg.processar(&[]);

    assert_eq!(resumo.total_empresas, 0);
    assert_eq!(resumo.capital_social_medio, None);
    assert!(resumo.porte.is_empty());
    assert!(resumo.cnae_top15.is_empty());
    assert!(resumo.regime_tributario.is_empty());
    assert!(resumo.capital_social.is_empty());
    assert!(resumo.situacao_cadastral.is_empty());
    assert!(resumo.uf.is_empty());
    assert!(resumo.municipio.is_empty());
    assert!(resumo.natureza_juridica.is_empty());
    assert!(resumo.hash_dados.is_some());
}

#[test]
fn test_agregacao_valores_limites_capital() {
    let agg = IndicadoresAggregator::new();
    let cap_gigante = Decimal::from_str_exact("999999999999999999.99").unwrap();

    let empresas = vec![
        EmpresaRegistro::new().with_capital(cap_gigante),
        EmpresaRegistro::new().with_capital(Decimal::from(0)),
    ];

    let resumo = agg.processar(&empresas);
    assert_eq!(resumo.total_empresas, 2);

    let faixa5 = resumo
        .capital_social
        .iter()
        .find(|i| i.codigo.as_deref() == Some("5"));
    assert!(faixa5.is_some());
    assert_eq!(faixa5.unwrap().quantidade, 1);
    assert_eq!(faixa5.unwrap().percentual, 50.0);

    let faixa0 = resumo
        .capital_social
        .iter()
        .find(|i| i.codigo.as_deref() == Some("0"));
    assert!(faixa0.is_some());
    assert_eq!(faixa0.unwrap().quantidade, 1);
    assert_eq!(faixa0.unwrap().percentual, 50.0);
}

#[test]
fn test_agregacao_cnaes_invalidos_e_faltantes() {
    let agg = IndicadoresAggregator::new();
    let empresas = vec![
        EmpresaRegistro::new().with_cnae(""),        // Vazio
        EmpresaRegistro::new().with_cnae("123"),     // Curto (< 7 dígitos)
        EmpresaRegistro::new().with_cnae("ABC"),     // Não numérico
        EmpresaRegistro::new().with_cnae("9999999"), // Válido de 7 dígitos mas não catalogado
    ];

    let resumo = agg.processar(&empresas);
    assert_eq!(resumo.total_empresas, 4);
    // Apenas 1 CNAE com 7 dígitos válidos deve ser considerado
    assert_eq!(resumo.cnae_top15.len(), 1);
    assert_eq!(resumo.cnae_top15[0].codigo.as_deref(), Some("9999999"));
    assert!(resumo.cnae_top15[0]
        .categoria
        .contains("CNAE não identificado"));
}

#[tokio::test]
async fn test_cache_resumo_e_itens_isolados() {
    let cache = IndicadoresCache::default();

    let itens =
        vec![ecd_analytics::IndicadorItem::new("01 - Micro empresa", 10, 100.0).with_codigo("01")];

    let chave_itens = "indicadores_porte_teste";
    cache
        .insert_itens(chave_itens.to_string(), itens.clone())
        .await;

    let res = cache.get_itens(chave_itens).await;
    assert_eq!(res, Some(itens));

    // Invalidação global
    cache.invalidate_all();
    assert!(cache.get_itens(chave_itens).await.is_none());
}

#[tokio::test]
async fn test_cache_concorrencia() {
    let cache = std::sync::Arc::new(IndicadoresCache::default());
    let mut handles = Vec::new();

    for i in 0..10 {
        let cache_clone = cache.clone();
        let handle = tokio::spawn(async move {
            let chave = format!("chave_concorrente_{}", i % 3);
            let agg = IndicadoresAggregator::new();
            let resumo = agg.processar(&[EmpresaRegistro::new().with_porte("01")]);
            cache_clone.insert_resumo(chave.clone(), resumo).await;
            let _ = cache_clone.get_resumo(&chave).await;
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.unwrap();
    }

    cache.run_pending_tasks().await;
    assert!(cache.entry_count() > 0);
}

#[test]
fn test_porte_normalizacao_digitos_simples() {
    let agg = IndicadoresAggregator::new();

    // Testa portes de dígito único ("1", "3", "5", "0") e 2 dígitos ("01", "03", "05", "00")
    let empresas = vec![
        EmpresaRegistro::new().with_porte("1"), // deve virar "01" (Micro empresa)
        EmpresaRegistro::new().with_porte("01"), // "01"
        EmpresaRegistro::new().with_porte("3"), // "03" (EPP)
        EmpresaRegistro::new().with_porte("5"), // "05" (Demais)
        EmpresaRegistro::new().with_porte("0"), // "00" (Não informado)
    ];

    let resumo = agg.processar(&empresas);
    assert_eq!(resumo.total_empresas, 5);

    let me = resumo
        .porte
        .iter()
        .find(|p| p.codigo.as_deref() == Some("01"));
    assert!(me.is_some());
    assert_eq!(me.unwrap().quantidade, 2);
    assert_eq!(me.unwrap().percentual, 40.0);

    let epp = resumo
        .porte
        .iter()
        .find(|p| p.codigo.as_deref() == Some("03"));
    assert!(epp.is_some());
    assert_eq!(epp.unwrap().quantidade, 1);

    let demais = resumo
        .porte
        .iter()
        .find(|p| p.codigo.as_deref() == Some("05"));
    assert!(demais.is_some());
    assert_eq!(demais.unwrap().quantidade, 1);

    let nao_inf = resumo
        .porte
        .iter()
        .find(|p| p.codigo.as_deref() == Some("00"));
    assert!(nao_inf.is_some());
    assert_eq!(nao_inf.unwrap().quantidade, 1);
}

#[test]
fn test_filtro_por_cnae() {
    let agg = IndicadoresAggregator::new();

    let empresas = vec![
        EmpresaRegistro::new().with_cnae("6201501").with_porte("01"), // TI
        EmpresaRegistro::new().with_cnae("6202300").with_porte("01"), // TI
        EmpresaRegistro::new().with_cnae("4711302").with_porte("03"), // Hipermercado
        EmpresaRegistro::new().with_cnae("5611201").with_porte("01"), // Restaurante
    ];

    // Filtro exato por subclasse
    let filtro_exato = IndicadoresFiltro::new().with_cnae("6201-5/01");
    let res_exato = agg.processar_com_filtro(&empresas, &filtro_exato);
    assert_eq!(res_exato.total_empresas, 1);
    assert_eq!(res_exato.cnae_top15[0].codigo.as_deref(), Some("6201501"));

    // Filtro por divisão (prefixo 62 - Informação e Comunicação / TI)
    let filtro_divisao = IndicadoresFiltro::new().with_cnae("62");
    let res_divisao = agg.processar_com_filtro(&empresas, &filtro_divisao);
    assert_eq!(res_divisao.total_empresas, 2);
    assert_eq!(res_divisao.porte[0].quantidade, 2);
    assert_eq!(res_divisao.porte[0].percentual, 100.0);
}

#[test]
fn test_resumo_compatibilidade_frontend_e_django() {
    let agg = IndicadoresAggregator::new();
    let empresas = vec![
        EmpresaRegistro::new()
            .with_porte("01")
            .with_cnae("6201501")
            .with_mei("S"),
        EmpresaRegistro::new()
            .with_porte("03")
            .with_cnae("4711302")
            .with_simples("S"),
    ];

    let resumo = agg.processar(&empresas);

    // Campos primários e dual aliases para paridade React SPA + Django
    assert_eq!(resumo.distribuicao_porte, resumo.porte);
    assert_eq!(resumo.top_cnaes, resumo.cnae_top15);
    assert_eq!(resumo.regimes_tributarios, resumo.regime_tributario);

    // IndicadorItem: categoria e nome devem ser equivalentes
    assert_eq!(resumo.porte[0].categoria, resumo.porte[0].nome);
    assert_eq!(resumo.top_cnaes[0].categoria, resumo.top_cnaes[0].nome);

    // Validação de serialização JSON esperada pelo Frontend
    let json_val: serde_json::Value = serde_json::to_value(&resumo).unwrap();
    assert!(json_val.get("distribuicao_porte").is_some());
    assert!(json_val.get("top_cnaes").is_some());
    assert!(json_val.get("regimes_tributarios").is_some());
    assert!(json_val.get("total_receita_apurada").is_some());
    assert!(json_val.get("total_alertas_sancoes").is_some());

    let item_json = &json_val["top_cnaes"][0];
    assert!(item_json.get("nome").is_some());
    assert!(item_json.get("quantidade").is_some());
    assert!(item_json.get("percentual").is_some());
}

#[test]
fn test_resumo_to_indicadores_empresa_persistencia() {
    let agg = IndicadoresAggregator::new();
    let empresas = vec![EmpresaRegistro::new()
        .with_porte("01")
        .with_cnae("6201501")
        .with_capital(Decimal::from(50_000))
        .with_uf("MS")
        .with_municipio("Campo Grande")];

    let resumo = agg.processar(&empresas);
    let data_ref = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
    let registros_banco = resumo.to_indicadores_empresa(data_ref);

    assert!(!registros_banco.is_empty());
    for reg in &registros_banco {
        assert_eq!(reg.data_referencia, data_ref);
        assert!(reg.hash_dados.is_some());
        assert!(!reg.categoria.is_empty());
    }

    // Consulta por tipo
    let porte_itens = resumo.itens_por_tipo(TipoIndicador::Porte);
    assert_eq!(porte_itens.len(), 1);
    assert_eq!(porte_itens[0].codigo.as_deref(), Some("01"));
}

#[tokio::test]
async fn test_cache_get_or_compute_itens_e_chave_filtro() {
    let cache = IndicadoresCache::default();
    let chave = "indicadores_porte_compute_test";

    let mut execucoes = 0;
    let r1 = cache
        .get_or_compute_itens(chave, || async {
            execucoes += 1;
            Ok::<_, String>(vec![ecd_analytics::IndicadorItem::new(
                "01 - Micro empresa",
                5,
                100.0,
            )])
        })
        .await
        .unwrap();

    assert_eq!(execucoes, 1);
    assert_eq!(r1.len(), 1);

    // Segunda chamada recupera do cache
    let r2 = cache
        .get_or_compute_itens(chave, || async {
            execucoes += 1;
            Ok::<_, String>(vec![])
        })
        .await
        .unwrap();

    assert_eq!(execucoes, 1);
    assert_eq!(r2.len(), 1);

    // Teste de chave de filtro
    let data = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
    let filtro = IndicadoresFiltro::new()
        .with_data(data)
        .with_uf("MS")
        .with_cnae("6201501");
    let chave_filtro = IndicadoresCache::gerar_chave_filtro(&filtro);
    assert!(chave_filtro.contains("2026-10-24"));
    assert!(chave_filtro.contains("uf-MS"));
    assert!(chave_filtro.contains("cnae-6201501"));
}

#[test]
fn test_hash_determinismo_sha256() {
    let agg = IndicadoresAggregator::new();
    let emp = vec![EmpresaRegistro::new().with_porte("01")];

    let mut r1 = agg.processar(&emp);
    let hash1 = r1.hash_dados.clone().unwrap();

    // SHA-256 produz string hexadecimal de 64 caracteres
    assert_eq!(hash1.len(), 64);

    // Garantir que recomputar sobre o mesmo resumo é idempotente
    let hash2 = agg.calcular_hash(&r1);
    assert_eq!(hash1, hash2);

    r1.hash_dados = None;
    let hash3 = agg.calcular_hash(&r1);
    assert_eq!(hash1, hash3);
}

#[cfg(feature = "sqlite")]
#[test]
fn test_sqlite_capital_social_text_e_inteiro() -> Result<(), Box<dyn std::error::Error>> {
    let conn = rusqlite::Connection::open_in_memory()?;

    conn.execute_batch(
        r#"
        CREATE TABLE estabelecimento (
            cnpj TEXT PRIMARY KEY,
            cnpj_basico TEXT,
            porte_empresa TEXT,
            situacao_cadastral TEXT,
            cnae_fiscal TEXT,
            uf TEXT,
            municipio TEXT
        );
        CREATE TABLE empresas (
            cnpj_basico TEXT PRIMARY KEY,
            porte_empresa TEXT,
            natureza_juridica TEXT,
            capital_social ANY
        );
        CREATE TABLE simples (
            cnpj_basico TEXT PRIMARY KEY,
            opcao_simples TEXT,
            opcao_mei TEXT
        );

        INSERT INTO estabelecimento VALUES 
            ('0001', '0001', '1', '2', '6201501', 'MS', 'Campo Grande'),
            ('0002', '0002', '3', '02', '4711302', 'MS', 'Campo Grande'),
            ('0003', '0003', '05', '02', '6202300', 'SP', 'São Paulo');

        -- capital_social como TEXT com vírgula, INTEGER e REAL
        INSERT INTO empresas VALUES 
            ('0001', '1', '2062', '50.000,00'),
            ('0002', '3', '2135', 100000),
            ('0003', '05', '2046', 500000.50);

        INSERT INTO simples VALUES 
            ('0001', 'S', 'S'),
            ('0002', 'S', 'N'),
            ('0003', 'N', 'N');
        "#,
    )?;

    let agg = IndicadoresAggregator::new();
    let filtro_cnae = IndicadoresFiltro::new().with_cnae("62");
    let resumo = agg.processar_sqlite(&conn, Some(&filtro_cnae))?;

    // Filtradas apenas 2 empresas do setor 62 (TI)
    assert_eq!(resumo.total_empresas, 2);

    // Capital social médio calculado com sucesso a partir de TEXT e REAL
    assert!(resumo.capital_social_medio.is_some());
    // (50000 + 500000.50) / 2 = 275000.25
    assert_eq!(resumo.capital_social_medio, Some(275_000.25));

    Ok(())
}
