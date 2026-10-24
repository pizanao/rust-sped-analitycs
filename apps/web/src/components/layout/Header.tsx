import React from 'react';
import {
  Shield,
  Activity,
  Bot,
  RefreshCw,
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

interface HeaderProps {
  title: string;
  subtitle: string;
  onOpenCopilot: () => void;
  isCopilotOpen: boolean;
  onRefresh?: () => void;
  isRefreshing?: boolean;
}

export const Header: React.FC<HeaderProps> = ({
  title,
  subtitle,
  onOpenCopilot,
  isCopilotOpen,
  onRefresh,
  isRefreshing,
}) => {
  return (
    <header className="flex h-16 shrink-0 items-center justify-between border-b border-slate-200 bg-white px-6">
      <div>
        <h1 className="text-lg font-bold tracking-tight text-slate-900 flex items-center gap-2">
          {title}
        </h1>
        <p className="text-xs text-slate-500">{subtitle}</p>
      </div>

      <div className="flex items-center gap-3">
        {onRefresh && (
          <Button
            variant="outline"
            size="sm"
            onClick={onRefresh}
            disabled={isRefreshing}
            className="h-8 gap-1.5 text-xs text-slate-600 border-slate-200"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${isRefreshing ? 'animate-spin' : ''}`} />
            <span>Atualizar</span>
          </Button>
        )}

        <Badge variant="success" className="gap-1 text-[11px] font-medium py-1">
          <Activity className="h-3 w-3 text-emerald-600 animate-pulse" />
          <span>Worker Ativo (Redis Queue)</span>
        </Badge>

        <Badge variant="outline" className="gap-1 text-[11px] font-medium py-1 text-slate-600">
          <Shield className="h-3 w-3 text-blue-600" />
          <span>JWT HttpOnly</span>
        </Badge>

        <Button
          variant={isCopilotOpen ? 'default' : 'outline'}
          size="sm"
          onClick={onOpenCopilot}
          className="h-8 gap-1.5 text-xs bg-indigo-600 hover:bg-indigo-700 text-white"
        >
          <Bot className="h-3.5 w-3.5" />
          <span>Copiloto de IA</span>
        </Button>
      </div>
    </header>
  );
};
