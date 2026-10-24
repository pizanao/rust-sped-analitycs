import React, { useState, useMemo } from 'react';
import {
  useReactTable,
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  flexRender,
  type ColumnDef,
} from '@tanstack/react-table';
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@/components/ui/table';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Search, ChevronLeft, ChevronRight, FileSpreadsheet } from 'lucide-react';
import type {
  Registro0000,
  RegistroJ100,
  RegJ150,
  RegistroI155,
} from '@/types';

interface SpedBlocksTabsProps {
  bloco0000: Registro0000[];
  blocoJ100: RegistroJ100[];
  blocoJ150: RegJ150[];
  blocoI155: RegistroI155[];
}

export const SpedBlocksTabs: React.FC<SpedBlocksTabsProps> = ({
  bloco0000,
  blocoJ100,
  blocoJ150,
  blocoI155,
}) => {
  const [activeTab, setActiveTab] = useState('j150');

  const formatCurrency = (val: number): string => {
    return new Intl.NumberFormat('pt-BR', {
      style: 'currency',
      currency: 'BRL',
    }).format(val);
  };

  // Colunas do Bloco 0000
  const cols0000 = useMemo<ColumnDef<Registro0000>[]>(
    () => [
      { accessorKey: 'nome_razao', header: 'Razão Social' },
      { accessorKey: 'cnpj', header: 'CNPJ' },
      { accessorKey: 'uf', header: 'UF' },
      { accessorKey: 'dt_ini', header: 'Data Inicial' },
      { accessorKey: 'dt_fim', header: 'Data Final' },
      {
        accessorKey: 'ind_grande_porte',
        header: 'Porte',
        cell: (info) => (info.getValue() === 1 ? 'Grande Porte' : 'Normal'),
      },
      {
        accessorKey: 'hash_arquivo_id',
        header: 'Hash SPED',
        cell: (info) => (
          <span className="font-mono text-xs">{String(info.getValue()).slice(0, 12)}...</span>
        ),
      },
    ],
    []
  );

  // Colunas do Bloco J100 (Balanço)
  const colsJ100 = useMemo<ColumnDef<RegistroJ100>[]>(
    () => [
      { accessorKey: 'cod_agl', header: 'Cód. Aglutinação' },
      { accessorKey: 'descr_cod_agl', header: 'Descrição da Rubrica' },
      { accessorKey: 'nivel_agl', header: 'Nível' },
      { accessorKey: 'ind_grp_bal', header: 'Grupo' },
      {
        accessorKey: 'vl_cta_ini',
        header: 'Saldo Inicial',
        cell: (info) => (
          <span className="font-mono text-right block">
            {formatCurrency(info.getValue() as number)}
          </span>
        ),
      },
      { accessorKey: 'ind_dc_cta_ini', header: 'D/C Ini' },
      {
        accessorKey: 'vl_cta_fin',
        header: 'Saldo Final',
        cell: (info) => (
          <span className="font-mono text-right block font-semibold">
            {formatCurrency(info.getValue() as number)}
          </span>
        ),
      },
      { accessorKey: 'ind_dc_cta_fin', header: 'D/C Fin' },
    ],
    []
  );

  // Colunas do Bloco J150 (DRE)
  const colsJ150 = useMemo<ColumnDef<RegJ150>[]>(
    () => [
      { accessorKey: 'nu_ordem', header: 'Ordem' },
      { accessorKey: 'cod_agl', header: 'Cód. Aglutinação' },
      { accessorKey: 'descr_cod_agl', header: 'Descrição da Rubrica DRE' },
      { accessorKey: 'nivel_agl', header: 'Nível' },
      {
        accessorKey: 'vl_cta',
        header: 'Valor Contábil',
        cell: (info) => (
          <span className="font-mono text-right block font-bold text-slate-900">
            {formatCurrency(info.getValue() as number)}
          </span>
        ),
      },
      {
        accessorKey: 'ind_dc_cta',
        header: 'D/C',
        cell: (info) => {
          const val = String(info.getValue());
          return (
            <Badge variant={val === 'C' ? 'success' : 'secondary'} className="text-[10px]">
              {val === 'C' ? 'Crédito (Receita)' : 'Débito (Custo/Desp)'}
            </Badge>
          );
        },
      },
      { accessorKey: 'ind_grp_dre', header: 'Grupo DRE' },
    ],
    []
  );

  // Colunas do Bloco I155 (Saldos de Contas)
  const colsI155 = useMemo<ColumnDef<RegistroI155>[]>(
    () => [
      { accessorKey: 'cod_cta', header: 'Código da Conta' },
      {
        accessorKey: 'vl_sld_ini',
        header: 'Saldo Inicial',
        cell: (info) => (
          <span className="font-mono text-right block">
            {formatCurrency(info.getValue() as number)}
          </span>
        ),
      },
      { accessorKey: 'ind_dc_ini', header: 'D/C' },
      {
        accessorKey: 'vl_deb',
        header: 'Débitos',
        cell: (info) => (
          <span className="font-mono text-right block text-blue-600">
            {formatCurrency(info.getValue() as number)}
          </span>
        ),
      },
      {
        accessorKey: 'vl_cred',
        header: 'Créditos',
        cell: (info) => (
          <span className="font-mono text-right block text-emerald-600">
            {formatCurrency(info.getValue() as number)}
          </span>
        ),
      },
      {
        accessorKey: 'vl_sld_fim',
        header: 'Saldo Final',
        cell: (info) => (
          <span className="font-mono text-right block font-bold">
            {formatCurrency(info.getValue() as number)}
          </span>
        ),
      },
      { accessorKey: 'ind_dc_fim', header: 'D/C' },
    ],
    []
  );

interface SpedTableProps<T extends object> {
  data: T[];
  columns: ColumnDef<T>[];
}

function SpedTable<T extends object>({ data, columns }: SpedTableProps<T>) {
  const [tableFilter, setTableFilter] = useState('');

  const table = useReactTable({
    data,
    columns,
    state: { globalFilter: tableFilter },
    onGlobalFilterChange: setTableFilter,
    getCoreRowModel: getCoreRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    initialState: {
      pagination: { pageSize: 15 },
    },
  });

  return (
    <div className="space-y-3">
      <div className="flex justify-between items-center">
        <div className="relative w-72">
          <Search className="absolute left-3 top-2.5 h-3.5 w-3.5 text-slate-400" />
          <Input
            value={tableFilter ?? ''}
            onChange={(e) => setTableFilter(e.target.value)}
            placeholder="Filtrar nesta tabela..."
            className="h-8 pl-8 text-xs bg-white"
          />
        </div>
        <span className="text-xs text-slate-500 font-mono">
          {table.getFilteredRowModel().rows.length} registros
        </span>
      </div>

      <div className="rounded-lg border border-slate-200 bg-white shadow-sm overflow-hidden">
        <Table>
          <TableHeader>
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id}>
                {headerGroup.headers.map((header) => (
                  <TableHead key={header.id} className="text-xs font-bold text-slate-700">
                    {header.isPlaceholder
                      ? null
                      : flexRender(header.column.columnDef.header, header.getContext())}
                  </TableHead>
                ))}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {table.getRowModel().rows.length === 0 ? (
              <TableRow>
                <TableCell colSpan={columns.length} className="h-24 text-center text-slate-500 text-xs">
                  Nenhum registro encontrado.
                </TableCell>
              </TableRow>
            ) : (
              table.getRowModel().rows.map((row) => (
                <TableRow key={row.id} className="text-xs hover:bg-slate-50">
                  {row.getVisibleCells().map((cell) => (
                    <TableCell key={cell.id}>
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>

      {/* Pagination controls */}
      <div className="flex items-center justify-between text-xs text-slate-600 px-1">
        <div>
          Página {table.getState().pagination.pageIndex + 1} de {table.getPageCount() || 1}
        </div>
        <div className="flex items-center gap-1">
          <Button
            variant="outline"
            size="sm"
            onClick={() => table.previousPage()}
            disabled={!table.getCanPreviousPage()}
            className="h-7 w-7 p-0"
          >
            <ChevronLeft className="h-4 w-4" />
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => table.nextPage()}
            disabled={!table.getCanNextPage()}
            className="h-7 w-7 p-0"
          >
            <ChevronRight className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>
  );
}

  return (
    <div className="space-y-4">
      <Tabs value={activeTab} onValueChange={setActiveTab} className="w-full">
        <TabsList className="bg-slate-200/80 p-1">
          <TabsTrigger value="j150" className="gap-2 text-xs">
            <FileSpreadsheet className="h-3.5 w-3.5 text-blue-600" />
            <span>DRE (Bloco J150)</span>
            <Badge variant="secondary" className="text-[10px] py-0 px-1.5 ml-1">
              {blocoJ150.length}
            </Badge>
          </TabsTrigger>
          <TabsTrigger value="j100" className="gap-2 text-xs">
            <FileSpreadsheet className="h-3.5 w-3.5 text-emerald-600" />
            <span>Balanço (Bloco J100)</span>
            <Badge variant="secondary" className="text-[10px] py-0 px-1.5 ml-1">
              {blocoJ100.length}
            </Badge>
          </TabsTrigger>
          <TabsTrigger value="i155" className="gap-2 text-xs">
            <FileSpreadsheet className="h-3.5 w-3.5 text-amber-600" />
            <span>Saldos de Contas (Bloco I155)</span>
            <Badge variant="secondary" className="text-[10px] py-0 px-1.5 ml-1">
              {blocoI155.length}
            </Badge>
          </TabsTrigger>
          <TabsTrigger value="0000" className="gap-2 text-xs">
            <span>Abertura (Bloco 0000)</span>
            <Badge variant="secondary" className="text-[10px] py-0 px-1.5 ml-1">
              {bloco0000.length}
            </Badge>
          </TabsTrigger>
        </TabsList>

        <TabsContent value="j150">
          <SpedTable data={blocoJ150} columns={colsJ150} />
        </TabsContent>
        <TabsContent value="j100">
          <SpedTable data={blocoJ100} columns={colsJ100} />
        </TabsContent>
        <TabsContent value="i155">
          <SpedTable data={blocoI155} columns={colsI155} />
        </TabsContent>
        <TabsContent value="0000">
          <SpedTable data={bloco0000} columns={cols0000} />
        </TabsContent>
      </Tabs>
    </div>
  );
};
