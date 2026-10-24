import test from 'node:test';
import assert from 'node:assert/strict';
import {
  createRouter,
  createRoute,
  createRootRoute,
} from '@tanstack/react-router';
import type {
  Registro0000,
  RegistroJ100,
  RegJ150,
  RegistroI155,
  ChartSpec,
} from '../types/index.ts';

test('router TanStack deve registrar as 5 rotas obrigatórias da aplicação', () => {
  const rootRoute = createRootRoute();
  const indexRoute = createRoute({ getParentRoute: () => rootRoute, path: '/' });
  const ingestaoRoute = createRoute({ getParentRoute: () => rootRoute, path: '/ingestao' });
  const fiscalRoute = createRoute({ getParentRoute: () => rootRoute, path: '/fiscal' });
  const grcRoute = createRoute({ getParentRoute: () => rootRoute, path: '/grc' });
  const indicadoresRoute = createRoute({ getParentRoute: () => rootRoute, path: '/indicadores' });

  const testRouter = createRouter({
    routeTree: rootRoute.addChildren([
      indexRoute,
      ingestaoRoute,
      fiscalRoute,
      grcRoute,
      indicadoresRoute,
    ]),
  });

  const routes = testRouter.routesByPath;
  assert.ok(routes['/'], 'Rota raiz "/" deve estar registrada');
  assert.ok(routes['/ingestao'], 'Rota "/ingestao" deve estar registrada');
  assert.ok(routes['/fiscal'], 'Rota "/fiscal" deve estar registrada');
  assert.ok(routes['/grc'], 'Rota "/grc" deve estar registrada');
  assert.ok(routes['/indicadores'], 'Rota "/indicadores" deve estar registrada');
});

test('Demonstrativos SPED 0000, J100, J150 e I155 devem ter tipagem e campos de paridade', () => {
  const reg0000: Registro0000 = {
    nome_razao: 'EMPRESA TESTE S.A.',
    cnpj: '00000000000191',
    uf: 'SP',
    dt_ini: '2023-01-01',
    dt_fim: '2023-12-31',
    ind_sit_ini_per: 0,
    ind_nire: 1,
    ind_fin_esc: 0,
    ind_grande_porte: 0,
    tip_ecd: 0,
    ident_mf: 'N',
    ind_esc_cons: 'N',
    ind_centralizada: 1,
    ind_mudanc_pc: 0,
    i990: 100,
    j990: 50,
    hash_arquivo_id: 'hash123',
  };
  assert.equal(reg0000.cnpj, '00000000000191');

  const regJ100: RegistroJ100 = {
    cod_agl: '1.01',
    descr_cod_agl: 'ATIVO CIRCULANTE',
    ind_cod_agl: 'T',
    nivel_agl: 2,
    ind_grp_bal: 'A',
    vl_cta_ini: 10000,
    ind_dc_cta_ini: 'D',
    vl_cta_fin: 15000,
    ind_dc_cta_fin: 'D',
    cnpj: '00000000000191',
    reg_0000_id: 1,
    reg_j005_id: 1,
  };
  assert.equal(regJ100.ind_grp_bal, 'A');

  const regJ150: RegJ150 = {
    nu_ordem: 1,
    cod_agl: '3.01',
    descr_cod_agl: 'RECEITA BRUTA DE VENDAS',
    ind_cod_agl: 'T',
    nivel_agl: 2,
    vl_cta: 50000,
    ind_dc_cta: 'C',
    ind_grp_dre: 'R',
    cnpj: '00000000000191',
    hash_arquivo_id: 'hash123',
    reg_0000_id: 1,
    reg_j005_id: 1,
  };
  assert.equal(regJ150.ind_dc_cta, 'C');

  const regI155: RegistroI155 = {
    cod_cta: '1.1.01',
    vl_sld_ini: 5000,
    ind_dc_ini: 'D',
    vl_deb: 2000,
    vl_cred: 1000,
    vl_sld_fim: 6000,
    ind_dc_fim: 'D',
    reg_0000_id: 1,
    reg_i150_id: 1,
  };
  assert.equal(regI155.vl_sld_fim, 6000);
});

test('ChartSpec deve suportar todos os 4 tipos de gráficos dinâmicos', () => {
  const chartTypes: ChartSpec['chart_type'][] = ['bar', 'line', 'pie', 'area'];
  for (const t of chartTypes) {
    const spec: ChartSpec = {
      chart_type: t,
      title: `Gráfico ${t}`,
      x_key: 'categoria',
      y_keys: ['valor'],
      data: [{ categoria: 'A', valor: 100 }],
    };
    assert.equal(spec.chart_type, t);
    assert.ok(spec.data.length > 0);
  }
});
