-- ==============================================================================
-- Migração Inicial: Schema Híbrido ECD Analytics (Preservação Core + Otimização)
-- ==============================================================================

-- 1. Extensões
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- 2. TABELAS DE APOIO E CONTROLE (OTIMIZADAS)

CREATE TABLE IF NOT EXISTS usuarios (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    nome VARCHAR(150) NOT NULL,
    senha_hash VARCHAR(255) NOT NULL,
    role VARCHAR(20) NOT NULL DEFAULT 'Analista',
    ativo BOOLEAN NOT NULL DEFAULT TRUE,
    criado_em TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ultimo_login TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS configuracoes_sistema (
    chave VARCHAR(100) PRIMARY KEY,
    valor JSONB NOT NULL,
    descricao TEXT,
    atualizado_em TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS importacoes_ecd (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hash_arquivo VARCHAR(40) UNIQUE NOT NULL,
    cnpj CHAR(14) NOT NULL,
    nome_arquivo_original VARCHAR(255) NOT NULL,
    caminho_armazenamento VARCHAR(500) NOT NULL,
    tamanho_bytes BIGINT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'Aguardando',
    linhas_processadas INTEGER NOT NULL DEFAULT 0,
    tempo_processamento_ms INTEGER NOT NULL DEFAULT 0,
    mensagem_erro TEXT,
    criado_em TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    atualizado_em TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_importacoes_cnpj ON importacoes_ecd(cnpj);
CREATE INDEX IF NOT EXISTS idx_importacoes_status ON importacoes_ecd(status);

CREATE TABLE IF NOT EXISTS resultados_exercicio (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cnpj CHAR(14) NOT NULL,
    razao_social VARCHAR(200) NOT NULL,
    periodo_inicial DATE NOT NULL,
    periodo_final DATE NOT NULL,
    cnpj_scp CHAR(14),
    receita_bruta DECIMAL(18,2) NOT NULL DEFAULT 0.00,
    receita_servico DECIMAL(18,2) NOT NULL DEFAULT 0.00,
    outras_receitas DECIMAL(18,2) NOT NULL DEFAULT 0.00,
    receita_acumulada DECIMAL(18,2) NOT NULL DEFAULT 0.00,
    total_nota DECIMAL(18,2) NOT NULL DEFAULT 0.00,
    hash_arquivo VARCHAR(40) NOT NULL,
    situacao VARCHAR(5) NOT NULL DEFAULT 'S',
    detalhes_j150 JSONB,
    detalhes_i355 JSONB,
    criado_em TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unq_resultado_cnpj_periodo UNIQUE (cnpj, periodo_inicial, periodo_final, cnpj_scp)
);

CREATE INDEX IF NOT EXISTS idx_resultados_cnpj ON resultados_exercicio(cnpj);
CREATE INDEX IF NOT EXISTS idx_resultados_hash ON resultados_exercicio(hash_arquivo);
CREATE INDEX IF NOT EXISTS idx_resultados_j150_gin ON resultados_exercicio USING GIN (detalhes_j150);

-- 3. TABELAS CONTÁBEIS INVIOLÁVEIS (LAYOUT OFICIAL SPED ECD / RFB)

CREATE TABLE IF NOT EXISTS core_registro0000 (
    id BIGSERIAL PRIMARY KEY,
    dt_ini DATE NOT NULL,
    dt_fim DATE NOT NULL,
    nome_razao VARCHAR(155) NOT NULL,
    cnpj CHAR(14) NOT NULL,
    uf CHAR(2) NOT NULL,
    ie VARCHAR(50),
    cod_mun BIGINT,
    im VARCHAR(20),
    ind_sit_esp INTEGER,
    ind_sit_ini_per INTEGER DEFAULT 0,
    ind_nire INTEGER DEFAULT 0,
    ind_fin_esc INTEGER DEFAULT 0,
    cod_hash_sub VARCHAR(40),
    ind_grande_porte INTEGER DEFAULT 0,
    tip_ecd INTEGER DEFAULT 0,
    cod_scp VARCHAR(14),
    ident_mf CHAR(1) DEFAULT 'N',
    ind_esc_cons CHAR(1) DEFAULT 'N',
    ind_centralizada INTEGER DEFAULT 0,
    ind_mudanc_pc INTEGER DEFAULT 0,
    cod_plan_ref VARCHAR(2),
    i990 BIGINT DEFAULT 0,
    j990 BIGINT DEFAULT 0,
    hash_arquivo_id VARCHAR(40) NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_reg0000_cnpj_hash ON core_registro0000(cnpj, hash_arquivo_id);

CREATE TABLE IF NOT EXISTS core_registroi010 (
    id BIGSERIAL PRIMARY KEY,
    ind_esc VARCHAR(2) NOT NULL,
    cod_ver_lc VARCHAR(4) NOT NULL,
    reg_0000_id BIGINT REFERENCES core_registro0000(id) ON DELETE CASCADE,
    hash_arquivo VARCHAR(40) NOT NULL
);

CREATE TABLE IF NOT EXISTS core_registroi030 (
    id BIGSERIAL PRIMARY KEY,
    num_ord BIGINT NOT NULL,
    nat_livr VARCHAR(80) NOT NULL,
    qtd_lin BIGINT NOT NULL,
    nome VARCHAR(100) NOT NULL,
    nire VARCHAR(20),
    cnpj CHAR(14) NOT NULL,
    dt_arq DATE,
    dt_arq_conv DATE,
    desc_mun VARCHAR(80),
    dt_ex_social DATE,
    reg_0000_id BIGINT REFERENCES core_registro0000(id) ON DELETE CASCADE,
    hash_arquivo VARCHAR(40) NOT NULL
);

CREATE TABLE IF NOT EXISTS core_regi050 (
    id VARCHAR(40) PRIMARY KEY, -- MD5 chave
    dt_alt DATE NOT NULL,
    cod_nat VARCHAR(2) NOT NULL,
    ind_cta CHAR(1) NOT NULL,
    nivel INTEGER NOT NULL,
    cod_cta VARCHAR(50) NOT NULL,
    cod_cta_sup VARCHAR(50),
    cta VARCHAR(255) NOT NULL,
    cta_pai VARCHAR(255),
    cnpj CHAR(14) NOT NULL,
    cod_ccus_51 VARCHAR(50),
    cod_cta_ref VARCHAR(50),
    nome_cta_referencia VARCHAR(255),
    cod_ccus_52 VARCHAR(50),
    cod_agl VARCHAR(50),
    hash_arquivo_id VARCHAR(40) NOT NULL,
    data_entrega TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_regi050_cnpj_cta ON core_regi050(cnpj, cod_cta);
CREATE INDEX IF NOT EXISTS idx_regi050_hash ON core_regi050(hash_arquivo_id);

CREATE TABLE IF NOT EXISTS core_registroi150 (
    id BIGSERIAL PRIMARY KEY,
    dt_ini DATE NOT NULL,
    dt_fim DATE NOT NULL,
    reg_0000_id BIGINT REFERENCES core_registro0000(id) ON DELETE CASCADE,
    hash_arquivo VARCHAR(40) NOT NULL
);

CREATE TABLE IF NOT EXISTS core_registroi155 (
    id BIGSERIAL PRIMARY KEY,
    cod_cta VARCHAR(50) NOT NULL,
    cod_ccus VARCHAR(50),
    vl_sld_ini DECIMAL(22,2) NOT NULL,
    ind_dc_ini CHAR(1) NOT NULL,
    vl_deb DECIMAL(22,2) NOT NULL,
    vl_cred DECIMAL(22,2) NOT NULL,
    vl_sld_fin DECIMAL(22,2) NOT NULL,
    ind_dc_fin CHAR(1) NOT NULL,
    reg_150_id BIGINT REFERENCES core_registroi150(id) ON DELETE CASCADE,
    hash_arquivo VARCHAR(40) NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_regi155_cta ON core_registroi155(cod_cta);
CREATE INDEX IF NOT EXISTS idx_regi155_hash ON core_registroi155(hash_arquivo);

CREATE TABLE IF NOT EXISTS core_regi355 (
    id VARCHAR(40) PRIMARY KEY,
    dt_res DATE NOT NULL,
    cod_cta VARCHAR(50) NOT NULL,
    cod_ccus VARCHAR(50),
    vl_cta DECIMAL(22,2) NOT NULL,
    ind_dc CHAR(1) NOT NULL,
    cnpj CHAR(14) NOT NULL,
    hash_arquivo VARCHAR(40) NOT NULL,
    data_entrega TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_regi355_cnpj ON core_regi355(cnpj);

CREATE TABLE IF NOT EXISTS core_registroj005 (
    id BIGSERIAL PRIMARY KEY,
    dt_ini DATE NOT NULL,
    dt_fim DATE NOT NULL,
    id_dem INTEGER NOT NULL,
    cab_dem TEXT,
    reg_0000_id BIGINT REFERENCES core_registro0000(id) ON DELETE CASCADE,
    hash_arquivo VARCHAR(40) NOT NULL,
    cnpj CHAR(14) NOT NULL
);

CREATE TABLE IF NOT EXISTS core_registroj100 (
    id BIGSERIAL PRIMARY KEY,
    cod_agl VARCHAR(50) NOT NULL,
    nivel_agl INTEGER NOT NULL,
    descr_cod_agl VARCHAR(255) NOT NULL,
    vl_cta DECIMAL(22,2) NOT NULL,
    ind_dc_cta CHAR(1) NOT NULL,
    vl_cta_ini DECIMAL(22,2) NOT NULL,
    ind_dc_cta_ini CHAR(1) NOT NULL,
    reg_005_id BIGINT REFERENCES core_registroj005(id) ON DELETE CASCADE,
    hash_arquivo VARCHAR(40) NOT NULL,
    cnpj CHAR(14) NOT NULL
);

CREATE TABLE IF NOT EXISTS core_regj150 (
    id VARCHAR(40) PRIMARY KEY,
    reg_005_id BIGINT,
    nu_ordem BIGINT,
    cod_agl VARCHAR(50) NOT NULL,
    nivel_agl INTEGER NOT NULL,
    descr_cod_agl VARCHAR(255) NOT NULL,
    vl_cta_fin DECIMAL(22,2) NOT NULL,
    ind_dc_cta_fin CHAR(1) NOT NULL,
    vl_cta_ini DECIMAL(22,2) NOT NULL,
    ind_dc_cta_ini CHAR(1) NOT NULL,
    ind_cod_agl CHAR(1) NOT NULL,
    hash_arquivo VARCHAR(40) NOT NULL,
    cnpj CHAR(14) NOT NULL,
    dt_ini DATE NOT NULL,
    dt_fim DATE NOT NULL,
    data_entrega TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_regj150_cnpj ON core_regj150(cnpj);
CREATE INDEX IF NOT EXISTS idx_regj150_hash ON core_regj150(hash_arquivo);
