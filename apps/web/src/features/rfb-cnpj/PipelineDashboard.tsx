import React, { useState, useMemo } from 'react';
import {
  Download,
  Database,
  Zap,
  GitBranch,
  BarChart,
  Network,
  Cpu,
  TrendingUp,
  FileText,
  Shield,
  RefreshCw,
  CheckCircle2,
  AlertCircle,
  Clock,
  Activity,
  Layers,
  HardDrive,
  Timer,
  ArrowRight
} from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
// import { Button } from '@/components/ui/button';
import { Progress } from '@/components/ui/progress';
import { Badge } from '@/components/ui/badge';
import { AreaChart, Area, BarChart as RechartsBarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';

interface PipelineStage {
  id: string;
  name: string;
  status: 'pending' | 'running' | 'completed' | 'error';
  progress: number;
  duration?: number;
  itemsProcessed?: number;
  throughput?: number;
  memoryUsage?: number;
}

interface PipelineMetrics {
  totalFiles: number;
  totalSize: number;
  downloadedSize: number;
  elapsedTime: number;
  estimatedTimeRemaining: number;
  averageSpeed: number;
  peakSpeed: number;
  memoryPeak: number;
  cpuUsage: number;
}

interface ComparisonMetrics {
  rust: {
    duration: number;
    throughput: number;
    memoryMb: number;
  };
  python: {
    duration: number;
    throughput: number;
    memoryMb: number;
  };
  speedup: number;
  memoryReduction: number;
}

// const COLORS = {
//   primary: '#3b82f6',
//   success: '#10b981',
//   warning: '#f59e0b',
//   error: '#ef4444',
//   accent: '#8b5cf6',
//   gradient: {
//     start: '#3b82f6',
//     end: '#8b5cf6'
//   }
// };

// const CHART_COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#06b6d4'];

const MOCK_SPEED_DATA = Array.from({ length: 20 }, (_, i) => ({
  time: `${i}s`,
  speed: Math.floor(Math.random() * 50 + 80),
  python: Math.floor(Math.random() * 20 + 30)
}));

const MOCK_THROUGHPUT_DATA = [
  { stage: 'Download', rust: 245, python: 45 },
  { stage: 'Decompress', rust: 512, python: 67 },
  { stage: 'Parse', rust: 890, python: 120 },
  { stage: 'Graph', rust: 340, python: 45 },
  { stage: 'Index', rust: 420, python: 89 }
];

const MOCK_SIZE_DATA = [
  { name: 'CNPJ.db', size: 72, compressed: 12 },
  { name: 'rede.db', size: 1.2, compressed: 0.3 },
  { name: 'links.db', size: 0.8, compressed: 0.2 },
  { name: 'Total', size: 74, compressed: 12.5 }
];

export const PipelineDashboard: React.FC = () => {
  const [stages] = useState<PipelineStage[]>([
    { id: 'download', name: 'Download Resumível HTTP 206', status: 'completed', progress: 100, duration: 45, itemsProcessed: 47, throughput: 245 },
    { id: 'decompress', name: 'Streaming Decompress Zero-Copy', status: 'completed', progress: 100, duration: 12, itemsProcessed: 47, throughput: 512 },
    { id: 'parse', name: 'Parse CSV Multithread', status: 'completed', progress: 100, duration: 180, itemsProcessed: 54800000, throughput: 890 },
    { id: 'graph', name: 'Build Societary Graph', status: 'completed', progress: 100, duration: 95, itemsProcessed: 125000, throughput: 340 },
    { id: 'fts', name: 'FTS5 Indexing', status: 'completed', progress: 100, duration: 45, itemsProcessed: 54800000, throughput: 420 }
  ]);

  const metrics: PipelineMetrics = useMemo(() => ({
    totalFiles: 47,
    totalSize: 74.5,
    downloadedSize: 74.5,
    elapsedTime: 377,
    estimatedTimeRemaining: 0,
    averageSpeed: 197.6,
    peakSpeed: 512,
    memoryPeak: 4.2,
    cpuUsage: 85
  }), []);

  const comparison: ComparisonMetrics = useMemo(() => ({
    rust: {
      duration: 377,
      throughput: 197.6,
      memoryMb: 4.2
    },
    python: {
      duration: 2840,
      throughput: 26.2,
      memoryMb: 18.7
    },
    speedup: 7.53,
    memoryReduction: 77.5
  }), []);

  const formatBytes = (gb: number) => {
    if (gb >= 1) return `${gb.toFixed(1)} GB`;
    return `${(gb * 1024).toFixed(0)} MB`;
  };

  const formatDuration = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    if (mins > 0) return `${mins}m ${secs}s`;
    return `${secs}s`;
  };

  const formatNumber = (num: number) => {
    if (num >= 1000000) return `${(num / 1000000).toFixed(1)}M`;
    if (num >= 1000) return `${(num / 1000).toFixed(0)}K`;
    return num.toString();
  };

  const getStatusIcon = (status: PipelineStage['status']) => {
    switch (status) {
      case 'completed': return <CheckCircle2 className="h-4 w-4 text-emerald-600" />;
      case 'running': return <RefreshCw className="h-4 w-4 text-blue-600 animate-spin" />;
      case 'error': return <AlertCircle className="h-4 w-4 text-red-600" />;
      default: return <Clock className="h-4 w-4 text-slate-400" />;
    }
  };

  const getStatusColor = (status: PipelineStage['status']) => {
    switch (status) {
      case 'completed': return 'bg-emerald-50 border-emerald-200';
      case 'running': return 'bg-blue-50 border-blue-200';
      case 'error': return 'bg-red-50 border-red-200';
      default: return 'bg-slate-50 border-slate-200';
    }
  };

  return (
    <div className="space-y-6">
      {/* Hero Stats */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card className="border-slate-200 bg-gradient-to-br from-blue-50 to-white">
          <CardContent className="p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs font-semibold text-slate-500 uppercase tracking-wide">Speedup vs Python</p>
                <div className="flex items-baseline gap-2 mt-1">
                  <span className="text-3xl font-extrabold text-blue-600">{comparison.speedup}x</span>
                  <span className="text-sm font-medium text-emerald-600">faster</span>
                </div>
              </div>
              <div className="h-12 w-12 rounded-xl bg-blue-100 text-blue-600 flex items-center justify-center">
                <Zap className="h-6 w-6" />
              </div>
            </div>
            <div className="mt-3 flex items-center gap-2 text-xs">
              <Badge variant="outline" className="border-emerald-200 text-emerald-700">
                {formatDuration(comparison.rust.duration)} vs {formatDuration(comparison.python.duration)}
              </Badge>
            </div>
          </CardContent>
        </Card>

        <Card className="border-slate-200 bg-gradient-to-br from-emerald-50 to-white">
          <CardContent className="p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs font-semibold text-slate-500 uppercase tracking-wide">Memory Reduction</p>
                <div className="flex items-baseline gap-2 mt-1">
                  <span className="text-3xl font-extrabold text-emerald-600">{comparison.memoryReduction}%</span>
                  <span className="text-sm font-medium text-slate-600">less RAM</span>
                </div>
              </div>
              <div className="h-12 w-12 rounded-xl bg-emerald-100 text-emerald-600 flex items-center justify-center">
                <HardDrive className="h-6 w-6" />
              </div>
            </div>
            <div className="mt-3 flex items-center gap-2 text-xs">
              <Badge variant="outline" className="border-slate-200 text-slate-600">
                {comparison.rust.memoryMb} MB vs {comparison.python.memoryMb} MB
              </Badge>
            </div>
          </CardContent>
        </Card>

        <Card className="border-slate-200 bg-gradient-to-br from-purple-50 to-white">
          <CardContent className="p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs font-semibold text-slate-500 uppercase tracking-wide">Throughput</p>
                <div className="flex items-baseline gap-2 mt-1">
                  <span className="text-3xl font-extrabold text-purple-600">{metrics.averageSpeed}</span>
                  <span className="text-sm font-medium text-slate-600">MB/s avg</span>
                </div>
              </div>
              <div className="h-12 w-12 rounded-xl bg-purple-100 text-purple-600 flex items-center justify-center">
                <TrendingUp className="h-6 w-6" />
              </div>
            </div>
            <div className="mt-3 flex items-center gap-2 text-xs">
              <Badge variant="outline" className="border-slate-200 text-slate-600">
                Peak: {metrics.peakSpeed} MB/s
              </Badge>
            </div>
          </CardContent>
        </Card>

        <Card className="border-slate-200 bg-gradient-to-br from-amber-50 to-white">
          <CardContent className="p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs font-semibold text-slate-500 uppercase tracking-wide">Total Processed</p>
                <div className="flex items-baseline gap-2 mt-1">
                  <span className="text-3xl font-extrabold text-amber-600">{formatBytes(metrics.totalSize)}</span>
                </div>
              </div>
              <div className="h-12 w-12 rounded-xl bg-amber-100 text-amber-600 flex items-center justify-center">
                <Database className="h-6 w-6" />
              </div>
            </div>
            <div className="mt-3 flex items-center gap-2 text-xs">
              <Badge variant="outline" className="border-slate-200 text-slate-600">
                {metrics.totalFiles} files
              </Badge>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Pipeline Stages Timeline */}
      <Card className="border-slate-200">
        <CardHeader className="border-b border-slate-100">
          <div className="flex items-center justify-between">
            <CardTitle className="text-lg font-bold flex items-center gap-2">
              <Layers className="h-5 w-5 text-blue-600" />
              Pipeline Stages - Zero-Copy Streaming
            </CardTitle>
            <Badge className="bg-emerald-100 text-emerald-700 border-emerald-200">
              All Completed
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="p-6">
          <div className="space-y-3">
            {stages.map((stage, index) => (
              <div
                key={stage.id}
                className={`flex items-center gap-4 p-4 rounded-lg border ${getStatusColor(stage.status)}`}
              >
                <div className="flex items-center gap-3 flex-1">
                  {getStatusIcon(stage.status)}
                  <div className="flex-1">
                    <div className="flex items-center gap-2">
                      <span className="font-semibold text-slate-800">{stage.name}</span>
                      {stage.status === 'running' && (
                        <Badge variant="outline" className="border-blue-200 text-blue-600 text-xs">
                          Running
                        </Badge>
                      )}
                    </div>
                    <div className="flex items-center gap-4 mt-1 text-xs text-slate-500">
                      <span className="flex items-center gap-1">
                        <Timer className="h-3 w-3" />
                        {formatDuration(stage.duration || 0)}
                      </span>
                      <span className="flex items-center gap-1">
                        <Activity className="h-3 w-3" />
                        {formatNumber(stage.itemsProcessed || 0)} items
                      </span>
                      <span className="flex items-center gap-1">
                        <TrendingUp className="h-3 w-3" />
                        {stage.throughput} MB/s
                      </span>
                    </div>
                  </div>
                </div>
                <div className="w-24">
                  <Progress value={stage.progress} className="h-2" />
                  <p className="text-xs text-slate-500 text-center mt-1">{stage.progress}%</p>
                </div>
                {index < stages.length - 1 && (
                  <ArrowRight className="h-4 w-4 text-slate-300 absolute right-[-24px]" />
                )}
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Performance Charts */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Speed Comparison Over Time */}
        <Card className="border-slate-200">
          <CardHeader>
            <CardTitle className="text-base font-bold flex items-center gap-2">
              <Activity className="h-5 w-5 text-blue-600" />
              Processing Speed (Rust vs Python Legacy)
            </CardTitle>
          </CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={250}>
              <AreaChart data={MOCK_SPEED_DATA}>
                <defs>
                  <linearGradient id="rustGradient" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#3b82f6" stopOpacity={0.3}/>
                    <stop offset="95%" stopColor="#3b82f6" stopOpacity={0}/>
                  </linearGradient>
                  <linearGradient id="pythonGradient" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#f59e0b" stopOpacity={0.3}/>
                    <stop offset="95%" stopColor="#f59e0b" stopOpacity={0}/>
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
                <XAxis dataKey="time" stroke="#64748b" fontSize={11} />
                <YAxis stroke="#64748b" fontSize={11} />
                <Tooltip
                  contentStyle={{
                    backgroundColor: '#fff',
                    border: '1px solid #e2e8f0',
                    borderRadius: '8px',
                    fontSize: '12px'
                  }}
                />
                <Area
                  type="monotone"
                  dataKey="speed"
                  stroke="#3b82f6"
                  strokeWidth={2}
                  fill="url(#rustGradient)"
                  name="Rust (MB/s)"
                />
                <Area
                  type="monotone"
                  dataKey="python"
                  stroke="#f59e0b"
                  strokeWidth={2}
                  fill="url(#pythonGradient)"
                  name="Python (MB/s)"
                />
              </AreaChart>
            </ResponsiveContainer>
            <div className="flex items-center justify-center gap-6 mt-4">
              <div className="flex items-center gap-2">
                <div className="h-3 w-3 rounded-full bg-blue-500" />
                <span className="text-xs font-medium text-slate-600">Rust Pipeline</span>
              </div>
              <div className="flex items-center gap-2">
                <div className="h-3 w-3 rounded-full bg-amber-500" />
                <span className="text-xs font-medium text-slate-600">Python Legacy</span>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Stage Throughput Comparison */}
        <Card className="border-slate-200">
          <CardHeader>
            <CardTitle className="text-base font-bold flex items-center gap-2">
              <BarChart className="h-5 w-5 text-emerald-600" />
              Throughput by Stage (MB/s)
            </CardTitle>
          </CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={250}>
              <RechartsBarChart data={MOCK_THROUGHPUT_DATA}>
                <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
                <XAxis dataKey="stage" stroke="#64748b" fontSize={11} />
                <YAxis stroke="#64748b" fontSize={11} />
                <Tooltip
                  contentStyle={{
                    backgroundColor: '#fff',
                    border: '1px solid #e2e8f0',
                    borderRadius: '8px',
                    fontSize: '12px'
                  }}
                />
                <Bar dataKey="rust" fill="#3b82f6" name="Rust" radius={[4, 4, 0, 0]} />
                <Bar dataKey="python" fill="#f59e0b" name="Python" radius={[4, 4, 0, 0]} />
              </RechartsBarChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      </div>

      {/* File Sizes and Database Info */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Database Files */}
        <Card className="border-slate-200">
          <CardHeader>
            <CardTitle className="text-base font-bold flex items-center gap-2">
              <Database className="h-5 w-5 text-purple-600" />
              Database Files (SQLite)
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {MOCK_SIZE_DATA.map((file) => (
                <div key={file.name} className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <FileText className="h-4 w-4 text-slate-400" />
                    <span className="text-sm font-medium text-slate-700">{file.name}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge variant="outline" className="text-xs">
                      {file.size} GB
                    </Badge>
                    <Badge className="bg-emerald-100 text-emerald-700 text-xs">
                      {file.compressed} GB
                    </Badge>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        {/* Technology Stack */}
        <Card className="border-slate-200">
          <CardHeader>
            <CardTitle className="text-base font-bold flex items-center gap-2">
              <Cpu className="h-5 w-5 text-blue-600" />
              Technology Stack
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              {[
                { name: 'memmap2', desc: 'Zero-copy memory mapping' },
                { name: 'flate2', desc: 'Streaming decompression' },
                { name: 'rusqlite', desc: 'Native SQLite FTS5' },
                { name: 'rayon', desc: 'Data parallelism' },
                { name: 'tokio', desc: 'Async runtime' }
              ].map((tech) => (
                <div key={tech.name} className="flex items-center justify-between p-2 rounded bg-slate-50">
                  <span className="text-sm font-mono font-semibold text-blue-600">{tech.name}</span>
                  <span className="text-xs text-slate-500">{tech.desc}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        {/* Key Features */}
        <Card className="border-slate-200">
          <CardHeader>
            <CardTitle className="text-base font-bold flex items-center gap-2">
              <Shield className="h-5 w-5 text-emerald-600" />
              Key Features
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              {[
                { icon: Download, text: 'HTTP 206 Resumable Downloads' },
                { icon: Zap, text: 'Zero-Copy Streaming Decompress' },
                { icon: GitBranch, text: 'Multi-threaded CSV Parsing' },
                { icon: Network, text: 'Societary Graph BFS Engine' },
                { icon: Database, text: 'FTS5 Full-Text Search' }
              ].map((feature, index) => (
                <div key={index} className="flex items-center gap-2 p-2 rounded bg-slate-50">
                  <feature.icon className="h-4 w-4 text-emerald-600" />
                  <span className="text-xs font-medium text-slate-700">{feature.text}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Performance Summary Table */}
      <Card className="border-slate-200">
        <CardHeader>
          <CardTitle className="text-base font-bold flex items-center gap-2">
            <TrendingUp className="h-5 w-5 text-purple-600" />
            Performance Summary - Rust vs Python Legacy
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-200">
                  <th className="text-left py-3 px-4 font-semibold text-slate-600">Metric</th>
                  <th className="text-right py-3 px-4 font-semibold text-blue-600">Rust Pipeline</th>
                  <th className="text-right py-3 px-4 font-semibold text-amber-600">Python Legacy</th>
                  <th className="text-right py-3 px-4 font-semibold text-emerald-600">Improvement</th>
                </tr>
              </thead>
              <tbody>
                <tr className="border-b border-slate-100 hover:bg-slate-50">
                  <td className="py-3 px-4 text-slate-700">Total Duration</td>
                  <td className="py-3 px-4 text-right font-mono font-semibold text-blue-600">{formatDuration(comparison.rust.duration)}</td>
                  <td className="py-3 px-4 text-right font-mono text-amber-600">{formatDuration(comparison.python.duration)}</td>
                  <td className="py-3 px-4 text-right">
                    <Badge className="bg-emerald-100 text-emerald-700 border-emerald-200">
                      {comparison.speedup}x faster
                    </Badge>
                  </td>
                </tr>
                <tr className="border-b border-slate-100 hover:bg-slate-50">
                  <td className="py-3 px-4 text-slate-700">Average Throughput</td>
                  <td className="py-3 px-4 text-right font-mono font-semibold text-blue-600">{comparison.rust.throughput} MB/s</td>
                  <td className="py-3 px-4 text-right font-mono text-amber-600">{comparison.python.throughput} MB/s</td>
                  <td className="py-3 px-4 text-right">
                    <Badge className="bg-emerald-100 text-emerald-700 border-emerald-200">
                      {(comparison.rust.throughput / comparison.python.throughput).toFixed(1)}x
                    </Badge>
                  </td>
                </tr>
                <tr className="hover:bg-slate-50">
                  <td className="py-3 px-4 text-slate-700">Peak Memory Usage</td>
                  <td className="py-3 px-4 text-right font-mono font-semibold text-blue-600">{comparison.rust.memoryMb} MB</td>
                  <td className="py-3 px-4 text-right font-mono text-amber-600">{comparison.python.memoryMb} MB</td>
                  <td className="py-3 px-4 text-right">
                    <Badge className="bg-emerald-100 text-emerald-700 border-emerald-200">
                      {comparison.memoryReduction}% reduction
                    </Badge>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>
    </div>
  );
};
