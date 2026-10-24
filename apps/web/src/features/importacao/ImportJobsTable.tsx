import React, { useState } from 'react';
import {
  FileText,
  Clock,
  Terminal,
  Search,
  CheckCircle2,
  AlertTriangle,
  RefreshCw,
  Layers,
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@/components/ui/table';
import { formatarCnpj } from '@/lib/cnpj';
import type { ImportacaoEcd, StatusImportacao } from '@/types';

interface ImportJobsTableProps {
  jobs: ImportacaoEcd[];
  isLoading: boolean;
  onOpenLiveLogs: (jobId: string, filename: string) => void;
  onRefresh: () => void;
}

export const ImportJobsTable: React.FC<ImportJobsTableProps> = ({
  jobs,
  isLoading,
  onOpenLiveLogs,
  onRefresh,
}) => {
  const [searchTerm, setSearchTerm] = useState('');
  const [statusFilter, setStatusFilter] = useState<string>('todos');

  const filteredJobs = jobs.filter((job) => {
    const matchesSearch =
      job.nome_arquivo_original.toLowerCase().includes(searchTerm.toLowerCase()) ||
      job.cnpj.includes(searchTerm.replace(/\D/g, '')) ||
      job.hash_arquivo.toLowerCase().includes(searchTerm.toLowerCase());

    const matchesStatus =
      statusFilter === 'todos' || job.status.toLowerCase() === statusFilter.toLowerCase();

    return matchesSearch && matchesStatus;
  });

  const getStatusBadge = (status: StatusImportacao) => {
    switch (status) {
      case 'Sucesso':
        return (
          <Badge variant="success" className="gap-1">
            <CheckCircle2 className="h-3 w-3" />
            <span>Sucesso</span>
          </Badge>
        );
      case 'EmProcessamento':
        return (
          <Badge variant="default" className="gap-1 bg-blue-600 animate-pulse">
            <RefreshCw className="h-3 w-3 animate-spin" />
            <span>Processando</span>
          </Badge>
        );
      case 'Erro':
        return (
          <Badge variant="destructive" className="gap-1">
            <AlertTriangle className="h-3 w-3" />
            <span>Erro</span>
          </Badge>
        );
      case 'Substituida':
        return (
          <Badge variant="secondary" className="gap-1 text-slate-600">
            <Layers className="h-3 w-3" />
            <span>Substituída</span>
          </Badge>
        );
      default:
        return (
          <Badge variant="warning" className="gap-1">
            <Clock className="h-3 w-3" />
            <span>Aguardando Fila</span>
          </Badge>
        );
    }
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  return (
    <div className="space-y-4">
      {/* Filters and search */}
      <div className="flex flex-col sm:flex-row justify-between gap-3">
        <div className="relative flex-1 max-w-sm">
          <Search className="absolute left-3 top-2.5 h-4 w-4 text-slate-400" />
          <Input
            placeholder="Buscar por arquivo, CNPJ ou hash..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="pl-9 bg-white"
          />
        </div>

        <div className="flex items-center gap-2">
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            className="h-9 rounded-md border border-slate-300 bg-white px-3 text-xs font-medium text-slate-700 shadow-sm focus:outline-none focus:ring-1 focus:ring-blue-600"
          >
            <option value="todos">Todos os Status</option>
            <option value="Sucesso">Sucesso</option>
            <option value="EmProcessamento">Em Processamento</option>
            <option value="Aguardando">Aguardando Fila</option>
            <option value="Erro">Erro</option>
            <option value="Substituida">Substituída</option>
          </select>

          <Button
            variant="outline"
            size="sm"
            onClick={onRefresh}
            disabled={isLoading}
            className="h-9 gap-1.5 border-slate-300"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${isLoading ? 'animate-spin' : ''}`} />
            <span>Atualizar</span>
          </Button>
        </div>
      </div>

      {/* Table */}
      <div className="rounded-xl border border-slate-200 bg-white shadow-sm overflow-hidden">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-[30%]">Arquivo Original</TableHead>
              <TableHead>CNPJ</TableHead>
              <TableHead>Tamanho</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="text-right">Linhas SPED</TableHead>
              <TableHead className="text-right">Tempo</TableHead>
              <TableHead className="text-right">Data/Hora</TableHead>
              <TableHead className="text-center">Ações</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {filteredJobs.length === 0 ? (
              <TableRow>
                <TableCell colSpan={8} className="h-32 text-center text-slate-500">
                  {isLoading ? 'Carregando arquivos processados...' : 'Nenhum arquivo encontrado.'}
                </TableCell>
              </TableRow>
            ) : (
              filteredJobs.map((job) => (
                <TableRow key={job.id} className="hover:bg-slate-50/70">
                  <TableCell className="font-medium">
                    <div className="flex items-center gap-2">
                      <FileText className="h-4 w-4 text-blue-600 shrink-0" />
                      <div className="truncate max-w-xs">
                        <span className="block truncate text-slate-900 font-semibold" title={job.nome_arquivo_original}>
                          {job.nome_arquivo_original}
                        </span>
                        <span className="block text-[11px] font-mono text-slate-400 truncate" title={job.hash_arquivo}>
                          Hash: {job.hash_arquivo.slice(0, 16)}...
                        </span>
                      </div>
                    </div>
                  </TableCell>
                  <TableCell className="font-mono text-xs text-slate-700">
                    {formatarCnpj(job.cnpj)}
                  </TableCell>
                  <TableCell className="text-xs text-slate-600">
                    {formatFileSize(job.tamanho_bytes)}
                  </TableCell>
                  <TableCell>{getStatusBadge(job.status)}</TableCell>
                  <TableCell className="text-right font-mono text-xs font-semibold text-slate-800">
                    {job.linhas_processadas > 0
                      ? job.linhas_processadas.toLocaleString('pt-BR')
                      : '—'}
                  </TableCell>
                  <TableCell className="text-right font-mono text-xs text-slate-600">
                    {job.tempo_processamento_ms > 0
                      ? `${(job.tempo_processamento_ms / 1000).toFixed(2)}s`
                      : '—'}
                  </TableCell>
                  <TableCell className="text-right text-xs text-slate-500">
                    {new Date(job.criado_em).toLocaleString('pt-BR', {
                      day: '2-digit',
                      month: '2-digit',
                      year: 'numeric',
                      hour: '2-digit',
                      minute: '2-digit',
                    })}
                  </TableCell>
                  <TableCell className="text-center">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => onOpenLiveLogs(job.id, job.nome_arquivo_original)}
                      className="h-7 px-2 text-xs gap-1 border-slate-300 hover:bg-slate-100 text-slate-700"
                      title="Abrir terminal de logs SSE"
                    >
                      <Terminal className="h-3 w-3 text-emerald-600" />
                      <span>Logs</span>
                    </Button>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  );
};
