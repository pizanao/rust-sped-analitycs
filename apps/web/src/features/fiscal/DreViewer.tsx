import React, { useState } from 'react';
import {
  DollarSign,
  TrendingUp,
  FileSpreadsheet,
  Download,
  CheckCircle2,
  Sparkles,
} from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { api } from '@/services/api';
import type { ResultadoExercicio } from '@/types';

interface DreViewerProps {
  resultado: ResultadoExercicio;
  cnpj: string;
}

export const DreViewer: React.FC<DreViewerProps> = ({ resultado, cnpj }) => {
  const [isExporting, setIsExporting] = useState(false);

  const formatCurrency = (val: number): string => {
    return new Intl.NumberFormat('pt-BR', {
      style: 'currency',
      currency: 'BRL',
    }).format(val);
  };

  const handleExportXlsx = async () => {
    try {
      setIsExporting(true);
      await api.fiscal.baixarXlsx(cnpj, resultado.razao_social);
    } catch (err) {
      console.error('Falha ao exportar planilha XLSX:', err);
      // Fallback: abrir em nova aba
      window.open(api.fiscal.exportarXlsxUrl(cnpj), '_blank');
    } finally {
      setIsExporting(false);
    }
  };

  const aderenciaFiscal =
    resultado.total_nota > 0
      ? ((resultado.receita_bruta / resultado.total_nota) * 100).toFixed(1)
      : '100.0';

  return (
    <div className="space-y-6">
      {/* Top Banner with Company Info & Export Button */}
      <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 bg-white p-5 rounded-xl border border-slate-200 shadow-sm">
        <div>
          <div className="flex items-center gap-2">
            <h3 className="text-lg font-bold text-slate-900">{resultado.razao_social}</h3>
            <Badge variant="outline" className="border-blue-200 bg-blue-50 text-blue-700">
              {resultado.situacao || 'Regular'}
            </Badge>
          </div>
          <p className="text-xs text-slate-500 mt-0.5">
            Período Contábil: {resultado.periodo_inicial} a {resultado.periodo_final} | Hash SPED:{' '}
            <span className="font-mono text-slate-700">{resultado.hash_arquivo.slice(0, 16)}...</span>
          </p>
        </div>

        <Button
          onClick={handleExportXlsx}
          disabled={isExporting}
          className="gap-2 bg-emerald-600 hover:bg-emerald-700 text-white shadow-sm shrink-0"
        >
          {isExporting ? (
            <div className="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent" />
          ) : (
            <Download className="h-4 w-4" />
          )}
          <FileSpreadsheet className="h-4 w-4" />
          <span>Exportar Relatório XLSX</span>
        </Button>
      </div>

      {/* Financial Metric Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card className="border-blue-100 bg-gradient-to-br from-blue-50/50 to-white">
          <CardHeader className="p-4 pb-2">
            <CardTitle className="text-xs font-semibold text-slate-500 uppercase tracking-wider flex items-center justify-between">
              <span>Receita Bruta Total</span>
              <DollarSign className="h-4 w-4 text-blue-600" />
            </CardTitle>
          </CardHeader>
          <CardContent className="p-4 pt-0">
            <div className="text-2xl font-extrabold text-blue-900">
              {formatCurrency(resultado.receita_bruta)}
            </div>
            <p className="text-[11px] text-blue-700 font-medium mt-1">
              Fuzzy Match partial_ratio ≥ 0.80
            </p>
          </CardContent>
        </Card>

        <Card className="border-slate-200">
          <CardHeader className="p-4 pb-2">
            <CardTitle className="text-xs font-semibold text-slate-500 uppercase tracking-wider flex items-center justify-between">
              <span>Receita de Serviços</span>
              <TrendingUp className="h-4 w-4 text-emerald-600" />
            </CardTitle>
          </CardHeader>
          <CardContent className="p-4 pt-0">
            <div className="text-2xl font-extrabold text-emerald-800">
              {formatCurrency(resultado.receita_servico)}
            </div>
            <p className="text-[11px] text-slate-500 mt-1">Classificação RegJ150 / I355</p>
          </CardContent>
        </Card>

        <Card className="border-slate-200">
          <CardHeader className="p-4 pb-2">
            <CardTitle className="text-xs font-semibold text-slate-500 uppercase tracking-wider flex items-center justify-between">
              <span>Outras Receitas</span>
              <DollarSign className="h-4 w-4 text-slate-500" />
            </CardTitle>
          </CardHeader>
          <CardContent className="p-4 pt-0">
            <div className="text-2xl font-extrabold text-slate-800">
              {formatCurrency(resultado.outras_receitas)}
            </div>
            <p className="text-[11px] text-slate-500 mt-1">Operacionais & Financeiras</p>
          </CardContent>
        </Card>

        <Card className="border-slate-200">
          <CardHeader className="p-4 pb-2">
            <CardTitle className="text-xs font-semibold text-slate-500 uppercase tracking-wider flex items-center justify-between">
              <span>Aderência Fiscal (Notas)</span>
              <CheckCircle2 className="h-4 w-4 text-indigo-600" />
            </CardTitle>
          </CardHeader>
          <CardContent className="p-4 pt-0">
            <div className="text-2xl font-extrabold text-indigo-900">
              {aderenciaFiscal}%
            </div>
            <p className="text-[11px] text-slate-500 mt-1">
              Notas fiscais: {formatCurrency(resultado.total_nota)}
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Rules Notice and Filters Summary */}
      <div className="rounded-lg bg-amber-50/70 border border-amber-200/80 p-4 text-xs text-amber-900 flex items-start gap-3">
        <Sparkles className="h-5 w-5 text-amber-600 shrink-0 mt-0.5" />
        <div>
          <span className="font-semibold">Critérios do Motor Fiscal (ecd-fiscal):</span>
          <p className="mt-1 text-amber-800 leading-relaxed">
            A apuração foi executada aplicando os 24 termos do catálogo da Receita Federal com similaridade textual
            substring superior a 80%, excluindo automaticamente os 12 filtros negativos de deduções (PIS/COFINS/ICMS),
            impostos sobre faturamento e descontos incondicionais da DRE.
          </p>
        </div>
      </div>
    </div>
  );
};
