import React from 'react';
import {
  ResponsiveContainer,
  BarChart,
  Bar,
  LineChart,
  Line,
  PieChart,
  Pie,
  Cell,
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
} from 'recharts';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import type { ChartSpec } from '@/types';

interface DynamicChartRendererProps {
  spec: ChartSpec;
}

const DEFAULT_COLORS = [
  '#2563eb', // Blue
  '#10b981', // Emerald
  '#f59e0b', // Amber
  '#8b5cf6', // Violet
  '#ec4899', // Pink
  '#06b6d4', // Cyan
  '#ef4444', // Red
];

export const DynamicChartRenderer: React.FC<DynamicChartRendererProps> = ({ spec }) => {
  if (!spec || !spec.data || !Array.isArray(spec.data) || spec.data.length === 0) {
    return (
      <Card className="border-indigo-100 bg-white/95 shadow-sm my-3">
        <CardHeader className="p-4 pb-2">
          <CardTitle className="text-sm font-semibold text-slate-900">{spec?.title || 'Gráfico'}</CardTitle>
          <CardDescription className="text-xs text-slate-500">
            Nenhum dado numérico disponível para plotagem.
          </CardDescription>
        </CardHeader>
      </Card>
    );
  }

  const colors = spec.colors && spec.colors.length > 0 ? spec.colors : DEFAULT_COLORS;
  const yKeys = Array.isArray(spec.y_keys) ? spec.y_keys : [];

  const renderChart = () => {
    switch (spec.chart_type) {
      case 'bar':
        return (
          <BarChart data={spec.data} margin={{ top: 10, right: 20, left: 10, bottom: 20 }}>
            <CartesianGrid strokeDasharray="3 3" vertical={false} />
            <XAxis dataKey={spec.x_key} tick={{ fontSize: 11 }} />
            <YAxis tick={{ fontSize: 11 }} />
            <Tooltip />
            <Legend wrapperStyle={{ fontSize: 12, paddingTop: 8 }} />
            {yKeys.map((key, idx) => (
              <Bar
                key={key}
                dataKey={key}
                fill={colors[idx % colors.length]}
                radius={[4, 4, 0, 0]}
              />
            ))}
          </BarChart>
        );

      case 'line':
        return (
          <LineChart data={spec.data} margin={{ top: 10, right: 20, left: 10, bottom: 20 }}>
            <CartesianGrid strokeDasharray="3 3" vertical={false} />
            <XAxis dataKey={spec.x_key} tick={{ fontSize: 11 }} />
            <YAxis tick={{ fontSize: 11 }} />
            <Tooltip />
            <Legend wrapperStyle={{ fontSize: 12, paddingTop: 8 }} />
            {yKeys.map((key, idx) => (
              <Line
                key={key}
                type="monotone"
                dataKey={key}
                stroke={colors[idx % colors.length]}
                strokeWidth={2.5}
                dot={{ r: 4 }}
                activeDot={{ r: 6 }}
              />
            ))}
          </LineChart>
        );

      case 'area':
        return (
          <AreaChart data={spec.data} margin={{ top: 10, right: 20, left: 10, bottom: 20 }}>
            <CartesianGrid strokeDasharray="3 3" vertical={false} />
            <XAxis dataKey={spec.x_key} tick={{ fontSize: 11 }} />
            <YAxis tick={{ fontSize: 11 }} />
            <Tooltip />
            <Legend wrapperStyle={{ fontSize: 12, paddingTop: 8 }} />
            {yKeys.map((key, idx) => (
              <Area
                key={key}
                type="monotone"
                dataKey={key}
                stroke={colors[idx % colors.length]}
                fill={colors[idx % colors.length]}
                fillOpacity={0.25}
              />
            ))}
          </AreaChart>
        );

      case 'pie': {
        const valKey = yKeys[0] || 'valor';
        return (
          <PieChart>
            <Pie
              data={spec.data}
              dataKey={valKey}
              nameKey={spec.x_key}
              cx="50%"
              cy="50%"
              innerRadius={40}
              outerRadius={75}
              paddingAngle={4}
            >
              {spec.data.map((_, index) => (
                <Cell key={`cell-${index}`} fill={colors[index % colors.length]} />
              ))}
            </Pie>
            <Tooltip />
            <Legend wrapperStyle={{ fontSize: 11 }} />
          </PieChart>
        );
      }

      default:
        return (
          <div className="flex h-40 items-center justify-center text-xs text-slate-400">
            Tipo de gráfico não suportado ({spec.chart_type})
          </div>
        );
    }
  };

  return (
    <Card className="border-indigo-100 bg-white/95 shadow-sm my-3">
      <CardHeader className="p-4 pb-2">
        <CardTitle className="text-sm font-semibold text-slate-900">{spec.title}</CardTitle>
        {spec.description && (
          <CardDescription className="text-xs text-slate-500">
            {spec.description}
          </CardDescription>
        )}
      </CardHeader>
      <CardContent className="p-4 pt-1">
        <div className="h-64 w-full">
          <ResponsiveContainer width="100%" height="100%">
            {renderChart()}
          </ResponsiveContainer>
        </div>
      </CardContent>
    </Card>
  );
};
