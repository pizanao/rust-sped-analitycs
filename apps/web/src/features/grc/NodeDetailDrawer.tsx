import React from 'react';
import {
  X,
  Building,
  User,
  AlertTriangle,
  ShieldAlert,
  Coins,
  MapPin,
  Briefcase,
  Layers,
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { formatarCnpj } from '@/lib/cnpj';
import type { GrafoNode, SancaoTipo } from '@/types';

interface NodeDetailDrawerProps {
  node: GrafoNode | null;
  isOpen: boolean;
  onClose: () => void;
  onExpandNode?: (nodeId: string) => void;
  onFindShortestPath?: (nodeId: string) => void;
}

export const NodeDetailDrawer: React.FC<NodeDetailDrawerProps> = ({
  node,
  isOpen,
  onClose,
  onExpandNode,
  onFindShortestPath,
}) => {
  if (!isOpen || !node) return null;

  const isPJ = node.tipo === 'PJ';

  const formatCurrency = (val?: number): string => {
    if (!val) return 'Não informado';
    return new Intl.NumberFormat('pt-BR', {
      style: 'currency',
      currency: 'BRL',
    }).format(val);
  };

  const getSancaoBadge = (sancao: SancaoTipo) => {
    switch (sancao) {
      case 'PEP':
        return (
          <Badge key={sancao} variant="destructive" className="gap-1 font-bold">
            <AlertTriangle className="h-3 w-3" />
            <span>PEP (Pessoa Politicamente Exposta)</span>
          </Badge>
        );
      case 'CEIS':
        return (
          <Badge key={sancao} variant="destructive" className="gap-1 font-bold bg-rose-700">
            <ShieldAlert className="h-3 w-3" />
            <span>CEIS (Inidônea / Suspensa)</span>
          </Badge>
        );
      case 'CNEP':
        return (
          <Badge key={sancao} variant="destructive" className="gap-1 font-bold bg-purple-700">
            <ShieldAlert className="h-3 w-3" />
            <span>CNEP (Empresa Punida)</span>
          </Badge>
        );
      case 'PGFN':
        return (
          <Badge key={sancao} variant="warning" className="gap-1 font-bold">
            <Coins className="h-3 w-3" />
            <span>Dívida Ativa PGFN</span>
          </Badge>
        );
      case 'CEPIM':
        return (
          <Badge key={sancao} variant="warning" className="gap-1 font-bold">
            <AlertTriangle className="h-3 w-3" />
            <span>CEPIM (Sem Fins Lucrativos Impedida)</span>
          </Badge>
        );
      default:
        return <Badge key={sancao}>{sancao}</Badge>;
    }
  };

  return (
    <div className="fixed inset-y-0 right-0 z-40 flex w-full max-w-md flex-col bg-white text-slate-900 shadow-2xl transition-all duration-300 border-l border-slate-200">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-slate-200 p-5 bg-slate-50/70">
        <div className="flex items-center gap-3">
          <div
            className={`flex h-10 w-10 items-center justify-center rounded-xl text-white shadow-sm ${
              isPJ ? 'bg-blue-600' : 'bg-emerald-600'
            }`}
          >
            {isPJ ? <Building className="h-5 w-5" /> : <User className="h-5 w-5" />}
          </div>
          <div>
            <h2 className="text-base font-bold text-slate-900 leading-tight">
              {node.nome}
            </h2>
            <p className="text-xs text-slate-500 font-mono">
              {isPJ ? `CNPJ: ${formatarCnpj(node.cpf_cnpj)}` : `CPF: ***.${node.cpf_cnpj.slice(3, 9)}-**`}
            </p>
          </div>
        </div>

        <Button
          variant="ghost"
          size="icon"
          onClick={onClose}
          className="h-8 w-8 text-slate-400 hover:text-slate-700 hover:bg-slate-200/60"
        >
          <X className="h-4 w-4" />
        </Button>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        {/* Alertas Sancionatórios */}
        {node.sancoes && node.sancoes.length > 0 ? (
          <div className="rounded-xl border border-red-200 bg-red-50/60 p-4 space-y-2">
            <div className="flex items-center gap-2 text-red-900 font-bold text-xs uppercase tracking-wide">
              <ShieldAlert className="h-4 w-4 text-red-600" />
              <span>Apontamentos Sancionatórios ({node.sancoes.length})</span>
            </div>
            <p className="text-xs text-red-700">
              Cruzamento automático com a base unificada de sanções da CGU e PGFN (links.db):
            </p>
            <div className="flex flex-wrap gap-1.5 pt-1">
              {node.sancoes.map(getSancaoBadge)}
            </div>
          </div>
        ) : (
          <div className="rounded-xl border border-emerald-200 bg-emerald-50/50 p-3.5 flex items-center gap-2.5 text-xs text-emerald-800">
            <span className="flex h-2 w-2 rounded-full bg-emerald-500" />
            <span>Nenhum apontamento restritivo ou sancionatório identificado.</span>
          </div>
        )}

        {/* Informações Cadastrais */}
        <div className="space-y-3">
          <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
            Dados Cadastrais da Entidade
          </h4>

          <div className="grid grid-cols-2 gap-3 text-xs">
            <div className="rounded-lg bg-slate-50 p-3 border border-slate-200/60">
              <span className="text-slate-500 block mb-1">Tipo de Registro</span>
              <span className="font-semibold text-slate-900">
                {isPJ ? 'Pessoa Jurídica (PJ)' : 'Pessoa Física (Sócio/PF)'}
              </span>
            </div>

            <div className="rounded-lg bg-slate-50 p-3 border border-slate-200/60">
              <span className="text-slate-500 block mb-1">Situação Cadastral</span>
              <Badge variant="outline" className="font-semibold">
                {node.situacao_cadastral || 'Ativa'}
              </Badge>
            </div>

            {node.capital_social !== undefined && (
              <div className="rounded-lg bg-slate-50 p-3 border border-slate-200/60 col-span-2">
                <span className="text-slate-500 block mb-1 flex items-center gap-1">
                  <Coins className="h-3 w-3 text-slate-400" /> Capital Social Declarado
                </span>
                <span className="font-mono font-bold text-slate-900 text-sm">
                  {formatCurrency(node.capital_social)}
                </span>
              </div>
            )}

            {node.cnae_fiscal && (
              <div className="rounded-lg bg-slate-50 p-3 border border-slate-200/60 col-span-2">
                <span className="text-slate-500 block mb-1 flex items-center gap-1">
                  <Briefcase className="h-3 w-3 text-slate-400" /> Atividade Principal (CNAE)
                </span>
                <span className="font-medium text-slate-800 text-xs">{node.cnae_fiscal}</span>
              </div>
            )}

            {(node.municipio || node.uf) && (
              <div className="rounded-lg bg-slate-50 p-3 border border-slate-200/60 col-span-2">
                <span className="text-slate-500 block mb-1 flex items-center gap-1">
                  <MapPin className="h-3 w-3 text-slate-400" /> Localização Geográfica
                </span>
                <span className="font-medium text-slate-800 text-xs">
                  {node.municipio || 'Capital'} - {node.uf || 'BR'}
                </span>
              </div>
            )}

            {node.camada !== undefined && (
              <div className="rounded-lg bg-slate-50 p-3 border border-slate-200/60 col-span-2">
                <span className="text-slate-500 block mb-1 flex items-center gap-1">
                  <Layers className="h-3 w-3 text-slate-400" /> Nível de Profundidade no Grafo
                </span>
                <span className="font-semibold text-slate-800 text-xs">
                  Camada {node.camada} a partir da raiz pesquisada
                </span>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Footer Actions */}
      <div className="border-t border-slate-200 p-4 bg-slate-50 flex gap-2">
        {onExpandNode && (
          <Button
            variant="outline"
            size="sm"
            onClick={() => onExpandNode(node.id)}
            className="flex-1 text-xs border-slate-300"
          >
            Expansão BFS (+1 Camada)
          </Button>
        )}
        {onFindShortestPath && (
          <Button
            variant="default"
            size="sm"
            onClick={() => onFindShortestPath(node.id)}
            className="flex-1 text-xs bg-blue-600 hover:bg-blue-700 text-white"
          >
            Menor Caminho
          </Button>
        )}
      </div>
    </div>
  );
};
