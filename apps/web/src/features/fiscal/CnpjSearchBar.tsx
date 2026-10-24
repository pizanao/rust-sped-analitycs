import React, { useState } from 'react';
import { Search, CheckCircle2, AlertCircle, Building2, ArrowRight } from 'lucide-react';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { formatarCnpj, validarCnpj, apenasDigitos } from '@/lib/cnpj';

interface CnpjSearchBarProps {
  onSearch: (cnpj: string) => void;
  isLoading: boolean;
  initialValue?: string;
}

export const CnpjSearchBar: React.FC<CnpjSearchBarProps> = ({
  onSearch,
  isLoading,
  initialValue = '',
}) => {
  const [value, setValue] = useState(formatarCnpj(initialValue));
  const digits = apenasDigitos(value);
  const isComplete = digits.length === 14;
  const isValid = isComplete && validarCnpj(digits);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const raw = e.target.value;
    const formatted = formatarCnpj(raw);
    setValue(formatted);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (isValid) {
      onSearch(digits);
    }
  };

  const handleSelectExample = (cnpjExample: string) => {
    const formatted = formatarCnpj(cnpjExample);
    setValue(formatted);
    onSearch(apenasDigitos(cnpjExample));
  };

  return (
    <div className="space-y-3">
      <form onSubmit={handleSubmit} className="flex flex-col sm:flex-row gap-2">
        <div className="relative flex-1">
          <Building2 className="absolute left-3 top-3 h-4 w-4 text-slate-400" />
          <Input
            value={value}
            onChange={handleChange}
            placeholder="Digite o CNPJ da empresa (ex: 00.000.000/0001-91)"
            className="h-10 pl-9 font-mono text-sm bg-white border-slate-300"
            maxLength={18}
          />

          <div className="absolute right-3 top-2.5">
            {isComplete && (
              isValid ? (
                <Badge variant="success" className="gap-1 text-[10px] py-0.5">
                  <CheckCircle2 className="h-3 w-3" />
                  <span>Dígito Válido</span>
                </Badge>
              ) : (
                <Badge variant="destructive" className="gap-1 text-[10px] py-0.5">
                  <AlertCircle className="h-3 w-3" />
                  <span>Dígito Inválido</span>
                </Badge>
              )
            )}
          </div>
        </div>

        <Button
          type="submit"
          disabled={!isValid || isLoading}
          className="h-10 px-5 gap-2 bg-blue-600 hover:bg-blue-700 text-white shadow-sm"
        >
          {isLoading ? (
            <div className="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent" />
          ) : (
            <Search className="h-4 w-4" />
          )}
          <span>Consultar Escriturações</span>
          <ArrowRight className="h-4 w-4" />
        </Button>
      </form>

      {/* Sugestões rápidas de CNPJ para demonstração */}
      <div className="flex flex-wrap items-center gap-2 text-xs text-slate-500">
        <span className="font-medium text-slate-600">Exemplos rápidos:</span>
        {[
          { label: 'Petrobras', cnpj: '33000167000101' },
          { label: 'Vale S.A.', cnpj: '33592510000154' },
          { label: 'Ambev', cnpj: '07526557000100' },
          { label: 'Empresa Teste', cnpj: '00000000000191' },
        ].map((item) => (
          <button
            key={item.cnpj}
            type="button"
            onClick={() => handleSelectExample(item.cnpj)}
            className="rounded-full bg-slate-100 hover:bg-blue-50 hover:text-blue-700 px-2.5 py-1 text-[11px] font-mono text-slate-600 transition border border-slate-200"
          >
            {item.label}: {formatarCnpj(item.cnpj)}
          </button>
        ))}
      </div>
    </div>
  );
};
