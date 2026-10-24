/**
 * Definições de Tipos TypeScript para a aplicação ECD Analytics
 * Espelha fielmente as structs em ecd-domain (Rust) e endpoints do Axum/LangChain.
 */

export type StatusImportacao =
  | 'Aguardando'
  | 'EmProcessamento'
  | 'Sucesso'
  | 'Erro'
  | 'Substituida';

export interface ImportacaoEcd {
  id: string;
  hash_arquivo: string;
  cnpj: string;
  nome_arquivo_original: string;
  caminho_armazenamento: string;
  tamanho_bytes: number;
  status: StatusImportacao;
  linhas_processadas: number;
  tempo_processamento_ms: number;
  mensagem_erro?: string | null;
  criado_em: string;
  atualizado_em: string;
}

export interface JobStatusEvent {
  job_id: string;
  hash_arquivo: string;
  status: StatusImportacao;
  progresso_percentual: number;
  linhas_processadas: number;
  mensagem: string;
  timestamp: string;
}

// Modelos do SPED Contábil (Blocos 0000, J100, J150, I155)
export interface Registro0000 {
  id?: number;
  dt_ini: string;
  dt_fim: string;
  nome_razao: string;
  cnpj: string;
  uf: string;
  ie?: string | null;
  cod_mun?: number | null;
  im?: string | null;
  ind_sit_esp?: number | null;
  ind_sit_ini_per: number;
  ind_nire: number;
  ind_fin_esc: number;
  cod_hash_sub?: string | null;
  ind_grande_porte: number;
  tip_ecd: number;
  cod_scp?: string | null;
  ident_mf: string;
  ind_esc_cons: string;
  ind_centralizada: number;
  ind_mudanc_pc: number;
  cod_plan_ref?: string | null;
  i990: number;
  j990: number;
  hash_arquivo_id: string;
}

export interface RegistroJ100 {
  id?: number;
  cod_agl: string;
  ind_cod_agl: string;
  nivel_agl: number;
  cod_agl_sup?: string | null;
  ind_grp_bal: string;
  descr_cod_agl: string;
  vl_cta_ini: number;
  ind_dc_cta_ini: string;
  vl_cta_fin: number;
  ind_dc_cta_fin: string;
  nota_exp_ref?: string | null;
  cnpj: string;
  hash_arquivo_id?: string | null;
  reg_0000_id: number;
  reg_j005_id: number;
}

export interface RegJ150 {
  id?: number;
  nu_ordem: number;
  cod_agl?: string | null;
  ind_cod_agl: string;
  nivel_agl: number;
  cod_agl_sup?: string | null;
  descr_cod_agl: string;
  vl_cta: number;
  ind_dc_cta: string;
  vl_cta_ini?: number | null;
  ind_dc_cta_ini?: string | null;
  ind_grp_dre: string;
  nota_exp_ref?: string | null;
  cnpj: string;
  hash_arquivo_id: string;
  reg_0000_id: number;
  reg_j005_id: number;
}

export interface RegistroI155 {
  id?: number;
  cod_cta: string;
  cod_ccus?: string | null;
  vl_sld_ini: number;
  ind_dc_ini: string;
  vl_deb: number;
  vl_cred: number;
  vl_sld_fim: number;
  ind_dc_fim: string;
  reg_0000_id: number;
  reg_i150_id: number;
}

export interface ResultadoExercicio {
  id: string;
  cnpj: string;
  razao_social: string;
  periodo_inicial: string;
  periodo_final: string;
  cnpj_scp?: string | null;
  receita_bruta: number;
  receita_servico: number;
  outras_receitas: number;
  receita_acumulada: number;
  total_nota: number;
  hash_arquivo: string;
  cod_hash_sub?: string | null;
  situacao: string;
  detalhes_j150?: Record<string, unknown> | null;
  detalhes_i355?: Record<string, unknown> | null;
  criado_em: string;
}

export interface ConsultaCnpjResponse {
  cnpj: string;
  razao_social: string;
  situacao_cadastral?: string;
  cnae_principal?: string;
  reg_0000: Registro0000[];
  reg_j100: RegistroJ100[];
  reg_j150: RegJ150[];
  reg_i155: RegistroI155[];
  resultado_exercicio?: ResultadoExercicio | null;
}

// Tipos da Rede Societária GRC (Vis-Network)
export type SancaoTipo = 'PEP' | 'CEIS' | 'CEPIM' | 'CNEP' | 'PGFN';

export interface GrafoNode {
  id: string;
  label: string;
  nome: string;
  tipo: 'PJ' | 'PF';
  cpf_cnpj: string;
  sancoes?: SancaoTipo[];
  capital_social?: number;
  situacao_cadastral?: string;
  cnae_fiscal?: string;
  uf?: string;
  municipio?: string;
  camada?: number;
}

export interface GrafoEdge {
  id?: string;
  from: string;
  to: string;
  label: string;
  qualificacao?: string;
  percentual_capital?: number;
  data_entrada?: string;
}

export interface GrafoRedeResult {
  raiz: string;
  camadas: number;
  total_nos: number;
  total_arestas: number;
  nos: GrafoNode[];
  arestas: GrafoEdge[];
  alertas_sancoes: number;
}

export interface MenorCaminhoResult {
  origem: string;
  destino: string;
  distancia: number;
  caminho_nos: GrafoNode[];
  caminho_arestas: GrafoEdge[];
}

// Tipos de Indicadores Econômicos
export interface IndicadorItem {
  nome: string;
  quantidade: number;
  percentual: number;
  valor?: number;
}

export interface IndicadoresResumo {
  total_empresas: number;
  capital_social_medio: number;
  total_receita_apurada: number;
  total_alertas_sancoes: number;
  top_cnaes: IndicadorItem[];
  distribuicao_porte: IndicadorItem[];
  regimes_tributarios: IndicadorItem[];
}

// Tipos do Copiloto de IA e Gráficos Dinâmicos
export type ChartType = 'bar' | 'line' | 'pie' | 'area';

export interface ChartSpec {
  chart_type: ChartType;
  title: string;
  description?: string;
  x_key: string;
  y_keys: string[];
  colors?: string[];
  data: Record<string, string | number>[];
}

export interface AiChatMessage {
  id: string;
  sender: 'user' | 'assistant' | 'system';
  content: string;
  chart?: ChartSpec | null;
  timestamp: string;
  isStreaming?: boolean;
}

export interface AiChatResponse {
  mensagem: string;
  chart?: ChartSpec | null;
  ferramentas_executadas?: string[];
}
