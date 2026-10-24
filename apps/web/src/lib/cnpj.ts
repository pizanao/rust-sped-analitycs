/**
 * Utilitários de validação e formatação de CNPJ em conformidade com o motor Rust (ecd-domain)
 */

export function apenasDigitos(valor: string): string {
  return valor.replace(/\D/g, "");
}

export function formatarCpf(valor: string): string {
  const digits = apenasDigitos(valor).slice(0, 11);
  if (digits.length <= 3) return digits;
  if (digits.length <= 6) return `${digits.slice(0, 3)}.${digits.slice(3)}`;
  if (digits.length <= 9) return `${digits.slice(0, 3)}.${digits.slice(3, 6)}.${digits.slice(6)}`;
  return `${digits.slice(0, 3)}.${digits.slice(3, 6)}.${digits.slice(6, 9)}-${digits.slice(9, 11)}`;
}

export function formatarCnpj(valor: string): string {
  const digits = apenasDigitos(valor).slice(0, 14);
  if (digits.length <= 2) return digits;
  if (digits.length <= 5) return `${digits.slice(0, 2)}.${digits.slice(2)}`;
  if (digits.length <= 8) return `${digits.slice(0, 2)}.${digits.slice(2, 5)}.${digits.slice(5)}`;
  if (digits.length <= 12)
    return `${digits.slice(0, 2)}.${digits.slice(2, 5)}.${digits.slice(5, 8)}/${digits.slice(8)}`;
  return `${digits.slice(0, 2)}.${digits.slice(2, 5)}.${digits.slice(5, 8)}/${digits.slice(8, 12)}-${digits.slice(12, 14)}`;
}

export function formatarCpfCnpj(valor: string): string {
  const digits = apenasDigitos(valor);
  if (digits.length <= 11) {
    return formatarCpf(valor);
  }
  return formatarCnpj(valor);
}

export function validarCnpj(cnpjInput: string): boolean {
  const digits = apenasDigitos(cnpjInput);
  if (digits.length !== 14) return false;

  // Rejeita sequências repetidas (ex: 00000000000000, 11111111111111)
  if (/^(\d)\1{13}$/.test(digits)) return false;

  const nums = digits.split("").map(Number);

  // 1º Dígito Verificador
  const pesos1 = [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
  let soma1 = 0;
  for (let i = 0; i < 12; i++) {
    soma1 += nums[i] * pesos1[i];
  }
  const resto1 = soma1 % 11;
  const dig1 = resto1 < 2 ? 0 : 11 - resto1;
  if (nums[12] !== dig1) return false;

  // 2º Dígito Verificador
  const pesos2 = [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
  let soma2 = 0;
  for (let i = 0; i < 13; i++) {
    soma2 += nums[i] * pesos2[i];
  }
  const resto2 = soma2 % 11;
  const dig2 = resto2 < 2 ? 0 : 11 - resto2;
  return nums[13] === dig2;
}
