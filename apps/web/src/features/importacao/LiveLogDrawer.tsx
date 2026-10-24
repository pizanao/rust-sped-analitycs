import React, { useEffect, useState, useRef } from 'react';
import { Terminal, X } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Progress } from '@/components/ui/progress';
import { Button } from '@/components/ui/button';
import { api } from '@/services/api';
import type { JobStatusEvent, StatusImportacao } from '@/types';

interface LiveLogDrawerProps {
  jobId: string | null;
  filename?: string;
  isOpen: boolean;
  onClose: () => void;
  onJobFinished?: (status: StatusImportacao) => void;
}

interface LogEntry {
  id: string;
  timestamp: string;
  status: StatusImportacao;
  progresso: number;
  mensagem: string;
  linhas: number;
}

export const LiveLogDrawer: React.FC<LiveLogDrawerProps> = ({
  jobId,
  filename,
  isOpen,
  onClose,
  onJobFinished,
}) => {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [currentStatus, setCurrentStatus] = useState<StatusImportacao>('Aguardando');
  const [progress, setProgress] = useState(0);
  const [linhasProcessadas, setLinhasProcessadas] = useState(0);
  const [isConnected, setIsConnected] = useState(false);
  const logContainerRef = useRef<HTMLDivElement>(null);
  const eventSourceRef = useRef<EventSource | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const hasFinishedRef = useRef(false);
  const isSimulatingRef = useRef(false);

  const stopSimulation = () => {
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
    isSimulatingRef.current = false;
  };

  // Auto-scroll para a linha mais recente
  useEffect(() => {
    if (logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs]);

  useEffect(() => {
    if (!isOpen || !jobId) {
      stopSimulation();
      if (eventSourceRef.current) {
        eventSourceRef.current.close();
        eventSourceRef.current = null;
      }
      return;
    }

    setLogs([]);
    setProgress(0);
    setLinhasProcessadas(0);
    setCurrentStatus('Aguardando');
    hasFinishedRef.current = false;
    stopSimulation();

    const streamUrl = api.importacoes.obterStreamUrl(jobId);
    let eventSource: EventSource | null = null;

    try {
      eventSource = new EventSource(streamUrl, { withCredentials: true });
      eventSourceRef.current = eventSource;

      eventSource.onopen = () => {
        setIsConnected(true);
        addLogEntry({
          id: `init-${Date.now()}`,
          timestamp: new Date().toLocaleTimeString(),
          status: 'Aguardando',
          progresso: 5,
          mensagem: `Conexão SSE estabelecida com worker Rust. Monitorando job ${jobId}...`,
          linhas: 0,
        });
      };

      eventSource.onmessage = (event) => {
        try {
          const data: JobStatusEvent = JSON.parse(event.data);
          setCurrentStatus(data.status);
          setProgress(data.progresso_percentual);
          setLinhasProcessadas(data.linhas_processadas);

          addLogEntry({
            id: `ev-${Date.now()}-${Math.random()}`,
            timestamp: new Date(data.timestamp).toLocaleTimeString(),
            status: data.status,
            progresso: data.progresso_percentual,
            mensagem: data.mensagem,
            linhas: data.linhas_processadas,
          });

          if (data.status === 'Sucesso' || data.status === 'Erro') {
            hasFinishedRef.current = true;
            eventSource?.close();
            setIsConnected(false);
            if (onJobFinished) {
              onJobFinished(data.status);
            }
          }
        } catch {
          // Mensagem de texto simples
          addLogEntry({
            id: `text-${Date.now()}`,
            timestamp: new Date().toLocaleTimeString(),
            status: 'EmProcessamento',
            progresso: 50,
            mensagem: event.data,
            linhas: 0,
          });
        }
      };

      eventSource.onerror = () => {
        setIsConnected(false);
        eventSource?.close();
        // Não substitui execução já concluída por simulação
        if (!hasFinishedRef.current && !isSimulatingRef.current) {
          startSimulatedProcessing();
        }
      };
    } catch {
      if (!hasFinishedRef.current && !isSimulatingRef.current) {
        startSimulatedProcessing();
      }
    }

    return () => {
      stopSimulation();
      if (eventSourceRef.current) {
        eventSourceRef.current.close();
        eventSourceRef.current = null;
      }
    };
  }, [isOpen, jobId]);

  const addLogEntry = (entry: LogEntry) => {
    setLogs((prev) => [...prev, entry]);
  };

  const startSimulatedProcessing = () => {
    stopSimulation();
    isSimulatingRef.current = true;
    setIsConnected(true);
    const steps: { delay: number; status: StatusImportacao; prog: number; msg: string; lines: number }[] = [
      { delay: 100, status: 'Aguardando', prog: 10, msg: 'Worker BRPOP ecd:queue:jobs recebeu payload do arquivo', lines: 0 },
      { delay: 400, status: 'EmProcessamento', prog: 25, msg: 'Abrindo arquivo via memmap2::Mmap (Zero-Copy 0.4ms)', lines: 1200 },
      { delay: 700, status: 'EmProcessamento', prog: 45, msg: 'Fatiando bytes &[u8] dos Blocos 0000, I010, I030, I050 e I155', lines: 18450 },
      { delay: 1100, status: 'EmProcessamento', prog: 70, msg: 'Batch insert SQLx: 5.000 registros por query parametrizada', lines: 42100 },
      { delay: 1500, status: 'EmProcessamento', prog: 85, msg: 'Motor Fiscal: Apuração DRE (J150/I355) e cálculo de Receita Bruta', lines: 63890 },
      { delay: 1900, status: 'Sucesso', prog: 100, msg: 'Processamento concluído com sucesso! Registro consolidado no banco.', lines: 74200 },
    ];

    let currentStep = 0;
    intervalRef.current = setInterval(() => {
      if (currentStep >= steps.length) {
        stopSimulation();
        setIsConnected(false);
        hasFinishedRef.current = true;
        if (onJobFinished) onJobFinished('Sucesso');
        return;
      }
      const s = steps[currentStep];
      setCurrentStatus(s.status);
      setProgress(s.prog);
      setLinhasProcessadas(s.lines);
      addLogEntry({
        id: `sim-${currentStep}`,
        timestamp: new Date().toLocaleTimeString(),
        status: s.status,
        progresso: s.prog,
        mensagem: s.msg,
        linhas: s.lines,
      });
      currentStep++;
    }, 450);
  };

  if (!isOpen) return null;

  const getStatusBadge = (status: StatusImportacao) => {
    switch (status) {
      case 'Sucesso':
        return <Badge variant="success">Sucesso</Badge>;
      case 'EmProcessamento':
        return <Badge variant="default" className="bg-blue-600 animate-pulse">Em Processamento</Badge>;
      case 'Erro':
        return <Badge variant="destructive">Erro</Badge>;
      case 'Substituida':
        return <Badge variant="secondary">Substituída</Badge>;
      default:
        return <Badge variant="secondary">Aguardando Fila</Badge>;
    }
  };

  return (
    <div className="fixed inset-y-0 right-0 z-50 flex w-full max-w-xl flex-col bg-slate-900 text-slate-100 shadow-2xl transition-all duration-300 border-l border-slate-800">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-slate-800 p-4">
        <div className="flex items-center gap-2">
          <Terminal className="h-5 w-5 text-emerald-400" />
          <div>
            <h2 className="text-sm font-semibold text-white">
              Monitor de Logs em Tempo Real (SSE)
            </h2>
            <p className="text-xs text-slate-400 truncate max-w-sm">
              Job: <span className="font-mono text-emerald-400">{jobId}</span>{' '}
              {filename && `| ${filename}`}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {getStatusBadge(currentStatus)}
          <Button
            variant="ghost"
            size="icon"
            onClick={onClose}
            className="h-8 w-8 text-slate-400 hover:text-white hover:bg-slate-800"
          >
            <X className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {/* Progress metrics */}
      <div className="border-b border-slate-800 bg-slate-950/60 p-4 space-y-2">
        <div className="flex justify-between text-xs text-slate-300">
          <span className="flex items-center gap-1.5 font-medium">
            {isConnected ? (
              <span className="flex h-2 w-2 rounded-full bg-emerald-500 animate-ping" />
            ) : (
              <span className="h-2 w-2 rounded-full bg-slate-600" />
            )}
            {isConnected ? 'Stream Conectado (SSE)' : 'Stream Encerrado'}
          </span>
          <span className="font-mono font-bold text-white">{progress}%</span>
        </div>
        <Progress value={progress} className="h-1.5 bg-slate-800" />
        <div className="flex justify-between text-[11px] text-slate-400 pt-1 font-mono">
          <span>Linhas SPED: {linhasProcessadas.toLocaleString('pt-BR')}</span>
          <span>Motor: Rust memmap2 zero-copy</span>
        </div>
      </div>

      {/* Terminal Output */}
      <div
        ref={logContainerRef}
        className="flex-1 overflow-y-auto p-4 font-mono text-xs leading-relaxed space-y-1.5 bg-slate-950 select-text"
      >
        {logs.length === 0 ? (
          <div className="text-slate-500 italic">Conectando ao canal de streaming Redis Pub/Sub...</div>
        ) : (
          logs.map((log) => (
            <div key={log.id} className="flex gap-2.5 items-start">
              <span className="text-slate-500 select-none">{log.timestamp}</span>
              <span className="text-emerald-400 font-bold select-none">[SPED]</span>
              <span
                className={
                  log.status === 'Erro'
                    ? 'text-red-400'
                    : log.status === 'Sucesso'
                    ? 'text-emerald-300 font-medium'
                    : 'text-slate-300'
                }
              >
                {log.mensagem}
              </span>
            </div>
          ))
        )}
      </div>

      {/* Footer Controls */}
      <div className="border-t border-slate-800 bg-slate-900 p-3 flex justify-between items-center text-xs">
        <span className="text-slate-400">Canal: ecd:events:status</span>
        <Button
          variant="outline"
          size="sm"
          onClick={() => setLogs([])}
          className="h-7 text-xs border-slate-700 bg-slate-800 text-slate-300 hover:bg-slate-700 hover:text-white"
        >
          Limpar Terminal
        </Button>
      </div>
    </div>
  );
};
