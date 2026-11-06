import React, { useEffect, useRef, useState, useMemo } from 'react';
import { Network, ZoomIn, ZoomOut, Maximize2, Filter, Users, Building2 } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

interface GraphNode {
  id: string;
  label: string;
  type: 'empresa' | 'socio' | 'sancao';
  cnpj?: string;
  cpf?: string;
  percentual?: number;
  sancoes?: string[];
}

interface GraphEdge {
  from: string;
  to: string;
  label: string;
  type: 'vinculo' | 'participacao';
}

const COLORS = {
  empresa: '#3b82f6',
  socio: '#10b981',
  sancao: '#ef4444',
  edge: '#64748b'
};

const MOCK_NODES: GraphNode[] = [
  { id: '1', label: 'EMPRESA MATRIZ LTDA', type: 'empresa', cnpj: '00.000.000/0001-91' },
  { id: '2', label: 'EMPRESA FILIAL 1 S.A.', type: 'empresa', cnpj: '00.000.000/0002-72' },
  { id: '3', label: 'EMPRESA FILIAL 2 LTDA', type: 'empresa', cnpj: '00.000.000/0003-53' },
  { id: '4', label: 'JOÃO SILVA', type: 'socio', cpf: '000.000.000-00', percentual: 45 },
  { id: '5', label: 'MARIA SANTOS', type: 'socio', cpf: '111.111.111-11', percentual: 30 },
  { id: '6', label: 'PEDRO OLIVEIRA', type: 'socio', cpf: '222.222.222-22', percentual: 25 },
  { id: '7', label: 'EMPRESA VINCULADA S.A.', type: 'empresa', cnpj: '11.111.111/0001-11' },
  { id: '8', label: 'CARLOS PEP', type: 'socio', cpf: '333.333.333-33', sancoes: ['PEP'] },
  { id: '9', label: 'EMPRESA SANÇÃO', type: 'sancao', cnpj: '22.222.222/0001-22', sancoes: ['CEIS', 'PGFN'] }
];

const MOCK_EDGES: GraphEdge[] = [
  { from: '1', to: '2', label: 'Filial', type: 'vinculo' },
  { from: '1', to: '3', label: 'Filial', type: 'vinculo' },
  { from: '4', to: '1', label: '45%', type: 'participacao' },
  { from: '5', to: '1', label: '30%', type: 'participacao' },
  { from: '6', to: '1', label: '25%', type: 'participacao' },
  { from: '4', to: '7', label: '60%', type: 'participacao' },
  { from: '8', to: '7', label: '40%', type: 'participacao' },
  { from: '9', to: '1', label: 'Vínculo', type: 'vinculo' }
];

export const NetworkGraphViewer: React.FC = () => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [zoom, setZoom] = useState(1);
  const [selectedNode, setSelectedNode] = useState<GraphNode | null>(null);
  const [filter, setFilter] = useState<'all' | 'empresa' | 'socio' | 'sancao'>('all');

  const filteredNodes = useMemo(() => {
    if (filter === 'all') return MOCK_NODES;
    return MOCK_NODES.filter(n => n.type === filter);
  }, [filter]);

  const filteredEdges = useMemo(() => {
    const nodeIds = new Set(filteredNodes.map(n => n.id));
    return MOCK_EDGES.filter(e => nodeIds.has(e.from) && nodeIds.has(e.to));
  }, [filteredNodes]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * window.devicePixelRatio;
    canvas.height = rect.height * window.devicePixelRatio;
    ctx.scale(window.devicePixelRatio, window.devicePixelRatio);

    const width = rect.width;
    const height = rect.height;

    // Clear canvas
    ctx.fillStyle = '#f8fafc';
    ctx.fillRect(0, 0, width, height);

    // Draw grid
    ctx.strokeStyle = '#e2e8f0';
    ctx.lineWidth = 0.5;
    for (let x = 0; x < width; x += 40) {
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();
    }
    for (let y = 0; y < height; y += 40) {
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(width, y);
      ctx.stroke();
    }

    // Calculate node positions (circular layout)
    const centerX = width / 2;
    const centerY = height / 2;
    const radius = Math.min(width, height) * 0.35;

    const nodePositions = new Map<string, { x: number; y: number }>();
    filteredNodes.forEach((node, index) => {
      const angle = (index / filteredNodes.length) * 2 * Math.PI - Math.PI / 2;
      nodePositions.set(node.id, {
        x: centerX + Math.cos(angle) * radius * zoom,
        y: centerY + Math.sin(angle) * radius * zoom
      });
    });

    // Draw edges
    ctx.strokeStyle = COLORS.edge;
    ctx.lineWidth = 1.5;
    filteredEdges.forEach(edge => {
      const from = nodePositions.get(edge.from);
      const to = nodePositions.get(edge.to);
      if (!from || !to) return;

      ctx.beginPath();
      ctx.moveTo(from.x, from.y);
      ctx.lineTo(to.x, to.y);
      ctx.stroke();

      // Draw edge label
      const midX = (from.x + to.x) / 2;
      const midY = (from.y + to.y) / 2;
      ctx.fillStyle = '#475569';
      ctx.font = '10px Inter, sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(edge.label, midX, midY - 5);
    });

    // Draw nodes
    filteredNodes.forEach(node => {
      const pos = nodePositions.get(node.id);
      if (!pos) return;

      const nodeRadius = 28 * zoom;
      const color = COLORS[node.type];

      // Node circle
      ctx.beginPath();
      ctx.arc(pos.x, pos.y, nodeRadius, 0, 2 * Math.PI);
      ctx.fillStyle = selectedNode?.id === node.id ? '#dbeafe' : 'white';
      ctx.fill();
      ctx.strokeStyle = color;
      ctx.lineWidth = 3;
      ctx.stroke();

      // Node icon (simplified)
      ctx.fillStyle = color;
      ctx.font = 'bold 14px Inter, sans-serif';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      const icon = node.type === 'empresa' ? '🏢' : node.type === 'socio' ? '👤' : '⚠️';
      ctx.fillText(icon, pos.x, pos.y - 6);

      // Node label
      ctx.fillStyle = '#1e293b';
      ctx.font = 'bold 10px Inter, sans-serif';
      const label = node.label.length > 15 ? node.label.substring(0, 12) + '...' : node.label;
      ctx.fillText(label, pos.x, pos.y + nodeRadius + 12);

      // Sanction badge
      if (node.sancoes && node.sancoes.length > 0) {
        ctx.beginPath();
        ctx.arc(pos.x + nodeRadius - 5, pos.y - nodeRadius + 5, 8, 0, 2 * Math.PI);
        ctx.fillStyle = '#ef4444';
        ctx.fill();
        ctx.fillStyle = 'white';
        ctx.font = 'bold 8px Inter, sans-serif';
        ctx.fillText(node.sancoes.length.toString(), pos.x + nodeRadius - 5, pos.y - nodeRadius + 5);
      }
    });

    // Draw legend
    const legendY = 20;
    let legendX = 20;

    const drawLegendItem = (label: string, color: string) => {
      ctx.beginPath();
      ctx.arc(legendX, legendY, 6, 0, 2 * Math.PI);
      ctx.fillStyle = color;
      ctx.fill();
      ctx.fillStyle = '#475569';
      ctx.font = '11px Inter, sans-serif';
      ctx.textAlign = 'left';
      ctx.fillText(label, legendX + 12, legendY + 4);
      legendX += ctx.measureText(label).width + 30;
    };

    drawLegendItem('Empresa', COLORS.empresa);
    drawLegendItem('Sócio', COLORS.socio);
    drawLegendItem('Sanção', COLORS.sancao);

  }, [filteredNodes, filteredEdges, zoom, selectedNode]);

  const handleCanvasClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    const centerX = rect.width / 2;
    const centerY = rect.height / 2;
    const radius = Math.min(rect.width, rect.height) * 0.35;

    filteredNodes.forEach((node, index) => {
      const angle = (index / filteredNodes.length) * 2 * Math.PI - Math.PI / 2;
      const nodeX = centerX + Math.cos(angle) * radius * zoom;
      const nodeY = centerY + Math.sin(angle) * radius * zoom;
      const nodeRadius = 28 * zoom;

      const distance = Math.sqrt(Math.pow(x - nodeX, 2) + Math.pow(y - nodeY, 2));
      if (distance <= nodeRadius) {
        setSelectedNode(node);
      }
    });
  };

  return (
    <Card className="border-slate-200">
      <CardHeader className="border-b border-slate-100">
        <div className="flex items-center justify-between">
          <CardTitle className="text-lg font-bold flex items-center gap-2">
            <Network className="h-5 w-5 text-blue-600" />
            Societary Network Graph
          </CardTitle>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setFilter(filter === 'all' ? 'empresa' : filter === 'empresa' ? 'socio' : filter === 'socio' ? 'sancao' : 'all')}
              className="text-xs"
            >
              <Filter className="h-3 w-3 mr-1" />
              {filter === 'all' ? 'Todos' : filter === 'empresa' ? 'Empresas' : filter === 'socio' ? 'Sócios' : 'Sanções'}
            </Button>
            <Button
              variant="outline"
              size="sm"
              title="Afastar Zoom"
              onClick={() => setZoom(Math.max(0.5, zoom - 0.1))}
              disabled={zoom <= 0.5}
            >
              <ZoomOut className="h-3 w-3" />
            </Button>
            <Button
              variant="outline"
              size="sm"
              title="Aproximar Zoom"
              onClick={() => setZoom(Math.min(2, zoom + 0.1))}
              disabled={zoom >= 2}
            >
              <ZoomIn className="h-3 w-3" />
            </Button>
            <Button
              variant="outline"
              size="sm"
              title="Enquadrar Visualização"
              onClick={() => setZoom(1)}
            >
              <Maximize2 className="h-3 w-3" />
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="p-0">
        <div ref={containerRef} className="relative">
          <canvas
            ref={canvasRef}
            onClick={handleCanvasClick}
            className="w-full cursor-pointer"
            style={{ height: '400px' }}
          />

          {selectedNode && (
            <div className="absolute bottom-4 left-4 right-4 bg-white rounded-lg border border-slate-200 p-4 shadow-lg">
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-3">
                  <div className={`h-10 w-10 rounded-lg flex items-center justify-center ${
                    selectedNode.type === 'empresa' ? 'bg-blue-100 text-blue-600' :
                    selectedNode.type === 'socio' ? 'bg-emerald-100 text-emerald-600' :
                    'bg-red-100 text-red-600'
                  }`}>
                    {selectedNode.type === 'empresa' ? <Building2 className="h-5 w-5" /> : <Users className="h-5 w-5" />}
                  </div>
                  <div>
                    <h4 className="font-semibold text-slate-900">{selectedNode.label}</h4>
                    <p className="text-xs text-slate-500">
                      {selectedNode.cnpj || selectedNode.cpf}
                    </p>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {selectedNode.percentual && (
                    <Badge variant="outline" className="text-xs">
                      {selectedNode.percentual}% participação
                    </Badge>
                  )}
                  {selectedNode.sancoes && selectedNode.sancoes.length > 0 && (
                    selectedNode.sancoes.map((s) => (
                      <Badge key={s} className="bg-red-100 text-red-700 text-xs">
                        {s}
                      </Badge>
                    ))
                  )}
                </div>
              </div>
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
};
