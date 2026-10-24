import React from 'react';
import {
  UploadCloud,
  FileSpreadsheet,
  Network,
  BarChart3,
  Bot,
  ChevronLeft,
  ChevronRight,
  ShieldCheck,
  Cpu,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';

export type NavItem = 'ingestao' | 'fiscal' | 'grc' | 'indicadores';

interface SidebarProps {
  activeTab: NavItem;
  onTabChange: (tab: NavItem) => void;
  collapsed: boolean;
  onToggleCollapse: () => void;
  onToggleCopilot: () => void;
  isCopilotOpen: boolean;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeTab,
  onTabChange,
  collapsed,
  onToggleCollapse,
  onToggleCopilot,
  isCopilotOpen,
}) => {
  const navItems = [
    {
      id: 'ingestao' as NavItem,
      label: 'Ingestão SPED',
      icon: UploadCloud,
      badge: 'Zero-Copy',
      description: 'Upload streaming & monitor SSE',
    },
    {
      id: 'fiscal' as NavItem,
      label: 'Auditoria & DRE',
      icon: FileSpreadsheet,
      badge: 'J150/I355',
      description: 'Consulta por CNPJ e balanço',
    },
    {
      id: 'grc' as NavItem,
      label: 'Rede Societária GRC',
      icon: Network,
      badge: 'Vis-Network',
      description: 'Grafo CNPJ e sanções PEP/CEIS',
    },
    {
      id: 'indicadores' as NavItem,
      label: 'Indicadores Econômicos',
      icon: BarChart3,
      badge: 'Recharts',
      description: 'CNAE, Porte e Regime',
    },
  ];

  return (
    <aside
      className={cn(
        'relative border-r border-slate-200 bg-white transition-all duration-300 flex flex-col justify-between select-none z-20',
        collapsed ? 'w-18' : 'w-64'
      )}
    >
      {/* Brand & Collapse Button */}
      <div>
        <div className="flex h-16 items-center justify-between border-b border-slate-100 px-4">
          <div className="flex items-center gap-3 overflow-hidden">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-blue-600 font-bold text-white shadow-sm">
              <Cpu className="h-5 w-5" />
            </div>
            {!collapsed && (
              <div className="truncate">
                <span className="block font-bold text-sm tracking-tight text-slate-900">
                  Rust Sped Analytics
                </span>
                <span className="block text-[11px] font-medium text-slate-400">
                  Rust Engine + React
                </span>
              </div>
            )}
          </div>
          <button
            onClick={onToggleCollapse}
            aria-label={collapsed ? 'Expandir menu lateral' : 'Recolher menu lateral'}
            className="flex h-7 w-7 items-center justify-center rounded-md border border-slate-200 text-slate-500 hover:bg-slate-100 hover:text-slate-800 transition"
          >
            {collapsed ? <ChevronRight className="h-4 w-4" /> : <ChevronLeft className="h-4 w-4" />}
          </button>
        </div>

        {/* Navigation */}
        <nav className="p-3 space-y-1.5">
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = activeTab === item.id;
            return (
              <button
                key={item.id}
                onClick={() => onTabChange(item.id)}
                title={collapsed ? item.label : undefined}
                className={cn(
                  'flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-all text-left cursor-pointer',
                  isActive
                    ? 'bg-blue-50 text-blue-700 shadow-sm ring-1 ring-blue-600/10'
                    : 'text-slate-600 hover:bg-slate-100 hover:text-slate-900'
                )}
              >
                <Icon className={cn('h-4 w-4 shrink-0', isActive ? 'text-blue-600' : 'text-slate-500')} />
                {!collapsed && (
                  <div className="flex flex-1 items-center justify-between overflow-hidden">
                    <span className="truncate">{item.label}</span>
                    {item.badge && (
                      <span className="ml-1 text-[10px] font-semibold text-slate-400">
                        {item.badge}
                      </span>
                    )}
                  </div>
                )}
              </button>
            );
          })}
        </nav>
      </div>

      {/* Footer / Copilot Trigger */}
      <div className="p-3 border-t border-slate-100 space-y-2">
        <Button
          variant={isCopilotOpen ? 'default' : 'outline'}
          size="sm"
          onClick={onToggleCopilot}
          className={cn(
            'w-full flex items-center justify-center gap-2 text-xs font-semibold',
            isCopilotOpen ? 'bg-indigo-600 hover:bg-indigo-700 text-white' : 'border-indigo-200 text-indigo-700 hover:bg-indigo-50'
          )}
        >
          <Bot className="h-4 w-4 shrink-0" />
          {!collapsed && <span>Copiloto de IA</span>}
        </Button>

        {!collapsed && (
          <div className="rounded-lg bg-slate-50 p-2.5 border border-slate-200/60">
            <div className="flex items-center gap-1.5 text-emerald-700 text-xs font-medium mb-1">
              <ShieldCheck className="h-3.5 w-3.5" />
              <span>Conformidade SPED</span>
            </div>
            <p className="text-[11px] text-slate-500">
              100% Core Preservado no PostgreSQL 16.
            </p>
          </div>
        )}
      </div>
    </aside>
  );
};
