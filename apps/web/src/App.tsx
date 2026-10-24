import { useState, useEffect } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useRouter, useRouterState } from '@tanstack/react-router';
import { AppLayout } from '@/components/layout/AppLayout';
import type { NavItem } from '@/components/layout/Sidebar';
import { FileDropzone } from '@/features/importacao/FileDropzone';
import { ImportJobsTable } from '@/features/importacao/ImportJobsTable';
import { LiveLogDrawer } from '@/features/importacao/LiveLogDrawer';
import { CnpjSearchBar } from '@/features/fiscal/CnpjSearchBar';
import { DreViewer } from '@/features/fiscal/DreViewer';
import { SpedBlocksTabs } from '@/features/fiscal/SpedBlocksTabs';
import { NetworkGraphCanvas } from '@/features/grc/NetworkGraphCanvas';
import { NodeDetailDrawer } from '@/features/grc/NodeDetailDrawer';
import { EconomicIndicatorsDashboard } from '@/features/indicadores/EconomicIndicatorsDashboard';
import { AiChatDrawer } from '@/features/copilot/AiChatDrawer';
import { Card, CardContent } from '@/components/ui/card';
import { Activity, Clock, ShieldCheck, FileSpreadsheet, Search } from 'lucide-react';
import { api } from '@/services/api';
import type {
  ImportacaoEcd,
  ConsultaCnpjResponse,
  GrafoNode,
  ResultadoExercicio,
} from '@/types';

const queryClient = new QueryClient();

// Dados de fallback iniciais para simulação offline
const INITIAL_MOCK_JOBS: ImportacaoEcd[] = [
  {
    id: 'job-petrobras-2023',
    hash_arquivo: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
    cnpj: '33000167000101',
    nome_arquivo_original: 'SPED_ECD_PETROBRAS_2023.txt',
    caminho_armazenamento: 'media/ecd_stage/processados/SPED_ECD_PETROBRAS_2023.txt',
    tamanho_bytes: 524288000,
    status: 'Sucesso',
    linhas_processadas: 74210,
    tempo_processamento_ms: 4820,
    criado_em: new Date(Date.now() - 3600000 * 2).toISOString(),
    atualizado_em: new Date().toISOString(),
  },
  {
    id: 'job-vale-2023',
    hash_arquivo: 'a591a6d40bf420404a011733cfb7b190d62c65bf0bcda32b57b277d9ad9f146e',
    cnpj: '33592510000154',
    nome_arquivo_original: 'SPED_ECD_VALE_2023.txt',
    caminho_armazenamento: 'media/ecd_stage/processados/SPED_ECD_VALE_2023.txt',
    tamanho_bytes: 312500000,
    status: 'Sucesso',
    linhas_processadas: 51200,
    tempo_processamento_ms: 3410,
    criado_em: new Date(Date.now() - 3600000 * 8).toISOString(),
    atualizado_em: new Date().toISOString(),
  },
  {
    id: 'job-holding-xyz',
    hash_arquivo: '5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8',
    cnpj: '00000000000191',
    nome_arquivo_original: 'SPED_ECD_HOLDING_2023_SUBST.txt',
    caminho_armazenamento: 'media/ecd_stage/processados/SPED_ECD_HOLDING_2023_SUBST.txt',
    tamanho_bytes: 48900000,
    status: 'Substituida',
    linhas_processadas: 8900,
    tempo_processamento_ms: 890,
    criado_em: new Date(Date.now() - 3600000 * 24).toISOString(),
    atualizado_em: new Date().toISOString(),
  },
];

const INITIAL_MOCK_FISCAL: ConsultaCnpjResponse = {
  cnpj: '33000167000101',
  razao_social: 'PETRÓLEO BRASILEIRO S.A. - PETROBRAS',
  situacao_cadastral: 'Ativa',
  cnae_principal: '19.21-7-00 - Fabricação de produtos do refino de petróleo',
  resultado_exercicio: {
    id: 'res-petrobras-2023',
    cnpj: '33000167000101',
    razao_social: 'PETRÓLEO BRASILEIRO S.A. - PETROBRAS',
    periodo_inicial: '2023-01-01',
    periodo_final: '2023-12-31',
    receita_bruta: 48250000.0,
    receita_servico: 35120000.0,
    outras_receitas: 13130000.0,
    receita_acumulada: 48250000.0,
    total_nota: 47980000.0,
    hash_arquivo: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
    situacao: 'Escrituração Regular',
    criado_em: new Date().toISOString(),
  },
  reg_0000: [
    {
      nome_razao: 'PETRÓLEO BRASILEIRO S.A. - PETROBRAS',
      cnpj: '33000167000101',
      uf: 'RJ',
      dt_ini: '2023-01-01',
      dt_fim: '2023-12-31',
      ind_sit_ini_per: 0,
      ind_nire: 1,
      ind_fin_esc: 0,
      ind_grande_porte: 1,
      tip_ecd: 0,
      ident_mf: 'N',
      ind_esc_cons: 'N',
      ind_centralizada: 1,
      ind_mudanc_pc: 0,
      i990: 1250,
      j990: 480,
      hash_arquivo_id: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
    },
  ],
  reg_j100: [
    {
      cod_agl: '1.01',
      descr_cod_agl: 'ATIVO CIRCULANTE',
      ind_cod_agl: 'T',
      nivel_agl: 2,
      ind_grp_bal: 'A',
      vl_cta_ini: 65420000.0,
      ind_dc_cta_ini: 'D',
      vl_cta_fin: 78150000.0,
      ind_dc_cta_fin: 'D',
      cnpj: '33000167000101',
      reg_0000_id: 1,
      reg_j005_id: 1,
    },
    {
      cod_agl: '1.01.01',
      descr_cod_agl: 'Caixa e Equivalentes de Caixa',
      ind_cod_agl: 'A',
      nivel_agl: 3,
      ind_grp_bal: 'A',
      vl_cta_ini: 24100000.0,
      ind_dc_cta_ini: 'D',
      vl_cta_fin: 31200000.0,
      ind_dc_cta_fin: 'D',
      cnpj: '33000167000101',
      reg_0000_id: 1,
      reg_j005_id: 1,
    },
    {
      cod_agl: '2.01',
      descr_cod_agl: 'PASSIVO CIRCULANTE',
      ind_cod_agl: 'T',
      nivel_agl: 2,
      ind_grp_bal: 'P',
      vl_cta_ini: 41200000.0,
      ind_dc_cta_ini: 'C',
      vl_cta_fin: 45800000.0,
      ind_dc_cta_fin: 'C',
      cnpj: '33000167000101',
      reg_0000_id: 1,
      reg_j005_id: 1,
    },
    {
      cod_agl: '2.03',
      descr_cod_agl: 'PATRIMÔNIO LÍQUIDO CONSOLIDADO',
      ind_cod_agl: 'T',
      nivel_agl: 2,
      ind_grp_bal: 'P',
      vl_cta_ini: 180000000.0,
      ind_dc_cta_ini: 'C',
      vl_cta_fin: 205431870.0,
      ind_dc_cta_fin: 'C',
      cnpj: '33000167000101',
      reg_0000_id: 1,
      reg_j005_id: 1,
    },
  ],
  reg_j150: [
    {
      nu_ordem: 1,
      cod_agl: '3.01.01.01',
      descr_cod_agl: 'RECEITA BRUTA DAS VENDAS E SERVICOS',
      ind_cod_agl: 'T',
      nivel_agl: 4,
      vl_cta: 48250000.0,
      ind_dc_cta: 'C',
      ind_grp_dre: 'R',
      cnpj: '33000167000101',
      hash_arquivo_id: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
      reg_0000_id: 1,
      reg_j005_id: 1,
    },
    {
      nu_ordem: 2,
      cod_agl: '3.01.01.01.01',
      descr_cod_agl: 'Receita de Venda de Combustíveis e Lubrificantes',
      ind_cod_agl: 'A',
      nivel_agl: 5,
      vl_cta: 35120000.0,
      ind_dc_cta: 'C',
      ind_grp_dre: 'R',
      cnpj: '33000167000101',
      hash_arquivo_id: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
      reg_0000_id: 1,
      reg_j005_id: 1,
    },
    {
      nu_ordem: 3,
      cod_agl: '3.01.01.01.02',
      descr_cod_agl: 'Receita de Serviços de Transporte e Afretamento',
      ind_cod_agl: 'A',
      nivel_agl: 5,
      vl_cta: 13130000.0,
      ind_dc_cta: 'C',
      ind_grp_dre: 'R',
      cnpj: '33000167000101',
      hash_arquivo_id: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
      reg_0000_id: 1,
      reg_j005_id: 1,
    },
    {
      nu_ordem: 4,
      cod_agl: '3.01.02.01',
      descr_cod_agl: '(-) Deduções da Receita Bruta (Impostos Faturados)',
      ind_cod_agl: 'T',
      nivel_agl: 4,
      vl_cta: 5620000.0,
      ind_dc_cta: 'D',
      ind_grp_dre: 'D',
      cnpj: '33000167000101',
      hash_arquivo_id: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
      reg_0000_id: 1,
      reg_j005_id: 1,
    },
  ],
  reg_i155: [
    {
      cod_cta: '1.1.1.01.001',
      vl_sld_ini: 15400000.0,
      ind_dc_ini: 'D',
      vl_deb: 42100000.0,
      vl_cred: 38200000.0,
      vl_sld_fim: 19300000.0,
      ind_dc_fim: 'D',
      reg_0000_id: 1,
      reg_i150_id: 1,
    },
    {
      cod_cta: '3.1.1.01.001',
      vl_sld_ini: 0.0,
      ind_dc_ini: 'C',
      vl_deb: 0.0,
      vl_cred: 35120000.0,
      vl_sld_fim: 35120000.0,
      ind_dc_fim: 'C',
      reg_0000_id: 1,
      reg_i150_id: 1,
    },
  ],
};

export default function App() {
  const router = useRouter();
  const currentPath = useRouterState({
    select: (s) => s.location.pathname,
  });

  const getActiveTabFromPath = (path: string): NavItem => {
    if (path.startsWith('/fiscal')) return 'fiscal';
    if (path.startsWith('/grc')) return 'grc';
    if (path.startsWith('/indicadores')) return 'indicadores';
    return 'ingestao';
  };

  const activeTab = getActiveTabFromPath(currentPath || '/');

  const handleTabChange = (tab: NavItem) => {
    const targetPath = tab === 'ingestao' ? '/ingestao' : `/${tab}`;
    router.navigate({ to: targetPath });
  };

  const [isCopilotOpen, setIsCopilotOpen] = useState(false);

  // Ingestão
  const [jobs, setJobs] = useState<ImportacaoEcd[]>(INITIAL_MOCK_JOBS);
  const [isLoadingJobs, setIsLoadingJobs] = useState(false);
  const [activeLogJobId, setActiveLogJobId] = useState<string | null>(null);
  const [activeLogFilename, setActiveLogFilename] = useState<string | undefined>(undefined);
  const [isLogDrawerOpen, setIsLogDrawerOpen] = useState(false);

  // Fiscal
  const [cnpjBuscado, setCnpjBuscado] = useState('33000167000101');
  const [fiscalData, setFiscalData] = useState<ConsultaCnpjResponse | null>(INITIAL_MOCK_FISCAL);
  const [isLoadingFiscal, setIsLoadingFiscal] = useState(false);

  // GRC
  const [selectedGraphNode, setSelectedGraphNode] = useState<GrafoNode | null>(null);
  const [isNodeDrawerOpen, setIsNodeDrawerOpen] = useState(false);
  const [selectedPathNode, setSelectedPathNode] = useState<string | null>(null);

  // Carregar jobs da API
  const carregarJobs = async () => {
    try {
      setIsLoadingJobs(true);
      const data = await api.importacoes.listar();
      if (data && data.length > 0) {
        setJobs(data);
      }
    } catch {
      // Mantém mock inicial
    } finally {
      setIsLoadingJobs(false);
    }
  };

  useEffect(() => {
    carregarJobs();
  }, []);

  const handleUploadSuccess = (jobId: string, filename: string) => {
    const newJob: ImportacaoEcd = {
      id: jobId,
      hash_arquivo: `sim-${Date.now()}`,
      cnpj: '33000167000101',
      nome_arquivo_original: filename,
      caminho_armazenamento: `media/ecd_stage/aguardando/${filename}`,
      tamanho_bytes: 145000000,
      status: 'Aguardando',
      linhas_processadas: 0,
      tempo_processamento_ms: 0,
      criado_em: new Date().toISOString(),
      atualizado_em: new Date().toISOString(),
    };
    setJobs((prev) => [newJob, ...prev]);
    setActiveLogJobId(jobId);
    setActiveLogFilename(filename);
    setIsLogDrawerOpen(true);
  };

  const handleOpenLiveLogs = (jobId: string, filename: string) => {
    setActiveLogJobId(jobId);
    setActiveLogFilename(filename);
    setIsLogDrawerOpen(true);
  };

  const handleBuscarCnpj = async (cnpj: string) => {
    setCnpjBuscado(cnpj);
    try {
      setIsLoadingFiscal(true);
      const res = await api.fiscal.consultarCnpj(cnpj);
      setFiscalData(res);
    } catch {
      // Ajusta mock com CNPJ buscado para demonstração
      const fallbackResultado: ResultadoExercicio = {
        ...INITIAL_MOCK_FISCAL.resultado_exercicio!,
        cnpj,
      };
      setFiscalData({
        ...INITIAL_MOCK_FISCAL,
        cnpj,
        resultado_exercicio: fallbackResultado,
      });
    } finally {
      setIsLoadingFiscal(false);
    }
  };

  const handleSelectGraphNode = (node: GrafoNode) => {
    setSelectedGraphNode(node);
    setIsNodeDrawerOpen(true);
  };

  const getTabTitleAndSubtitle = () => {
    switch (activeTab) {
      case 'ingestao':
        return {
          title: 'Painel de Ingestão SPED & Filas de Processamento',
          subtitle:
            'Upload multipart streaming, worker assíncrono consumidor Redis e logs em tempo real via SSE.',
        };
      case 'fiscal':
        return {
          title: 'Auditoria Fiscal & Apuração Contábil (DRE)',
          subtitle:
            'Demonstrativos oficiais SPED ECD (0000, J100, J150, I155) e classificação de Receita Bruta por fuzzy matching.',
        };
      case 'grc':
        return {
          title: 'Rede Societária & Análise de Vínculos GRC',
          subtitle:
            'Canvas interativo com algoritmo BFS de 1 a 10 camadas, busca de menor caminho e cruzamento de sanções (links.db).',
        };
      case 'indicadores':
        return {
          title: 'Inteligência Setorial & Indicadores Econômicos',
          subtitle:
            'Distribuição de empresas por CNAE, Porte Empresarial, Regimes Tributários e métricas de capital social.',
        };
    }
  };

  const { title, subtitle } = getTabTitleAndSubtitle();

  return (
    <QueryClientProvider client={queryClient}>
      <AppLayout
        activeTab={activeTab}
        onTabChange={handleTabChange}
        title={title}
        subtitle={subtitle}
        isCopilotOpen={isCopilotOpen}
        onToggleCopilot={() => setIsCopilotOpen((prev) => !prev)}
        onRefresh={activeTab === 'ingestao' ? carregarJobs : undefined}
        isRefreshing={isLoadingJobs}
        copilotDrawer={
          <AiChatDrawer
            isOpen={isCopilotOpen}
            onClose={() => setIsCopilotOpen(false)}
            activeCnpj={cnpjBuscado}
          />
        }
      >
        {/* ABA 1: INGESTÃO */}
        {activeTab === 'ingestao' && (
          <div className="space-y-8">
            {/* KPI Cards Ingestão */}
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
              <Card className="border-slate-200">
                <CardContent className="p-5 flex items-center justify-between">
                  <div>
                    <span className="text-xs font-semibold text-slate-500 uppercase">
                      Escriturações Importadas
                    </span>
                    <div className="text-2xl font-extrabold text-slate-900 mt-1">
                      {jobs.length.toLocaleString('pt-BR')}
                    </div>
                    <p className="text-[11px] text-emerald-600 font-medium mt-0.5">
                      Banco PostgreSQL 16
                    </p>
                  </div>
                  <div className="h-10 w-10 rounded-xl bg-blue-50 text-blue-600 flex items-center justify-center">
                    <FileSpreadsheet className="h-5 w-5" />
                  </div>
                </CardContent>
              </Card>

              <Card className="border-slate-200">
                <CardContent className="p-5 flex items-center justify-between">
                  <div>
                    <span className="text-xs font-semibold text-slate-500 uppercase">
                      Tempo Médio de Parsing
                    </span>
                    <div className="text-2xl font-extrabold text-slate-900 mt-1">3.8s</div>
                    <p className="text-[11px] text-blue-600 font-medium mt-0.5">
                      memmap2 zero-copy (500MB)
                    </p>
                  </div>
                  <div className="h-10 w-10 rounded-xl bg-emerald-50 text-emerald-600 flex items-center justify-center">
                    <Clock className="h-5 w-5" />
                  </div>
                </CardContent>
              </Card>

              <Card className="border-slate-200">
                <CardContent className="p-5 flex items-center justify-between">
                  <div>
                    <span className="text-xs font-semibold text-slate-500 uppercase">
                      Worker & Fila Redis
                    </span>
                    <div className="text-2xl font-extrabold text-emerald-600 mt-1">Online</div>
                    <p className="text-[11px] text-slate-500 font-medium mt-0.5">
                      BRPOP ecd:queue:jobs
                    </p>
                  </div>
                  <div className="h-10 w-10 rounded-xl bg-emerald-50 text-emerald-600 flex items-center justify-center">
                    <Activity className="h-5 w-5 animate-pulse" />
                  </div>
                </CardContent>
              </Card>
            </div>

            {/* Dropzone */}
            <div className="space-y-2">
              <h3 className="text-sm font-bold text-slate-900">Nova Ingestão de Escrituração</h3>
              <FileDropzone onUploadSuccess={handleUploadSuccess} />
            </div>

            {/* Jobs Table */}
            <div className="space-y-2">
              <h3 className="text-sm font-bold text-slate-900">Histórico de Arquivos Processados</h3>
              <ImportJobsTable
                jobs={jobs}
                isLoading={isLoadingJobs}
                onOpenLiveLogs={handleOpenLiveLogs}
                onRefresh={carregarJobs}
              />
            </div>

            {/* Live Logs SSE Drawer */}
            <LiveLogDrawer
              jobId={activeLogJobId}
              filename={activeLogFilename}
              isOpen={isLogDrawerOpen}
              onClose={() => setIsLogDrawerOpen(false)}
            />
          </div>
        )}

        {/* ABA 2: AUDITORIA FISCAL */}
        {activeTab === 'fiscal' && (
          <div className="space-y-6">
            <Card className="border-slate-200">
              <CardContent className="p-6">
                <CnpjSearchBar
                  initialValue={cnpjBuscado}
                  onSearch={handleBuscarCnpj}
                  isLoading={isLoadingFiscal}
                />
              </CardContent>
            </Card>

            {fiscalData ? (
              <div className="space-y-6">
                {fiscalData.resultado_exercicio && (
                  <DreViewer
                    resultado={fiscalData.resultado_exercicio}
                    cnpj={fiscalData.cnpj}
                  />
                )}

                <div className="space-y-2">
                  <h3 className="text-sm font-bold text-slate-900 flex items-center gap-2">
                    <ShieldCheck className="h-4 w-4 text-blue-600" />
                    <span>Demonstrativos Contábeis Oficiais da Escrituração (SPED)</span>
                  </h3>
                  <SpedBlocksTabs
                    bloco0000={fiscalData.reg_0000}
                    blocoJ100={fiscalData.reg_j100}
                    blocoJ150={fiscalData.reg_j150}
                    blocoI155={fiscalData.reg_i155}
                  />
                </div>
              </div>
            ) : (
              <div className="flex flex-col items-center justify-center p-12 text-center bg-white rounded-xl border border-slate-200">
                <Search className="h-10 w-10 text-slate-400 mb-3" />
                <h4 className="font-semibold text-slate-800">Nenhum CNPJ consultado</h4>
                <p className="text-xs text-slate-500 max-w-sm mt-1">
                  Digite um CNPJ válido na barra acima para visualizar a DRE apurada e os blocos
                  do SPED Contábil.
                </p>
              </div>
            )}
          </div>
        )}

        {/* ABA 3: REDE SOCIETÁRIA GRC */}
        {activeTab === 'grc' && (
          <div className="space-y-6">
            <NetworkGraphCanvas
              initialRaiz={cnpjBuscado}
              onSelectNode={handleSelectGraphNode}
              externalPathNode={selectedPathNode}
            />

            <NodeDetailDrawer
              node={selectedGraphNode}
              isOpen={isNodeDrawerOpen}
              onClose={() => setIsNodeDrawerOpen(false)}
              onExpandNode={(nodeId) => {
                console.log('Expandir nó:', nodeId);
              }}
              onFindShortestPath={(nodeId) => {
                setSelectedPathNode(nodeId);
                setIsNodeDrawerOpen(false);
              }}
            />
          </div>
        )}

        {/* ABA 4: INDICADORES ECONÔMICOS */}
        {activeTab === 'indicadores' && <EconomicIndicatorsDashboard />}
      </AppLayout>
    </QueryClientProvider>
  );
}
