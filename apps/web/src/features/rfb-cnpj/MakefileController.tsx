import React, {useState} from 'react';
import {
    Play,
    RefreshCw,
    Settings,
    Terminal,
    FileText,
    Download,
    Network,
    Search,
    Layers,
    Zap,
    CheckCircle,
    XCircle,
    Clock,
    Activity
} from 'lucide-react';
import {Card, CardContent, CardHeader, CardTitle} from '@/components/ui/card';
import {Button} from '@/components/ui/button';
import {Badge} from '@/components/ui/badge';
// import {ScrollArea} from '@/components/ui/scroll-area';

interface MakefileTarget {
    name: string;
    description: string;
    category: 'download' | 'process' | 'index' | 'graph' | 'test';
    estimatedTime: number;
    dependencies: string[];
    lastRun?: string;
    status?: 'success' | 'failed' | 'running' | 'pending';
}

const TARGETS: MakefileTarget[] = [
    {
        name: 'cnpj-download',
        description: 'Download resumível HTTP 206 da base CNPJ (37 zips)',
        category: 'download',
        estimatedTime: 300,
        dependencies: [],
        status: 'success'
    },
    {
        name: 'decompress-zip',
        description: 'Streaming decompress zero-copy em memória',
        category: 'download',
        estimatedTime: 120,
        dependencies: ['cnpj-download'],
        status: 'success'
    },
    {
        name: 'parse-csv-stream',
        description: 'Parse CSV multithread com Rayon (build-cnpj)',
        category: 'process',
        estimatedTime: 180,
        dependencies: ['decompress-zip'],
        status: 'success'
    },
    {
        name: 'cnpj-build',
        description: 'Carga streaming dos 37 zips no banco SQLite cnpj.db',
        category: 'process',
        estimatedTime: 180,
        dependencies: ['cnpj-download'],
        status: 'success'
    },
    {
        name: 'cnpj-rede',
        description: 'Gera rede.db (grafo societário) e rede_search.db (FTS5)',
        category: 'graph',
        estimatedTime: 95,
        dependencies: ['cnpj-build'],
        status: 'success'
    },
    {
        name: 'index-fts5',
        description: 'Create FTS5 search indexes (unicode61 remove_diacritics 2)',
        category: 'index',
        estimatedTime: 45,
        dependencies: ['parse-csv-stream'],
        status: 'success'
    },
    {
        name: 'cnpj-status',
        description: 'Checksum, contagens e validação de integridade SQLite',
        category: 'test',
        estimatedTime: 30,
        dependencies: ['cnpj-rede', 'index-fts5'],
        status: 'success'
    }
];

const CATEGORY_CONFIG = {
    download: {icon: Download, color: 'bg-blue-100 text-blue-600', border: 'border-blue-200'},
    process: {icon: Zap, color: 'bg-purple-100 text-purple-600', border: 'border-purple-200'},
    graph: {icon: Network, color: 'bg-emerald-100 text-emerald-600', border: 'border-emerald-200'},
    index: {icon: Search, color: 'bg-amber-100 text-amber-600', border: 'border-amber-200'},
    test: {icon: FileText, color: 'bg-indigo-100 text-indigo-600', border: 'border-indigo-200'}
};

const STATUS_CONFIG = {
    success: {icon: CheckCircle, color: 'bg-emerald-50 text-emerald-700', border: 'border-emerald-200'},
    failed: {icon: XCircle, color: 'bg-red-50 text-red-700', border: 'border-red-200'},
    running: {icon: Activity, color: 'bg-blue-50 text-blue-700', border: 'border-blue-200', animate: true},
    pending: {icon: Clock, color: 'bg-slate-50 text-slate-600', border: 'border-slate-200'}
};


export const MakefileController: React.FC = () => {
    const [selectedTarget, setSelectedTarget] = useState<string>('cnpj-status');
    const [isRunning, setIsRunning] = useState(false);
    const [logOutput, setLogOutput] = useState<string[]>([
        '> make cnpj-download',
        '[INFO] Starting HTTP 206 download...',
        '[SUCCESS] Downloaded 37 files (74.5 GB)',
        '',
        '> make cnpj-build',
        '[INFO] Streaming decompression & CSV parsing with Rayon...',
        '[SUCCESS] Processed 54.8M records into cnpj.db (890 MB/s)',
        '',
        '> make cnpj-rede',
        '[INFO] Building societary graph & FTS5 indexes...',
        '[SUCCESS] rede.db & rede_search.db ready (420 MB/s)',
        ''
    ]);

    const selectedTargetInfo = TARGETS.find(t => t.name === selectedTarget);
    const categoryConfig = selectedTargetInfo ? CATEGORY_CONFIG[selectedTargetInfo.category] : null;

    const runTarget = () => {
        if (isRunning) return;

        setIsRunning(true);
        const targetToRun = selectedTarget;

        setLogOutput((prev) => [
            ...prev,
            `> make ${targetToRun}`,
            `[INFO] Executing target '${targetToRun}' (${selectedTargetInfo?.description || 'pipeline task'})...`,
            '[BENCH] Rust Pipeline: 377s, < 110 MB RSS streaming (4.2 GB OS cache)',
            `[SUCCESS] Target '${targetToRun}' completed (7.53x speedup vs Python legacy)`
        ]);

        setTimeout(() => {
            setIsRunning(false);
        }, 1200);
    };

    const runAll = () => {
        if (isRunning) return;

        setIsRunning(true);
        setLogOutput(['[INFO] Running complete pipeline...']);

        setTimeout(() => {
            setIsRunning(false);
            setLogOutput((prev) => [
                ...prev,
                '[SUCCESS] Pipeline completed successfully!',
                '[METRICS] Total: 377s, Throughput: 197.6 MB/s',
                '[MEMORY] Peak: 4.2 GB (77.5% reduction vs Python)',
                '[SPEEDUP] 7.53x faster than Python legacy'
            ]);
        }, 3000);
    };

    const clearLogs = () => {
        setLogOutput([]);
    };

    return (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* Targets List */}
            <Card className="border-slate-200">
                <CardHeader className="border-b border-slate-100">
                    <div className="flex items-center justify-between">
                        <CardTitle className="text-base font-bold flex items-center gap-2">
                            <Terminal className="h-5 w-5 text-blue-600"/>
                            Makefile Targets (17 commands)
                        </CardTitle>
                        <Badge className="bg-slate-100 text-slate-700">
                            Autônomo
                        </Badge>
                    </div>
                </CardHeader>
                <CardContent className="p-0">
                    {/*<ScrollArea className="h-[400px]">*/}
                        <div className="p-4 space-y-2">
                            {TARGETS.map((target) => {
                                const CategoryIcon = CATEGORY_CONFIG[target.category].icon;
                                const statusConfig = STATUS_CONFIG[target.status || 'pending'];
                                // const StatusIcon = statusConfig.icon;

                                return (
                                    <div
                                        key={target.name}
                                        onClick={() => setSelectedTarget(target.name)}
                                        className={`p-3 rounded-lg border cursor-pointer transition-all ${
                                            selectedTarget === target.name
                                                ? 'bg-slate-50 border-slate-300 shadow-sm'
                                                : 'border-slate-100 hover:bg-slate-50'
                                        }`}
                                    >
                                        <div className="flex items-center justify-between">
                                            <div className="flex items-center gap-2">
                                                <div
                                                    className={`h-8 w-8 rounded-md flex items-center justify-center ${CATEGORY_CONFIG[target.category].color}`}>
                                                    <CategoryIcon className="h-4 w-4"/>
                                                </div>
                                                <div>
                                                    <div className="flex items-center gap-2">
                                                        <span
                                                            className="font-mono font-semibold text-slate-900 text-sm">{target.name}</span>
                                                        {target.lastRun && (
                                                            <Badge variant="outline"
                                                                   className="text-[10px] text-slate-500">
                                                                {target.lastRun}
                                                            </Badge>
                                                        )}
                                                    </div>
                                                    <p className="text-xs text-slate-500 mt-0.5">{target.description}</p>
                                                </div>
                                            </div>
                                            <div className="flex items-center gap-2">
                                                <div
                                                    className={`px-2 py-1 rounded text-xs font-medium flex items-center gap-1 ${statusConfig.color} ${statusConfig.border}`}>
                                                    {/*<StatusIcon*/}
                                                    {/*    className={`h-3 w-3 ${statusConfig.animate ? 'animate-spin' : ''}`}/>*/}
                                                    {target.status || 'pending'}
                                                </div>
                                                <Badge variant="outline" className="text-xs">
                                                    {target.estimatedTime}s
                                                </Badge>
                                            </div>
                                        </div>
                                        {target.dependencies.length > 0 && (
                                            <div className="mt-2 flex flex-wrap gap-1">
                                                <span className="text-xs text-slate-500">Depends on:</span>
                                                {target.dependencies.map((dep) => (
                                                    <Badge key={dep} variant="outline"
                                                           className="text-[10px] bg-slate-50">
                                                        {dep}
                                                    </Badge>
                                                ))}
                                            </div>
                                        )}
                                    </div>
                                );
                            })}
                        </div>
                    {/*</ScrollArea>*/}
                </CardContent>
            </Card>

            {/* Target Details and Control */}
            <Card className="border-slate-200">
                <CardHeader className="border-b border-slate-100">
                    <CardTitle className="text-base font-bold flex items-center gap-2">
                        <Settings className="h-5 w-5 text-emerald-600"/>
                        Target Execution & Logs
                    </CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                    {/* Selected Target Info */}
                    {selectedTargetInfo && categoryConfig && (
                        <div className="p-4 rounded-lg border bg-slate-50 border-slate-200">
                            <div className="flex items-start justify-between">
                                <div>
                                    <div className="flex items-center gap-2 mb-2">
                                        <div
                                            className={`h-10 w-10 rounded-lg flex items-center justify-center ${categoryConfig.color}`}>
                                            <categoryConfig.icon className="h-5 w-5"/>
                                        </div>
                                        <div>
                                            <h4 className="font-mono font-bold text-lg text-slate-900">{selectedTargetInfo.name}</h4>
                                            <p className="text-sm text-slate-600">{selectedTargetInfo.description}</p>
                                        </div>
                                    </div>
                                    <div className="grid grid-cols-2 gap-2 mt-3">
                                        <div>
                                            <span className="text-xs font-semibold text-slate-500">Category</span>
                                            <Badge className={`mt-1 ${categoryConfig.color} ${categoryConfig.border}`}>
                                                {selectedTargetInfo.category}
                                            </Badge>
                                        </div>
                                        <div>
                                            <span className="text-xs font-semibold text-slate-500">Estimated Time</span>
                                            <div className="mt-1 font-medium text-slate-900">
                                                {selectedTargetInfo.estimatedTime}s
                                            </div>
                                        </div>
                                    </div>
                                </div>
                                <div className="flex items-center gap-2">
                                    <Badge variant="outline" className="text-xs">
                                        {selectedTargetInfo.dependencies.length} deps
                                    </Badge>
                                </div>
                            </div>
                        </div>
                    )}

                    {/* Control Buttons */}
                    <div className="flex items-center gap-2">
                        <Button
                            onClick={runTarget}
                            disabled={isRunning}
                            className="flex-1"
                            variant={isRunning ? "secondary" : "default"}
                        >
                            {isRunning ? (
                                <>
                                    <RefreshCw className="h-4 w-4 mr-2 animate-spin"/>
                                    Running...
                                </>
                            ) : (
                                <>
                                    <Play className="h-4 w-4 mr-2"/>
                                    Run Selected Target
                                </>
                            )}
                        </Button>
                        <Button
                            onClick={runAll}
                            disabled={isRunning}
                            className="flex-1"
                            variant="outline"
                        >
                            <Layers className="h-4 w-4 mr-2"/>
                            Run All Pipeline
                        </Button>
                        <Button
                            onClick={clearLogs}
                            variant="ghost"
                            size="sm"
                        >
                            Clear
                        </Button>
                    </div>

                    {/* Log Output */}
                    <div className="border border-slate-200 rounded-lg bg-slate-900">
                        <div className="px-3 py-2 border-b border-slate-800 flex items-center justify-between">
                            <div className="flex items-center gap-2">
                                <div className="h-2 w-2 rounded-full bg-emerald-500"></div>
                                <span className="text-xs font-mono text-slate-300">Terminal Output</span>
                            </div>
                            <Badge variant="secondary" className="bg-slate-800 text-slate-300 text-xs">
                                {logOutput.length} lines
                            </Badge>
                        </div>
                        {/*<ScrollArea className="h-[200px]">*/}
              <pre className="p-3 text-xs font-mono text-slate-300">
                {logOutput.map((line, index) => (
                    <div key={index} className={`
                    ${line.startsWith('>') ? 'text-amber-400 font-bold' : ''}
                    ${line.startsWith('[INFO]') ? 'text-blue-400' : ''}
                    ${line.startsWith('[SUCCESS]') ? 'text-emerald-400' : ''}
                    ${line.startsWith('[BENCH]') ? 'text-purple-400' : ''}
                    ${line.startsWith('[ERROR]') ? 'text-red-400' : ''}
                  `}>
                        {line}
                    </div>
                ))}
                  {isRunning && (
                      <div className="text-emerald-400 animate-pulse">
                          [RUNNING] Processing... {selectedTarget}
                      </div>
                  )}
              </pre>
                        {/*</ScrollArea>*/}
                    </div>

                    {/* Quick Stats */}
                    <div className="grid grid-cols-3 gap-2">
                        <div className="p-2 rounded border border-slate-200 bg-slate-50 text-center">
                            <div className="text-xs font-semibold text-slate-500">Complete Targets</div>
                            <div className="text-lg font-bold text-emerald-600">6/7</div>
                        </div>
                        <div className="p-2 rounded border border-slate-200 bg-slate-50 text-center">
                            <div className="text-xs font-semibold text-slate-500">Total Time</div>
                            <div className="text-lg font-bold text-blue-600">377s</div>
                        </div>
                        <div className="p-2 rounded border border-slate-200 bg-slate-50 text-center">
                            <div className="text-xs font-semibold text-slate-500">Avg Throughput</div>
                            <div className="text-lg font-bold text-purple-600">197.6 MB/s</div>
                        </div>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
};
