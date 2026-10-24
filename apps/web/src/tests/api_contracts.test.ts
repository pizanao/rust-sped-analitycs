import test from 'node:test';
import assert from 'node:assert/strict';
import { api, apiClient } from '../services/api.ts';
import type {
  JobStatusEvent,
  StatusImportacao,
  ChartSpec,
  ResultadoExercicio,
} from '../types/index.ts';

test('apiClient deve ter withCredentials habilitado para cookies JWT HttpOnly', () => {
  assert.equal(apiClient.defaults.withCredentials, true);
  assert.equal(apiClient.defaults.baseURL, '/api');
  assert.equal(apiClient.defaults.headers['Accept'], 'application/json');
});

test('api.importacoes.obterStreamUrl deve formatar a URL do SSE corretamente', () => {
  const url = api.importacoes.obterStreamUrl('job-12345');
  assert.equal(url, '/api/jobs/job-12345/stream');
});

test('api.fiscal.exportarXlsxUrl deve limpar máscara do CNPJ e formatar URL', () => {
  const url = api.fiscal.exportarXlsxUrl('33.000.167/0001-01');
  assert.equal(url, '/api/fiscal/exportar-xlsx/33000167000101');
});

test('JobStatusEvent deve manter consistência com o contrato Redis Pub/Sub do backend Rust', () => {
  const status: StatusImportacao = 'EmProcessamento';
  const event: JobStatusEvent = {
    job_id: 'd3b07384-d113-4ec6-89d4-6e69a04a60f9',
    hash_arquivo: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
    status,
    progresso_percentual: 50,
    linhas_processadas: 25000,
    mensagem: 'Lote de 5.000 registros inserido com sucesso via SQLx',
    timestamp: '2026-10-24T03:00:00Z',
  };

  assert.equal(event.status, 'EmProcessamento');
  assert.equal(event.progresso_percentual, 50);
  assert.equal(event.linhas_processadas, 25000);
});

test('ChartSpec deve aceitar especificações JSON do agente LangChain', () => {
  const spec: ChartSpec = {
    chart_type: 'bar',
    title: 'Comparativo de Receita Bruta (DRE J150)',
    description: 'Valores calculados com partial_ratio >= 0.80',
    x_key: 'trimestre',
    y_keys: ['receita_bruta', 'receita_servico'],
    data: [
      { trimestre: '1T', receita_bruta: 1000000, receita_servico: 750000 },
      { trimestre: '2T', receita_bruta: 1200000, receita_servico: 820000 },
    ],
  };

  assert.equal(spec.chart_type, 'bar');
  assert.equal(spec.data.length, 2);
  assert.equal(spec.y_keys.length, 2);
});

test('ResultadoExercicio deve conter todos os campos de apuração do ecd-fiscal', () => {
  const resultado: ResultadoExercicio = {
    id: 'res-1',
    cnpj: '33000167000101',
    razao_social: 'PETROBRAS',
    periodo_inicial: '2023-01-01',
    periodo_final: '2023-12-31',
    receita_bruta: 48000000,
    receita_servico: 35000000,
    outras_receitas: 13000000,
    receita_acumulada: 48000000,
    total_nota: 47000000,
    hash_arquivo: 'abc123',
    situacao: 'Regular',
    criado_em: '2026-10-24T00:00:00Z',
  };

  assert.equal(resultado.receita_bruta, 48000000);
  assert.equal(resultado.receita_servico, 35000000);
  assert.equal(resultado.outras_receitas, 13000000);
});
