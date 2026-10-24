-- ==============================================================================
-- Migração Inicial: Schema Híbrido Rust Sped Analytics (Preservação Core 100% + Otimização)
-- ==============================================================================

-- 1. Extensões
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- 2. TABELAS DE APOIO E CONTROLE (OTIMIZADAS - UUIDv7 gerado na aplicação via uuid::Uuid::now_v7())

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

CREATE TABLE IF NOT EXISTS plano_contas_base (
    codigo_conta VARCHAR(50) PRIMARY KEY,
    nome_conta VARCHAR(255),
    dt_ini VARCHAR(20),
    dt_fim VARCHAR(20),
    tipo VARCHAR(5),
    conta_pai VARCHAR(50),
    nivel INTEGER,
    natureza INTEGER,
    observacoes TEXT
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
    receita_bruta DECIMAL(22,2) NOT NULL DEFAULT 0.00,
    receita_servico DECIMAL(22,2) NOT NULL DEFAULT 0.00,
    outras_receitas DECIMAL(22,2) NOT NULL DEFAULT 0.00,
    receita_acumulada DECIMAL(22,2) NOT NULL DEFAULT 0.00,
    total_nota DECIMAL(22,2) NOT NULL DEFAULT 0.00,
    hash_arquivo VARCHAR(40) UNIQUE NOT NULL,
    cod_hash_sub VARCHAR(40),
    situacao VARCHAR(5) NOT NULL DEFAULT 'S',
    detalhes_j150 JSONB,
    detalhes_i355 JSONB,
    criado_em TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unq_resultado_cnpj_periodo UNIQUE NULLS NOT DISTINCT (cnpj, periodo_inicial, periodo_final, cnpj_scp)
);

CREATE INDEX IF NOT EXISTS idx_resultados_cnpj ON resultados_exercicio(cnpj);
CREATE INDEX IF NOT EXISTS idx_resultados_hash ON resultados_exercicio(hash_arquivo);
CREATE INDEX IF NOT EXISTS idx_resultados_j150_gin ON resultados_exercicio USING GIN (detalhes_j150);

-- ==============================================================================
-- 3. TABELAS CONTÁBEIS INVIOLÁVEIS (PARIDADE 100% COM APPS/CORE/MODELS.PY)
-- ==============================================================================

-- 3.1 Registro0000 (core_registro0000)
CREATE TABLE IF NOT EXISTS core_registro0000 (
    id BIGSERIAL PRIMARY KEY,
    dt_ini DATE NOT NULL,
    dt_fim DATE NOT NULL,
    nome_razao VARCHAR(155) NOT NULL,
    cnpj VARCHAR(14) NOT NULL,
    uf VARCHAR(2) NOT NULL,
    ie VARCHAR(50),
    cod_mun BIGINT,
    im VARCHAR(20),
    ind_sit_esp INTEGER,
    ind_sit_ini_per INTEGER NOT NULL DEFAULT 0,
    ind_nire INTEGER NOT NULL DEFAULT 0,
    ind_fin_esc INTEGER NOT NULL DEFAULT 0,
    cod_hash_sub VARCHAR(40),
    ind_grande_porte INTEGER NOT NULL DEFAULT 0,
    tip_ecd INTEGER NOT NULL DEFAULT 0,
    cod_scp VARCHAR(14),
    ident_mf VARCHAR(1) NOT NULL DEFAULT 'N',
    ind_esc_cons VARCHAR(1) NOT NULL DEFAULT 'N',
    ind_centralizada INTEGER NOT NULL DEFAULT 0,
    ind_mudanc_pc INTEGER NOT NULL DEFAULT 0,
    cod_plan_ref VARCHAR(2),
    i990 BIGINT NOT NULL DEFAULT 0,
    j990 BIGINT NOT NULL DEFAULT 0,
    hash_arquivo_id VARCHAR(40) NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_reg0000_cnpj_hash ON core_registro0000(cnpj, hash_arquivo_id);
CREATE INDEX IF NOT EXISTS idx_reg0000_cod_hash_sub ON core_registro0000(cod_hash_sub);

-- 3.2 RegistroI010 (core_registroi010)
CREATE TABLE IF NOT EXISTS core_registroi010 (
    id BIGSERIAL PRIMARY KEY,
    ind_esc VARCHAR(1) NOT NULL,
    cod_ver_lc VARCHAR(10) NOT NULL DEFAULT '9.00',
    reg_0000_id BIGINT NOT NULL REFERENCES core_registro0000(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_regi010_reg0000 ON core_registroi010(reg_0000_id);

-- 3.3 RegistroI030 (core_registroi030)
CREATE TABLE IF NOT EXISTS core_registroi030 (
    id BIGSERIAL PRIMARY KEY,
    num_ord INTEGER NOT NULL,
    nat_livr VARCHAR(90) NOT NULL,
    qtd_lin INTEGER NOT NULL,
    nome VARCHAR(150) NOT NULL,
    nire VARCHAR(20) NOT NULL,
    cnpj VARCHAR(14) NOT NULL,
    dt_arq DATE,
    dt_arq_conv DATE,
    desc_mun VARCHAR(55),
    dt_ex_social DATE NOT NULL,
    reg_0000_id BIGINT NOT NULL REFERENCES core_registro0000(id) ON DELETE CASCADE,
    hash_arquivo_id VARCHAR(40)
);

CREATE INDEX IF NOT EXISTS idx_regi030_reg0000 ON core_registroi030(reg_0000_id);

-- 3.4 RegistroI050 (core_registroi050)
CREATE TABLE IF NOT EXISTS core_registroi050 (
    id BIGSERIAL PRIMARY KEY,
    cod_cta VARCHAR(50) NOT NULL,
    cta VARCHAR(120) NOT NULL,
    nivel INTEGER NOT NULL,
    ind_cta VARCHAR(1) NOT NULL,
    cod_nat VARCHAR(2) NOT NULL,
    cod_cta_sup VARCHAR(50),
    dt_alt DATE NOT NULL,
    descr_cta VARCHAR(120) NOT NULL,
    descr_conta VARCHAR(120),
    codigo_conta VARCHAR(50),
    reg_0000_id BIGINT NOT NULL REFERENCES core_registro0000(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_core_regi050_cod_cta ON core_registroi050(cod_cta);
CREATE INDEX IF NOT EXISTS idx_core_regi050_reg0000 ON core_registroi050(reg_0000_id);

-- 3.5 RegistroI051 (core_registroi051)
CREATE TABLE IF NOT EXISTS core_registroi051 (
    id BIGSERIAL PRIMARY KEY,
    cod_ccus VARCHAR(50),
    cod_cta_ref VARCHAR(50) NOT NULL,
    reg_0000_id BIGINT NOT NULL REFERENCES core_registro0000(id) ON DELETE CASCADE,
    reg_i050_id BIGINT NOT NULL REFERENCES core_registroi050(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_regi051_cta_ref ON core_registroi051(cod_cta_ref);
CREATE INDEX IF NOT EXISTS idx_regi051_regi050 ON core_registroi051(reg_i050_id);

-- 3.6 RegistroI052 (core_registroi052)
CREATE TABLE IF NOT EXISTS core_registroi052 (
    id BIGSERIAL PRIMARY KEY,
    cod_ccus VARCHAR(50),
    cod_agl VARCHAR(150) NOT NULL,
    reg_0000_id BIGINT NOT NULL REFERENCES core_registro0000(id) ON DELETE CASCADE,
    reg_i050_id BIGINT REFERENCES core_registroi050(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_regi052_cod_agl ON core_registroi052(cod_agl);
CREATE INDEX IF NOT EXISTS idx_regi052_regi050 ON core_registroi052(reg_i050_id);

-- 3.7 RegistroI053 (core_registroi053)
CREATE TABLE IF NOT EXISTS core_registroi053 (
    id BIGSERIAL PRIMARY KEY,
    cod_idt VARCHAR(6) NOT NULL,
    cod_cnt_corr VARCHAR(50) NOT NULL,
    nat_sub_cnt VARCHAR(2) NOT NULL,
    reg_0000_id BIGINT NOT NULL REFERENCES core_registro0000(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_regi053_reg0000 ON core_registroi053(reg_0000_id);

-- 3.8 RegI050 Consolidado (db_table = regi050_plano_contas)
CREATE TABLE IF NOT EXISTS regi050_plano_contas (
    id UUID PRIMARY KEY,
    cod_cta VARCHAR(50) NOT NULL,
    cta VARCHAR(120) NOT NULL,
    nivel INTEGER NOT NULL,
    ind_cta VARCHAR(1) NOT NULL,
    cod_nat VARCHAR(2) NOT NULL,
    cod_cta_sup VARCHAR(50),
    dt_alt DATE NOT NULL,
    cod_ccus_51 VARCHAR(50),
    cod_cta_ref VARCHAR(60),
    cod_ccus_52 VARCHAR(50),
    cod_agl VARCHAR(60),
    cnpj VARCHAR(14) NOT NULL,
    data_entrega TIMESTAMPTZ NOT NULL,
    cta_pai VARCHAR(120),
    nome_cta_referencia VARCHAR(255),
    reg051_json JSONB,
    reg052_json JSONB,
    hash_arquivo VARCHAR(40) NOT NULL DEFAULT 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
);

CREATE INDEX IF NOT EXISTS idx_regi050_pc_cnpj_cta ON regi050_plano_contas(cnpj, cod_cta);
CREATE INDEX IF NOT EXISTS idx_regi050_pc_hash ON regi050_plano_contas(hash_arquivo);
CREATE INDEX IF NOT EXISTS idx_regi050_pc_cta_ref ON regi050_plano_contas(cod_cta_ref);
CREATE INDEX IF NOT EXISTS idx_regi050_pc_cod_agl ON regi050_plano_contas(cod_agl);

-- 3.9 RegistroI150 (core_registroi150)
CREATE TABLE IF NOT EXISTS core_registroi150 (
    id BIGSERIAL PRIMARY KEY,
    dt_ini DATE NOT NULL,
    dt_fim DATE NOT NULL,
    reg_0000_id BIGINT NOT NULL REFERENCES core_registro0000(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_regi150_reg0000 ON core_registroi150(reg_0000_id);

-- 3.10 RegistroI155 (core_registroi155)
CREATE TABLE IF NOT EXISTS core_registroi155 (
    id BIGSERIAL PRIMARY KEY,
    cod_cta VARCHAR(50) NOT NULL,
    cod_ccus VARCHAR(50),
    vl_sld_ini DECIMAL(19,2) NOT NULL,
    ind_dc_ini VARCHAR(1) NOT NULL,
    vl_deb DECIMAL(19,2) NOT NULL DEFAULT 0,
    vl_cred DECIMAL(19,2) NOT NULL DEFAULT 0,
    vl_sld_fim DECIMAL(19,2) NOT NULL DEFAULT 0,
    ind_dc_fim VARCHAR(1) NOT NULL,
    reg_0000_id BIGINT NOT NULL REFERENCES core_registro0000(id) ON DELETE CASCADE,
    reg_i150_id BIGINT NOT NULL REFERENCES core_registroi150(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_regi155_cta ON core_registroi155(cod_cta);
CREATE INDEX IF NOT EXISTS idx_regi155_reg0000 ON core_registroi155(reg_0000_id);
CREATE INDEX IF NOT EXISTS idx_regi155_regi150 ON core_registroi155(reg_i150_id);

-- 3.11 RegI355 (db_table = regi355_detalhes_contas_resultado)
CREATE TABLE IF NOT EXISTS regi355_detalhes_contas_resultado (
    id UUID PRIMARY KEY,
    cnpj VARCHAR(14) NOT NULL,
    cod_cta VARCHAR(150) NOT NULL,
    nome_cta_referencia VARCHAR(255),
    cod_ccus VARCHAR(50),
    vl_cta DECIMAL(19,2) NOT NULL DEFAULT 0,
    ind_dc VARCHAR(1) NOT NULL,
    vl_cta_mf DECIMAL(19,2) DEFAULT 0,
    ind_dc_mf VARCHAR(1),
    dt_res DATE NOT NULL,
    cod_agl VARCHAR(50),
    cod_cta_ref VARCHAR(60),
    nome_aglutinador VARCHAR(255),
    hash_arquivo VARCHAR(40) NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_regi355_cnpj ON regi355_detalhes_contas_resultado(cnpj);
CREATE INDEX IF NOT EXISTS idx_regi355_hash ON regi355_detalhes_contas_resultado(hash_arquivo);
CREATE INDEX IF NOT EXISTS idx_regi355_cod_agl ON regi355_detalhes_contas_resultado(cod_agl);

-- 3.12 RegistroJ005 (core_registroj005)
CREATE TABLE IF NOT EXISTS core_registroj005 (
    id BIGSERIAL PRIMARY KEY,
    dt_ini DATE NOT NULL,
    dt_fim DATE NOT NULL,
    id_dem INTEGER NOT NULL,
    cab_dem TEXT,
    cnpj VARCHAR(14) NOT NULL,
    hash_arquivo_id VARCHAR(40),
    reg_0000_id BIGINT NOT NULL REFERENCES core_registro0000(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_regj005_cnpj ON core_registroj005(cnpj);
CREATE INDEX IF NOT EXISTS idx_regj005_reg0000 ON core_registroj005(reg_0000_id);

-- 3.13 RegistroJ100 (core_registroj100)
CREATE TABLE IF NOT EXISTS core_registroj100 (
    id BIGSERIAL PRIMARY KEY,
    cod_agl VARCHAR(255) NOT NULL,
    ind_cod_agl VARCHAR(1) NOT NULL,
    nivel_agl INTEGER NOT NULL,
    cod_agl_sup VARCHAR(255),
    ind_grp_bal VARCHAR(1) NOT NULL,
    descr_cod_agl VARCHAR(255) NOT NULL,
    vl_cta_ini DECIMAL(22,2) NOT NULL,
    ind_dc_cta_ini VARCHAR(1) NOT NULL,
    vl_cta_fin DECIMAL(16,2) NOT NULL,
    ind_dc_cta_fin VARCHAR(1) NOT NULL,
    nota_exp_ref VARCHAR(25),
    cnpj VARCHAR(14) NOT NULL,
    hash_arquivo_id VARCHAR(40),
    reg_0000_id BIGINT NOT NULL REFERENCES core_registro0000(id) ON DELETE CASCADE,
    reg_j005_id BIGINT NOT NULL REFERENCES core_registroj005(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_regj100_cnpj ON core_registroj100(cnpj);
CREATE INDEX IF NOT EXISTS idx_regj100_hash ON core_registroj100(hash_arquivo_id);

-- 3.14 RegJ150 (db_table = regj150_demonstracao_resultado)
CREATE TABLE IF NOT EXISTS regj150_demonstracao_resultado (
    id BIGSERIAL PRIMARY KEY,
    nu_ordem BIGINT NOT NULL,
    cod_agl VARCHAR(60),
    ind_cod_agl VARCHAR(1) NOT NULL,
    nivel_agl INTEGER NOT NULL,
    cod_agl_sup VARCHAR(60),
    descr_cod_agl VARCHAR(255) NOT NULL,
    vl_cta_ini DECIMAL(22,2) NOT NULL DEFAULT 0.00,
    ind_dc_cta_ini VARCHAR(1),
    vl_cta_fin DECIMAL(22,2) NOT NULL DEFAULT 0.00,
    ind_dc_cta_fin VARCHAR(1) NOT NULL,
    ind_grp_dre VARCHAR(1) NOT NULL,
    nota_exp_ref VARCHAR(12),
    cnpj VARCHAR(14) NOT NULL,
    dt_ini DATE,
    dt_fim DATE,
    hash_arquivo VARCHAR(40) NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_regj150_cnpj ON regj150_demonstracao_resultado(cnpj);
CREATE INDEX IF NOT EXISTS idx_regj150_hash ON regj150_demonstracao_resultado(hash_arquivo);

-- 3.15 RegistroJ210 (core_registroj210)
CREATE TABLE IF NOT EXISTS core_registroj210 (
    id BIGSERIAL PRIMARY KEY,
    ind_tip VARCHAR(1) NOT NULL,
    cod_agl VARCHAR(255) NOT NULL,
    descr_cod_agl VARCHAR(255) NOT NULL,
    vl_cta_ini DECIMAL(22,2) NOT NULL,
    ind_dc_cta_ini VARCHAR(1) NOT NULL,
    vl_cta_fin DECIMAL(22,2) NOT NULL,
    ind_dc_cta_fin VARCHAR(1) NOT NULL,
    nota_exp_ref VARCHAR(25),
    cnpj VARCHAR(14) NOT NULL,
    hash_arquivo_id VARCHAR(40),
    reg_0000_id BIGINT REFERENCES core_registro0000(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_regj210_cnpj ON core_registroj210(cnpj);

-- 3.16 RegistroJ215 (core_registroj215)
CREATE TABLE IF NOT EXISTS core_registroj215 (
    id BIGSERIAL PRIMARY KEY,
    cod_hist_fat VARCHAR(255) NOT NULL,
    desc_fat VARCHAR(255) NOT NULL,
    vl_fat_cont DECIMAL(16,2) NOT NULL,
    ind_dc_fat VARCHAR(20) NOT NULL,
    cnpj VARCHAR(14) NOT NULL,
    hash_arquivo_id VARCHAR(40),
    reg_0000_id BIGINT REFERENCES core_registro0000(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_regj215_cnpj ON core_registroj215(cnpj);

-- 4. USUÁRIO INICIAL DE TESTES / HOMOLOGAÇÃO
-- Senha: SenhaForte@Auditor2026!
INSERT INTO usuarios (id, email, nome, senha_hash, role, ativo)
VALUES (
    '018f0000-0000-7000-8000-000000000001',
    'admin@ecd.gov.br',
    'Administrador de Testes',
    '$argon2id$v=19$m=19456,t=2,p=1$VuhvswdLRfSVXkaJIwyZyA$mdylnw2nFK75TPqI6lr1ME9aB80Wn2uuSa/y09XomDU',
    'Admin',
    TRUE
)
ON CONFLICT (email) DO NOTHING;

