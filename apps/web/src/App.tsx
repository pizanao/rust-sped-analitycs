import React from 'react';
import { Activity, Database, Network, FileSpreadsheet, Bot } from 'lucide-react';

export default function App() {
  return (
    <div className="flex h-screen bg-slate-50 text-slate-900 font-sans">
      {/* Sidebar */}
      <aside className="w-64 border-r border-slate-200 bg-white p-5 flex flex-col justify-between">
        <div>
          <div className="flex items-center gap-2 mb-8">
            <div className="h-8 w-8 rounded-lg bg-blue-600 flex items-center justify-center text-white font-bold">
              EC
            </div>
            <div>
              <h1 className="font-semibold text-sm">ECD Analytics</h1>
              <p className="text-xs text-slate-500">Speed Rust Engine</p>
            </div>
          </div>

          <nav className="space-y-1">
            <button className="flex w-full items-center gap-3 rounded-md bg-blue-50 px-3 py-2 text-sm font-medium text-blue-700">
              <Activity className="h-4 w-4" /> Visão Geral & Ingestão
            </button>
            <button className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-100">
              <Database className="h-4 w-4" /> Auditoria Fiscal
            </button>
            <button className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-100">
              <Network className="h-4 w-4" /> Rede CNPJ & GRC
            </button>
            <button className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-100">
              <FileSpreadsheet className="h-4 w-4" /> Demonstrativos SPED
            </button>
          </nav>
        </div>

        <div className="rounded-lg bg-blue-50 p-3">
          <div className="flex items-center gap-2 text-blue-800 text-xs font-semibold mb-1">
            <Bot className="h-4 w-4" /> Copiloto de IA Ativo
          </div>
          <p className="text-[11px] text-blue-600">LangChain Gateway conectado.</p>
        </div>
      </aside>

      {/* Main Content Area */}
      <main className="flex-1 p-8 overflow-y-auto">
        <header className="mb-6 flex justify-between items-center">
          <div>
            <h2 className="text-2xl font-bold tracking-tight">Painel de Ingestão & Auditoria</h2>
            <p className="text-sm text-slate-500">
              Processamento de alta performance em Rust e análise em tempo real.
            </p>
          </div>
          <div className="flex gap-2">
            <span className="inline-flex items-center rounded-full bg-emerald-50 px-3 py-1 text-xs font-medium text-emerald-700 ring-1 ring-inset ring-emerald-600/20">
              Motor Rust Online (Zero-Copy)
            </span>
          </div>
        </header>

        <section className="grid grid-cols-1 md:grid-cols-3 gap-6 mb-8">
          <div className="p-6 bg-white rounded-xl border border-slate-200 shadow-sm">
            <span className="text-xs font-medium text-slate-500 uppercase">Arquivos Processados</span>
            <div className="text-3xl font-extrabold text-slate-900 mt-2">1.240</div>
            <p className="text-xs text-emerald-600 mt-1">Tempo médio: 4.8s / 500MB</p>
          </div>
          <div className="p-6 bg-white rounded-xl border border-slate-200 shadow-sm">
            <span className="text-xs font-medium text-slate-500 uppercase">Receita Bruta Total</span>
            <div className="text-3xl font-extrabold text-slate-900 mt-2">R$ 48,2 Mi</div>
            <p className="text-xs text-slate-500 mt-1">Apuração via DRE (J150 / I355)</p>
          </div>
          <div className="p-6 bg-white rounded-xl border border-slate-200 shadow-sm">
            <span className="text-xs font-medium text-slate-500 uppercase">Alertas Sancionatórios</span>
            <div className="text-3xl font-extrabold text-amber-600 mt-2">14</div>
            <p className="text-xs text-amber-700 mt-1">PEP, CEIS e PGFN identificados</p>
          </div>
        </section>
      </main>
    </div>
  );
}
