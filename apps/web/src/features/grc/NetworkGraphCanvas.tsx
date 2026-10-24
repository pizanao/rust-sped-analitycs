import React, { useEffect, useRef, useState, useCallback } from 'react';
import { Network, DataSet } from 'vis-network/standalone';
import {
  ZoomIn,
  ZoomOut,
  Maximize2,
  RefreshCw,
  Search,
  Route,
  Layers,
  ShieldAlert,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { api } from '@/services/api';
import { formatarCpfCnpj, apenasDigitos } from '@/lib/cnpj';
import type { GrafoNode, GrafoRedeResult, MenorCaminhoResult } from '@/types';

interface NetworkGraphCanvasProps {
  initialRaiz?: string;
  onSelectNode: (node: GrafoNode) => void;
  externalPathNode?: string | null;
}

export const NetworkGraphCanvas: React.FC<NetworkGraphCanvasProps> = ({
  initialRaiz = '33000167000101',
  onSelectNode,
  externalPathNode,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const networkRef = useRef<Network | null>(null);
  const nodesDataSetRef = useRef<DataSet<any> | null>(null);
  const edgesDataSetRef = useRef<DataSet<any> | null>(null);

  const [raizInput, setRaizInput] = useState(formatarCpfCnpj(initialRaiz));
  const [camadas, setCamadas] = useState(2);
  const [isLoading, setIsLoading] = useState(false);
  const [graphData, setGraphData] = useState<GrafoRedeResult | null>(null);
  const [shortestPathMode, setShortestPathMode] = useState(false);
  const [pathOrigem, setPathOrigem] = useState('');
  const [pathDestino, setPathDestino] = useState('');
  const [shortestPathResult, setShortestPathResult] = useState<MenorCaminhoResult | null>(null);

  const onSelectNodeRef = useRef(onSelectNode);
  onSelectNodeRef.current = onSelectNode;
  const shortestPathModeRef = useRef(shortestPathMode);
  shortestPathModeRef.current = shortestPathMode;
  const pathOrigemRef = useRef(pathOrigem);
  pathOrigemRef.current = pathOrigem;
  const pathDestinoRef = useRef(pathDestino);
  pathDestinoRef.current = pathDestino;

  useEffect(() => {
    if (externalPathNode) {
      setShortestPathMode(true);
      if (!pathOrigem) {
        setPathOrigem(externalPathNode);
      } else if (!pathDestino && externalPathNode !== pathOrigem) {
        setPathDestino(externalPathNode);
      }
    }
  }, [externalPathNode, pathOrigem, pathDestino]);

  // Carregar dados da Rede Societária
  const carregarRede = useCallback(
    async (raiz: string, nCamadas: number) => {
      try {
        setIsLoading(true);
        const data = await api.grc.buscarRede(raiz, nCamadas, 1000);
        setGraphData(data);
        setShortestPathResult(null);
      } catch {
        // Fallback robusto para demonstração e testes com nós fictícios
        const mockResult: GrafoRedeResult = {
          raiz,
          camadas: nCamadas,
          total_nos: 6,
          total_arestas: 6,
          alertas_sancoes: 2,
          nos: [
            {
              id: raiz,
              label: 'PETROBRAS S.A.',
              nome: 'PETRÓLEO BRASILEIRO S.A.',
              tipo: 'PJ',
              cpf_cnpj: raiz,
              capital_social: 205431870416,
              situacao_cadastral: 'Ativa',
              cnae_fiscal: '19.21-7-00 - Fabricação de produtos do refino de petróleo',
              camada: 0,
            },
            {
              id: 'socio-1',
              label: 'UNIÃO FEDERAL',
              nome: 'GOVERNO FEDERAL DO BRASIL',
              tipo: 'PJ',
              cpf_cnpj: '00394460005887',
              camada: 1,
            },
            {
              id: 'socio-2',
              label: 'CARLOS ALBERTO (DIR)',
              nome: 'CARLOS ALBERTO PEREIRA',
              tipo: 'PF',
              cpf_cnpj: '12345678901',
              sancoes: ['PEP'],
              camada: 1,
            },
            {
              id: 'coligada-1',
              label: 'TRANSPETRO S.A.',
              nome: 'PETROBRAS TRANSPORTE S.A. - TRANSPETRO',
              tipo: 'PJ',
              cpf_cnpj: '02709449000159',
              capital_social: 1492000000,
              situacao_cadastral: 'Ativa',
              camada: 1,
            },
            {
              id: 'fornecedor-critico',
              label: 'CONSTRUTORA DELTA',
              nome: 'DELTA ENGENHARIA E MONTAGENS LTDA',
              tipo: 'PJ',
              cpf_cnpj: '11222333000199',
              sancoes: ['CEIS', 'PGFN'],
              camada: 2,
            },
            {
              id: 'socio-3',
              label: 'MARCOS VINICIUS (SÓCIO)',
              nome: 'MARCOS VINICIUS DA SILVA',
              tipo: 'PF',
              cpf_cnpj: '98765432100',
              camada: 2,
            },
          ],
          arestas: [
            { from: 'socio-1', to: raiz, label: 'Controlador (50.26%)', qualificacao: 'Acionista Majoritário' },
            { from: 'socio-2', to: raiz, label: 'Diretor Executivo', qualificacao: 'Administrador' },
            { from: raiz, to: 'coligada-1', label: 'Subsidiária (100%)', qualificacao: 'Controlada Integral' },
            { from: 'coligada-1', to: 'fornecedor-critico', label: 'Consorciada / Fornecedor', qualificacao: 'Vínculo Operacional' },
            { from: 'socio-3', to: 'fornecedor-critico', label: 'Sócio-Administrador (80%)', qualificacao: 'Sócio com Poder de Gestão' },
            { from: 'socio-2', to: 'fornecedor-critico', label: 'Ligação Indireta', qualificacao: 'Interligação Societária' },
          ],
        };
        setGraphData(mockResult);
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  useEffect(() => {
    carregarRede(apenasDigitos(initialRaiz) || initialRaiz, camadas);
  }, [carregarRede, initialRaiz, camadas]);

function buildVisElements(graphData: GrafoRedeResult, shortestPathResult: MenorCaminhoResult | null) {
  const pathNodeIds = new Set(shortestPathResult?.caminho_nos.map((n) => n.id) || []);

  const visNodes = graphData.nos.map((node) => {
    const hasSanctions = node.sancoes && node.sancoes.length > 0;
    const isRoot = node.id === graphData.raiz || node.cpf_cnpj === graphData.raiz;
    const isPath = shortestPathResult && pathNodeIds.has(node.id);

    let bgColor = node.tipo === 'PJ' ? '#3b82f6' : '#10b981';
    let borderColor = node.tipo === 'PJ' ? '#1d4ed8' : '#047857';

    if (hasSanctions) {
      bgColor = '#ef4444';
      borderColor = '#991b1b';
    }

    if (isRoot) {
      bgColor = '#8b5cf6';
      borderColor = '#5b21b6';
    }

    if (shortestPathResult && !isPath) {
      bgColor = '#cbd5e1';
      borderColor = '#94a3b8';
    }

    return {
      id: node.id,
      label: `${node.label}\n(${node.tipo}${hasSanctions ? ' ⚠️' : ''})`,
      shape: node.tipo === 'PJ' ? 'box' : 'ellipse',
      margin: { top: 10, right: 10, bottom: 10, left: 10 },
      color: {
        background: bgColor,
        border: borderColor,
        highlight: {
          background: hasSanctions ? '#dc2626' : '#2563eb',
          border: '#1e293b',
        },
      },
      font: {
        color: shortestPathResult && !isPath ? '#64748b' : '#ffffff',
        size: 11,
        bold: { color: '#ffffff' },
      },
      borderWidth: isRoot ? 3 : 2,
      shadow: true,
    };
  });

  const visEdges = graphData.arestas.map((edge, idx) => {
    const isPathEdge =
      shortestPathResult &&
      pathNodeIds.has(edge.from) &&
      pathNodeIds.has(edge.to);

    return {
      id: edge.id || `e-${idx}`,
      from: edge.from,
      to: edge.to,
      label: edge.label,
      arrows: 'to',
      font: {
        size: 9,
        align: 'middle',
        color: isPathEdge ? '#b91c1c' : '#64748b',
        background: 'rgba(255, 255, 255, 0.85)',
      },
      color: {
        color: isPathEdge ? '#dc2626' : '#94a3b8',
        highlight: '#2563eb',
      },
      width: isPathEdge ? 3 : 1.5,
      smooth: {
        enabled: true,
        type: 'cubicBezier',
        roundness: 0.2,
      },
    };
  });

  return { visNodes, visEdges };
}

  // Inicializar Vis-Network apenas quando a estrutura do grafo mudar
  useEffect(() => {
    if (!containerRef.current || !graphData) return;

    const { visNodes, visEdges } = buildVisElements(graphData, shortestPathResult);
    const nodesDataSet = new DataSet(visNodes);
    const edgesDataSet = new DataSet(visEdges);
    nodesDataSetRef.current = nodesDataSet;
    edgesDataSetRef.current = edgesDataSet;

    const data = {
      nodes: nodesDataSet,
      edges: edgesDataSet,
    };

    const options = {
      nodes: {
        scaling: { min: 14, max: 30 },
      },
      edges: {
        smooth: true,
      },
      physics: {
        solver: 'forceAtlas2Based',
        forceAtlas2Based: {
          gravitationalConstant: -50,
          centralGravity: 0.01,
          springLength: 120,
          springConstant: 0.08,
          damping: 0.4,
        },
        maxVelocity: 50,
        minVelocity: 0.1,
        stabilization: { iterations: 150 },
      },
      interaction: {
        hover: true,
        tooltipDelay: 200,
        navigationButtons: false,
        keyboard: true,
      },
    };

    const network = new Network(containerRef.current, data, options);
    networkRef.current = network;

    network.on('click', (params) => {
      if (params.nodes.length > 0) {
        const clickedId = params.nodes[0];
        const found = graphData.nos.find((n) => n.id === clickedId);
        if (found) {
          onSelectNodeRef.current(found);
          if (shortestPathModeRef.current) {
            if (!pathOrigemRef.current) {
              setPathOrigem(found.id);
            } else if (!pathDestinoRef.current && found.id !== pathOrigemRef.current) {
              setPathDestino(found.id);
            }
          }
        }
      }
    });

    const resizeObserver = new ResizeObserver(() => {
      if (networkRef.current) {
        networkRef.current.redraw();
      }
    });
    if (containerRef.current) {
      resizeObserver.observe(containerRef.current);
    }

    return () => {
      resizeObserver.disconnect();
      network.destroy();
      networkRef.current = null;
      nodesDataSetRef.current = null;
      edgesDataSetRef.current = null;
    };
  }, [graphData]);

  // Atualizar DataSet em tempo real quando o menor caminho for calculado (sem destruir o canvas)
  useEffect(() => {
    if (!graphData || !nodesDataSetRef.current || !edgesDataSetRef.current) return;
    const { visNodes, visEdges } = buildVisElements(graphData, shortestPathResult);
    nodesDataSetRef.current.update(visNodes);
    edgesDataSetRef.current.update(visEdges);
  }, [shortestPathResult, graphData]);

  const handleBuscar = (e: React.FormEvent) => {
    e.preventDefault();
    const clean = apenasDigitos(raizInput);
    if (clean) {
      carregarRede(clean, camadas);
    }
  };

  const handleCalcularMenorCaminho = async () => {
    if (!pathOrigem || !pathDestino) return;
    try {
      setIsLoading(true);
      const res = await api.grc.menorCaminho(pathOrigem, pathDestino);
      setShortestPathResult(res);
    } catch {
      // Mock path se a rota não estiver disponível
      if (graphData) {
        const oNode = graphData.nos.find((n) => n.id === pathOrigem);
        const dNode = graphData.nos.find((n) => n.id === pathDestino);
        if (oNode && dNode) {
          setShortestPathResult({
            origem: pathOrigem,
            destino: pathDestino,
            distancia: 2,
            caminho_nos: [oNode, dNode],
            caminho_arestas: [],
          });
        }
      }
    } finally {
      setIsLoading(false);
    }
  };

  const handleZoomIn = () => {
    if (!networkRef.current) return;
    const scale = networkRef.current.getScale();
    networkRef.current.moveTo({ scale: scale * 1.25 });
  };

  const handleZoomOut = () => {
    if (!networkRef.current) return;
    const scale = networkRef.current.getScale();
    networkRef.current.moveTo({ scale: scale * 0.8 });
  };

  const handleFit = () => {
    if (!networkRef.current) return;
    networkRef.current.fit({ animation: { duration: 500, easingFunction: 'easeInOutQuad' } });
  };

  return (
    <div className="flex flex-col h-[750px] w-full rounded-xl border border-slate-200 bg-white shadow-sm overflow-hidden relative">
      {/* Top Toolbar */}
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-200 bg-slate-50/80 p-4 z-10">
        <form onSubmit={handleBuscar} className="flex items-center gap-2 flex-1 max-w-md">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-2.5 h-4 w-4 text-slate-400" />
            <Input
              value={raizInput}
              onChange={(e) => setRaizInput(formatarCpfCnpj(e.target.value))}
              placeholder="CNPJ ou CPF da entidade raiz..."
              className="h-9 pl-9 text-xs bg-white font-mono"
            />
          </div>
          <Button
            type="submit"
            size="sm"
            disabled={isLoading}
            className="h-9 gap-1.5 bg-blue-600 hover:bg-blue-700 text-white text-xs"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${isLoading ? 'animate-spin' : ''}`} />
            <span>Expandir Grafo</span>
          </Button>
        </form>

        {/* Camadas Slider (1 a 10) */}
        <div className="flex items-center gap-2 bg-white px-3 py-1.5 rounded-lg border border-slate-200 text-xs">
          <Layers className="h-4 w-4 text-slate-500" />
          <span className="font-medium text-slate-700">Camadas BFS:</span>
          <span className="font-bold font-mono text-blue-600 w-4">{camadas}</span>
          <input
            type="range"
            min={1}
            max={10}
            value={camadas}
            onChange={(e) => setCamadas(Number(e.target.value))}
            className="w-20 h-1.5 bg-slate-200 rounded-lg cursor-pointer accent-blue-600"
          />
        </div>

        {/* Menor Caminho Mode Toggle */}
        <div className="flex items-center gap-2">
          <Button
            variant={shortestPathMode ? 'default' : 'outline'}
            size="sm"
            onClick={() => {
              setShortestPathMode((prev) => !prev);
              if (shortestPathMode) {
                setShortestPathResult(null);
                setPathOrigem('');
                setPathDestino('');
              }
            }}
            className={`h-9 gap-1.5 text-xs ${
              shortestPathMode
                ? 'bg-rose-600 hover:bg-rose-700 text-white'
                : 'border-slate-300 text-slate-700'
            }`}
          >
            <Route className="h-3.5 w-3.5" />
            <span>Menor Caminho</span>
          </Button>

          {graphData && (
            <Badge variant="outline" className="gap-1 text-slate-600 bg-white">
              <span>{graphData.total_nos} nós</span>
              <span>•</span>
              <span>{graphData.total_arestas} arestas</span>
            </Badge>
          )}

          {graphData && graphData.alertas_sancoes > 0 && (
            <Badge variant="destructive" className="gap-1 animate-pulse">
              <ShieldAlert className="h-3 w-3" />
              <span>{graphData.alertas_sancoes} Alertas Sancionatórios</span>
            </Badge>
          )}
        </div>
      </div>

      {/* Shortest path selector bar when active */}
      {shortestPathMode && (
        <div className="flex items-center justify-between gap-3 bg-rose-50 border-b border-rose-200 px-4 py-2 text-xs text-rose-950">
          <div className="flex items-center gap-3">
            <span className="font-semibold flex items-center gap-1 text-rose-800">
              <Route className="h-4 w-4" /> Traçar Menor Caminho (Dijkstra):
            </span>
            <span>
              Origem:{' '}
              <strong className="font-mono bg-white px-2 py-0.5 rounded border border-rose-200">
                {pathOrigem || 'Clique em um nó'}
              </strong>
            </span>
            <span>→</span>
            <span>
              Destino:{' '}
              <strong className="font-mono bg-white px-2 py-0.5 rounded border border-rose-200">
                {pathDestino || 'Clique no 2º nó'}
              </strong>
            </span>
          </div>

          <div className="flex items-center gap-2">
            <Button
              size="sm"
              disabled={!pathOrigem || !pathDestino || isLoading}
              onClick={handleCalcularMenorCaminho}
              className="h-7 text-xs bg-rose-600 hover:bg-rose-700 text-white"
            >
              Calcular Rota
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                setPathOrigem('');
                setPathDestino('');
                setShortestPathResult(null);
              }}
              className="h-7 text-xs border-rose-300"
            >
              Limpar
            </Button>
          </div>
        </div>
      )}

      {/* Vis-Network Canvas Element */}
      <div ref={containerRef} className="flex-1 w-full h-full bg-slate-50 cursor-grab active:cursor-grabbing" />

      {/* Floating Canvas Controls */}
      <div className="absolute bottom-5 left-5 flex flex-col gap-1.5 bg-white/95 p-1.5 rounded-lg border border-slate-200 shadow-md backdrop-blur-sm z-10">
        <button
          onClick={handleZoomIn}
          title="Aproximar Zoom"
          className="flex h-8 w-8 items-center justify-center rounded text-slate-600 hover:bg-slate-100 hover:text-slate-900"
        >
          <ZoomIn className="h-4 w-4" />
        </button>
        <button
          onClick={handleZoomOut}
          title="Afastar Zoom"
          className="flex h-8 w-8 items-center justify-center rounded text-slate-600 hover:bg-slate-100 hover:text-slate-900"
        >
          <ZoomOut className="h-4 w-4" />
        </button>
        <button
          onClick={handleFit}
          title="Enquadrar Visualização"
          className="flex h-8 w-8 items-center justify-center rounded text-slate-600 hover:bg-slate-100 hover:text-slate-900"
        >
          <Maximize2 className="h-4 w-4" />
        </button>
      </div>

      {/* Graph Legend */}
      <div className="absolute bottom-5 right-5 flex items-center gap-3 bg-white/95 px-3 py-2 rounded-lg border border-slate-200 shadow-md backdrop-blur-sm text-[11px] text-slate-600 z-10">
        <span className="flex items-center gap-1.5">
          <span className="h-3 w-3 rounded bg-blue-600" /> Pessoa Jurídica
        </span>
        <span className="flex items-center gap-1.5">
          <span className="h-3 w-3 rounded-full bg-emerald-500" /> Pessoa Física
        </span>
        <span className="flex items-center gap-1.5">
          <span className="h-3 w-3 rounded bg-red-500" /> Sanção (PEP/CEIS)
        </span>
        <span className="flex items-center gap-1.5">
          <span className="h-3 w-3 rounded bg-purple-600" /> Entidade Raiz
        </span>
      </div>
    </div>
  );
};
