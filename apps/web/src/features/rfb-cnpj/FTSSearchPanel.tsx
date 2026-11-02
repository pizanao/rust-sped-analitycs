import React, { useState, useMemo } from 'react';
import {
  Search,
  Building2,
  Users,
  MapPin,
  Activity,
  TrendingUp,
  Clock,
  Filter,
  X
} from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';

interface SearchResult {
  id: string;
  type: 'empresa' | 'socio' | 'estabelecimento';
  cnpj?: string;
  cpf?: string;
  razaoSocial: string;
  nomeFantasia?: string;
  situacao: 'Ativa' | 'Baixada' | 'Suspensa' | 'Inapta';
  cnae?: string;
  municipio?: string;
  uf?: string;
  capitalSocial?: number;
  dataAbertura?: string;
  score: number;
  highlights: string[];
}

const MOCK_RESULTS: SearchResult[] = [
  {
    id: '1',
    type: 'empresa',
    cnpj: '00.000.000/0001-91',
    razaoSocial: 'PETRÓLEO BRASILEIRO S.A. - PETROBRAS',
    nomeFantasia: 'PETROBRAS',
    situacao: 'Ativa',
    cnae: '19.21-7-00 - Refino de petróleo',
    municipio: 'Rio de Janeiro',
    uf: 'RJ',
    capitalSocial: 85472000000,
    dataAbertura: '1953-10-03',
    score: 0.98,
    highlights: ['<mark>PETRÓLEO</mark> BRASILEIRO S.A.', 'Refino de <mark>petróleo</mark>']
  },
  {
    id: '2',
    type: 'empresa',
    cnpj: '33.000.167/0001-01',
    razaoSocial: 'PETROBRAS DISTRIBUIDORA S.A.',
    nomeFantasia: 'BR PETROBRAS',
    situacao: 'Ativa',
    cnae: '47.31-5-00 - Comércio varejista de combustíveis',
    municipio: 'Rio de Janeiro',
    uf: 'RJ',
    capitalSocial: 2430000000,
    dataAbertura: '1972-04-29',
    score: 0.95,
    highlights: ['<mark>PETROBRAS</mark> DISTRIBUIDORA S.A.']
  },
  {
    id: '3',
    type: 'socio',
    cpf: '000.000.000-00',
    razaoSocial: 'JOÃO SILVA SANTOS',
    situacao: 'Ativa',
    municipio: 'São Paulo',
    uf: 'SP',
    score: 0.89,
    highlights: ['Participa em 3 empresas de <mark>petróleo</mark>']
  }
];

const renderHighlightedText = (text: string): React.ReactNode => {
  const parts = text.split(/(<mark>.*?<\/mark>)/gi);
  return parts.map((part, i) => {
    const match = part.match(/^<mark>(.*?)<\/mark>$/i);
    if (match) {
      return <mark key={i}>{match[1]}</mark>;
    }
    return <React.Fragment key={i}>{part}</React.Fragment>;
  });
};

export const FTSSearchPanel: React.FC = () => {
  const [query, setQuery] = useState('petróleo');
  const [results, setResults] = useState<SearchResult[]>(MOCK_RESULTS);
  const [isLoading, setIsLoading] = useState(false);
  const [filters, setFilters] = useState<{
    tipo: 'all' | 'empresa' | 'socio' | 'estabelecimento';
    situacao: 'all' | 'Ativa' | 'Baixada' | 'Suspensa' | 'Inapta';
    uf: string;
  }>({
    tipo: 'all',
    situacao: 'all',
    uf: ''
  });

  const filteredResults = useMemo(() => {
    return results.filter(r => {
      if (filters.tipo !== 'all' && r.type !== filters.tipo) return false;
      if (filters.situacao !== 'all' && r.situacao !== filters.situacao) return false;
      if (filters.uf && r.uf !== filters.uf) return false;
      return true;
    });
  }, [results, filters]);

  const formatCurrency = (value: number) => {
    return new Intl.NumberFormat('pt-BR', {
      style: 'currency',
      currency: 'BRL',
      notation: 'compact',
      compactDisplay: 'short'
    }).format(value);
  };

  const handleSearch = async () => {
    setIsLoading(true);
    // Simulate search
    setTimeout(() => {
      setResults(MOCK_RESULTS);
      setIsLoading(false);
    }, 500);
  };

  const clearFilters = () => {
    setFilters({
      tipo: 'all',
      situacao: 'all',
      uf: ''
    });
  };

  const getTypeIcon = (type: SearchResult['type']) => {
    switch (type) {
      case 'empresa': return <Building2 className="h-4 w-4 text-blue-600" />;
      case 'socio': return <Users className="h-4 w-4 text-emerald-600" />;
      case 'estabelecimento': return <MapPin className="h-4 w-4 text-purple-600" />;
    }
  };

  const getSituacaoColor = (situacao: SearchResult['situacao']) => {
    switch (situacao) {
      case 'Ativa': return 'bg-emerald-100 text-emerald-700 border-emerald-200';
      case 'Baixada': return 'bg-slate-100 text-slate-700 border-slate-200';
      case 'Suspensa': return 'bg-amber-100 text-amber-700 border-amber-200';
      case 'Inapta': return 'bg-red-100 text-red-700 border-red-200';
    }
  };

  return (
    <Card className="border-slate-200">
      <CardHeader className="border-b border-slate-100">
        <div className="flex items-center justify-between">
          <CardTitle className="text-base font-bold flex items-center gap-2">
            <Search className="h-5 w-5 text-blue-600" />
            Full-Text Search (SQLite FTS5)
          </CardTitle>
          <div className="flex items-center gap-2">
            <Badge className="bg-emerald-100 text-emerald-700 border-emerald-200">
              {filteredResults.length} results
            </Badge>
            <Badge variant="outline" className="text-xs">
              54.8M records indexed
            </Badge>
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Search Bar */}
        <div className="flex items-center gap-2">
          <div className="flex-1 relative">
            <Input
              type="text"
              placeholder="Search by razão social, CNPJ, CNAE, sócio..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyPress={(e) => e.key === 'Enter' && handleSearch()}
              className="pr-10 font-mono"
            />
            <Search className="absolute right-3 top-1/2 -translate-y-1/2 h-4 w-4 text-slate-400" />
          </div>
          <Button onClick={handleSearch} disabled={isLoading}>
            {isLoading ? (
              <>
                <Activity className="h-4 w-4 mr-2 animate-pulse" />
                Searching...
              </>
            ) : (
              <>
                <Search className="h-4 w-4 mr-2" />
                Search
              </>
            )}
          </Button>
        </div>

        {/* Filters */}
        <div className="flex items-center gap-2 flex-wrap">
          <div className="flex items-center gap-1">
            <Filter className="h-3 w-3 text-slate-400" />
            <span className="text-xs font-semibold text-slate-500">Filters:</span>
          </div>

          <select
            value={filters.tipo}
            onChange={(e) => setFilters({ ...filters, tipo: e.target.value as typeof filters.tipo })}
            className="text-xs border border-slate-200 rounded px-2 py-1 bg-white"
          >
            <option value="all">All Types</option>
            <option value="empresa">Empresa</option>
            <option value="socio">Sócio</option>
            <option value="estabelecimento">Estabelecimento</option>
          </select>

          <select
            value={filters.situacao}
            onChange={(e) => setFilters({ ...filters, situacao: e.target.value as typeof filters.situacao })}
            className="text-xs border border-slate-200 rounded px-2 py-1 bg-white"
          >
            <option value="all">All Situations</option>
            <option value="Ativa">Ativa</option>
            <option value="Baixada">Baixada</option>
            <option value="Suspensa">Suspensa</option>
            <option value="Inapta">Inapta</option>
          </select>

          <input
            type="text"
            placeholder="UF"
            value={filters.uf}
            onChange={(e) => setFilters({ ...filters, uf: e.target.value.toUpperCase() })}
            className="text-xs border border-slate-200 rounded px-2 py-1 w-16 bg-white uppercase"
            maxLength={2}
          />

          <Button
            variant="ghost"
            size="sm"
            onClick={clearFilters}
            className="text-xs"
          >
            <X className="h-3 w-3 mr-1" />
            Clear
          </Button>
        </div>

        {/* Results */}
        <div className="space-y-3">
          {filteredResults.map((result) => (
            <div
              key={result.id}
              className="p-4 rounded-lg border border-slate-200 bg-white hover:bg-slate-50 transition cursor-pointer"
            >
              <div className="flex items-start justify-between">
                <div className="flex items-start gap-3">
                  <div className={`h-10 w-10 rounded-lg flex items-center justify-center ${
                    result.type === 'empresa' ? 'bg-blue-100' :
                    result.type === 'socio' ? 'bg-emerald-100' : 'bg-purple-100'
                  }`}>
                    {getTypeIcon(result.type)}
                  </div>
                  <div className="flex-1">
                    <div className="flex items-center gap-2 mb-1">
                      <h4 className="font-semibold text-slate-900">
                        {renderHighlightedText(result.razaoSocial)}
                      </h4>
                      <Badge className={getSituacaoColor(result.situacao)}>
                        {result.situacao}
                      </Badge>
                    </div>

                    <div className="flex items-center gap-3 text-xs text-slate-500 mt-1">
                      {result.cnpj && (
                        <span className="font-mono">{result.cnpj}</span>
                      )}
                      {result.cpf && (
                        <span className="font-mono">{result.cpf}</span>
                      )}
                      {result.cnae && (
                        <span>{result.cnae}</span>
                      )}
                    </div>

                    {result.municipio && result.uf && (
                      <div className="flex items-center gap-1 text-xs text-slate-500 mt-1">
                        <MapPin className="h-3 w-3" />
                        <span>{result.municipio}, {result.uf}</span>
                      </div>
                    )}

                    {/* Highlights */}
                    {result.highlights.length > 0 && (
                      <div className="mt-2 flex flex-wrap gap-1">
                        {result.highlights.map((h, idx) => (
                          <Badge
                            key={idx}
                            variant="outline"
                            className="text-[10px] bg-blue-50 border-blue-200 text-blue-700"
                          >
                            {renderHighlightedText(h)}
                          </Badge>
                        ))}
                      </div>
                    )}
                  </div>
                </div>

                <div className="flex flex-col items-end gap-2">
                  <div className="flex items-center gap-1">
                    <TrendingUp className="h-3 w-3 text-emerald-600" />
                    <span className="text-xs font-semibold text-emerald-600">
                      {(result.score * 100).toFixed(0)}% match
                    </span>
                  </div>
                  {result.capitalSocial && (
                    <span className="text-sm font-semibold text-slate-900">
                      {formatCurrency(result.capitalSocial)}
                    </span>
                  )}
                  {result.dataAbertura && (
                    <div className="flex items-center gap-1 text-xs text-slate-500">
                      <Clock className="h-3 w-3" />
                      <span>{result.dataAbertura}</span>
                    </div>
                  )}
                </div>
              </div>
            </div>
          ))}

          {filteredResults.length === 0 && (
            <div className="text-center py-12">
              <Search className="h-10 w-10 text-slate-300 mx-auto mb-3" />
              <h4 className="font-semibold text-slate-700">No results found</h4>
              <p className="text-xs text-slate-500 mt-1">
                Try adjusting your search query or filters
              </p>
            </div>
          )}
        </div>

        {/* Stats */}
        <div className="grid grid-cols-3 gap-2 pt-4 border-t border-slate-200">
          <div className="text-center">
            <div className="text-xs font-semibold text-slate-500">Query Time</div>
            <div className="text-lg font-bold text-blue-600">23ms</div>
          </div>
          <div className="text-center">
            <div className="text-xs font-semibold text-slate-500">Index Size</div>
            <div className="text-lg font-bold text-purple-600">12.5 GB</div>
          </div>
          <div className="text-center">
            <div className="text-xs font-semibold text-slate-500">Total Records</div>
            <div className="text-lg font-bold text-emerald-600">54.8M</div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
};
