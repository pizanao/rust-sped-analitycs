import test from 'node:test';
import assert from 'node:assert/strict';
import { formatarCnpj, formatarCpf, formatarCpfCnpj, validarCnpj, apenasDigitos } from '../lib/cnpj.ts';
import { cn } from '../lib/utils.ts';

test('apenasDigitos deve extrair apenas números', () => {
  assert.equal(apenasDigitos('33.000.167/0001-01'), '33000167000101');
  assert.equal(apenasDigitos('abc-123.456'), '123456');
  assert.equal(apenasDigitos(''), '');
});

test('formatarCnpj deve aplicar máscara brasileira padrão', () => {
  assert.equal(formatarCnpj('33000167000101'), '33.000.167/0001-01');
  assert.equal(formatarCnpj('00000000000191'), '00.000.000/0001-91');
  assert.equal(formatarCnpj('12345'), '12.345');
});

test('formatarCpf deve formatar CPF de pessoa física corretamente', () => {
  assert.equal(formatarCpf('12345678901'), '123.456.789-01');
  assert.equal(formatarCpf('123456'), '123.456');
  assert.equal(formatarCpf('12'), '12');
  assert.equal(formatarCpf(''), '');
});

test('formatarCpfCnpj deve chavear dinamicamente entre CPF e CNPJ pelo comprimento', () => {
  // CPF (até 11 dígitos)
  assert.equal(formatarCpfCnpj('12345678901'), '123.456.789-01');
  // CNPJ (mais de 11 dígitos)
  assert.equal(formatarCpfCnpj('33000167000101'), '33.000.167/0001-01');
});

test('validarCnpj deve validar CNPJs reais e rejeitar inválidos e repetidos', () => {
  // CNPJs válidos
  assert.equal(validarCnpj('33.000.167/0001-01'), true); // Petrobras
  assert.equal(validarCnpj('33592510000154'), true); // Vale
  assert.equal(validarCnpj('07.526.557/0001-00'), true); // Ambev
  assert.equal(validarCnpj('00.000.000/0001-91'), true); // Banco do Brasil

  // Sequências repetidas
  assert.equal(validarCnpj('00000000000000'), false);
  assert.equal(validarCnpj('11111111111111'), false);

  // Dígitos verificadores incorretos
  assert.equal(validarCnpj('33.000.167/0001-02'), false);
  assert.equal(validarCnpj('33.000.167/0001-99'), false);

  // Tamanhos incorretos
  assert.equal(validarCnpj('123'), false);
  assert.equal(validarCnpj(''), false);
});

test('cn deve mesclar classes tailwind corretamente', () => {
  const result = cn('px-2 py-1', 'bg-blue-500', { 'text-white': true, 'hidden': false });
  assert.match(result, /px-2/);
  assert.match(result, /bg-blue-500/);
  assert.match(result, /text-white/);
  assert.doesNotMatch(result, /hidden/);

  // Resolução de conflitos tailwind
  const conflict = cn('p-2', 'p-4');
  assert.equal(conflict, 'p-4');
});
