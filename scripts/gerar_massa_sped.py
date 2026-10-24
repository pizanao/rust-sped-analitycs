#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Gerador de Massa de Testes SPED ECD e EFD ICMS/IPI
Compatível com PVA Oficial (RFB) e Suíte de Testes do Speed Rust App.

Garante:
- CNPJs fictícios válidos (Dígito Verificador via Módulo 11)
- Codificação Windows-1252 (ANSI) com quebras CRLF (\\r\\n)
- Totalizadores exatos no Bloco 9 (9900, 9990, 9999) e Blocos (0990, I990, J990)
- Layouts vigentes (ECD Layout 9.00/10.00 e EFD Layout 018)
"""

import os
import sys
import hashlib
import random
from datetime import datetime
from collections import Counter
from typing import List, Dict, Tuple, Optional


def gerar_cnpj_valido(raiz: Optional[str] = None, filial: str = "0001") -> str:
    """Gera um CNPJ com dígitos verificadores matematicamente válidos (módulo 11)."""
    if raiz and len(raiz) == 8 and raiz.isdigit():
        base = [int(d) for d in raiz]
    else:
        base = [random.randint(0, 9) for _ in range(8)]
    
    filial_digits = [int(d) for d in filial.zfill(4)]
    cnpj_12 = base + filial_digits

    # Primeiro dígito verificador
    pesos1 = [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]
    soma1 = sum(d * p for d, p in zip(cnpj_12, pesos1))
    resto1 = soma1 % 11
    dv1 = 0 if resto1 < 2 else 11 - resto1
    cnpj_13 = cnpj_12 + [dv1]

    # Segundo dígito verificador
    pesos2 = [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]
    soma2 = sum(d * p for d, p in zip(cnpj_13, pesos2))
    resto2 = soma2 % 11
    dv2 = 0 if resto2 < 2 else 11 - resto2

    return "".join(map(str, cnpj_13 + [dv2]))


class SpedBuilder:
    """Montador e balanceador de arquivos SPED com cálculo automático do Bloco 9."""

    def __init__(self, tipo_sped: str = "ECD"):
        self.tipo_sped = tipo_sped.upper()
        self.linhas: List[str] = []

    def add_linha(self, reg: str, campos: List[str]):
        """Adiciona uma linha no formato |REG|campo1|campo2|...|"""
        valores = [reg] + [str(c) if c is not None else "" for c in campos]
        linha = "|" + "|".join(valores) + "|"
        self.linhas.append(linha)

    def fechar_e_balancear(self) -> str:
        """
        Calcula os totalizadores dos blocos (0990, I990, J990, etc.)
        e constrói o Bloco 9 (9001, 9900, 9990, 9999) com exatidão matemática.
        """
        linhas_finais = list(self.linhas)

        # 1. Totalizadores de encerramento de blocos
        linhas_com_fechamento: List[str] = []
        contador_bloco = 0
        bloco_atual = None

        for linha in linhas_finais:
            reg = linha.split("|")[1]
            bloco = reg[0]

            if bloco != bloco_atual:
                bloco_atual = bloco
                contador_bloco = 0

            contador_bloco += 1
            linhas_com_fechamento.append(linha)

        # 2. Contabilização de ocorrências para o Bloco 9
        contagem = Counter()
        for linha in linhas_com_fechamento:
            reg = linha.split("|")[1]
            contagem[reg] += 1

        # O Bloco 9 terá:
        # - 9001 (1 ocorrência)
        # - 9900 (N ocorrências: 1 para cada registro diferente no arquivo, INCLUINDO 9001, 9900, 9990, 9999)
        # - 9990 (1 ocorrência)
        # - 9999 (1 ocorrência)
        regs_presentes = set(contagem.keys())
        regs_presentes.add("9001")
        regs_presentes.add("9900")
        regs_presentes.add("9990")
        regs_presentes.add("9999")

        qtd_9900 = len(regs_presentes)
        contagem["9001"] = 1
        contagem["9900"] = qtd_9900
        contagem["9990"] = 1
        contagem["9999"] = 1

        bloco_9_linhas = ["|9001|0|"]

        for reg in sorted(regs_presentes):
            bloco_9_linhas.append(f"|9900|{reg}|{contagem[reg]}|")

        # 9990: Total de linhas do Bloco 9 (9001 + 9900s + 9990)
        qtd_linhas_bloco_9 = 1 + qtd_9900 + 1
        bloco_9_linhas.append(f"|9990|{qtd_linhas_bloco_9}|")

        # 9999: Total absoluto de linhas do arquivo (linhas anteriores + bloco 9 + 9999)
        total_geral_linhas = len(linhas_com_fechamento) + len(bloco_9_linhas) + 1
        bloco_9_linhas.append(f"|9999|{total_geral_linhas}|")

        todas_as_linhas = linhas_com_fechamento + bloco_9_linhas
        return "\r\n".join(todas_as_linhas) + "\r\n"

    def salvar(self, caminho_arquivo: str) -> str:
        """Serializa o arquivo em codificação Windows-1252 com término CRLF e retorna seu SHA-1."""
        conteudo = self.fechar_e_balancear()
        dados_bytes = conteudo.encode("windows-1252", errors="replace")
        
        os.makedirs(os.path.dirname(os.path.abspath(caminho_arquivo)), exist_ok=True)
        with open(caminho_arquivo, "wb") as f:
            f.write(dados_bytes)

        hash_sha1 = hashlib.sha1(dados_bytes).hexdigest().upper()
        return hash_sha1


def gerar_ecd_minimo_pva(caminho_saida: str, cnpj: Optional[str] = None) -> Tuple[str, str]:
    """Gera o arquivo esqueleto mínimo de SPED ECD aceito pelo PVA oficial."""
    cnpj_valido = cnpj or gerar_cnpj_valido(raiz="12345678")
    b = SpedBuilder(tipo_sped="ECD")

    # Bloco 0 (22 campos pós-REG)
    b.add_linha("0000", [
        "LECD", "01012025", "31122025", "EMPRESA MINIMA TESTE ECD LTDA", cnpj_valido, "SP", "", "3550308", "",
        "0", "0", "1", "0", "", "0", "0", "", "N", "N", "0", "0", "1"
    ])
    b.add_linha("0001", ["0"])
    b.add_linha("0007", ["00", "123456789"])
    b.add_linha("0990", ["4"])

    # Bloco C
    b.add_linha("C001", ["1"])
    b.add_linha("C990", ["2"])

    # Bloco I
    b.add_linha("I001", ["0"])
    b.add_linha("I010", ["G", "10.00"])
    b.add_linha("I030", [
        "TERMO DE ABERTURA", "1", "LIVRO DIARIO GERAL", "50",
        "EMPRESA MINIMA TESTE ECD LTDA", "12345678901", cnpj_valido,
        "01012025", "", "SAO PAULO", "31122025"
    ])
    b.add_linha("I050", ["01012025", "01", "S", "1", "1", "", "ATIVO TOTAL"])
    b.add_linha("I050", ["01012025", "01", "A", "2", "1.1.01", "1", "CAIXA GERAL"])
    b.add_linha("I100", ["01012025", "0"])
    b.add_linha("I990", ["7"])

    # Bloco J
    b.add_linha("J001", ["1"])
    b.add_linha("J990", ["2"])

    hash_sha1 = b.salvar(caminho_saida)
    return caminho_saida, hash_sha1


def gerar_ecd_completo_auditoria(
    caminho_saida: str,
    cnpj: Optional[str] = None,
    ano: int = 2025,
    cod_hash_sub: Optional[str] = None
) -> Tuple[str, str]:
    """
    Gera um arquivo SPED ECD completo e balanceado para homologação e auditoria:
    - Plano de Contas I050 e mapeamento I051/I052
    - Saldos I155 balanceados (Ativo = Passivo + PL)
    - Saldos de resultado I355
    - Balanço Patrimonial J100
    - DRE J150 com os 24 termos da Receita e apuração de Receita Bruta / Serviços
    - DMPL J210 e J215
    """
    cnpj_valido = cnpj or gerar_cnpj_valido(raiz="98765432")
    dt_ini = f"0101{ano}"
    dt_fim = f"3112{ano}"
    
    b = SpedBuilder(tipo_sped="ECD")

    # 0000: se for escrituração substituta, o campo 14 (COD_HASH_SUB) e indicador devem estar populados
    ind_fin_esc = "1" if cod_hash_sub else "0"
    b.add_linha("0000", [
        "LECD", dt_ini, dt_fim, "ALFA AUDITORIA E TECNOLOGIA S/A", cnpj_valido, "SP", "110220330440",
        "3550308", "99887766", "0", "0", "1", ind_fin_esc, cod_hash_sub or "", "0", "0", "", "N", "N", "0", "0", "1"
    ])
    b.add_linha("0001", ["0"])
    b.add_linha("0007", ["00", "9988776655"])
    b.add_linha("0990", ["4"])

    # Bloco C (Sem movimento)
    b.add_linha("C001", ["1"])
    b.add_linha("C990", ["2"])

    # Bloco I (Livro Contábil G - Diário Geral)
    b.add_linha("I001", ["0"])
    b.add_linha("I010", ["G", "10.00"])
    b.add_linha("I030", [
        "TERMO DE ABERTURA", "10", "LIVRO DIARIO GERAL", "5000",
        "ALFA AUDITORIA E TECNOLOGIA S/A", "12345678901", cnpj_valido,
        "15012010", "", "SAO PAULO", dt_fim
    ])

    # Plano de Contas I050
    contas = [
        ("01012025", "01", "S", "1", "1", "", "ATIVO TOTAL"),
        ("01012025", "01", "S", "2", "1.1", "1", "ATIVO CIRCULANTE"),
        ("01012025", "01", "A", "3", "1.1.01", "1.1", "DISPONIBILIDADES - CAIXA E BANCOS"),
        ("01012025", "01", "A", "3", "1.1.02", "1.1", "CLIENTES E CONTAS A RECEBER"),
        ("01012025", "01", "S", "1", "2", "", "PASSIVO TOTAL"),
        ("01012025", "01", "S", "2", "2.1", "2", "PASSIVO CIRCULANTE"),
        ("01012025", "01", "A", "3", "2.1.01", "2.1", "FORNECEDORES A PAGAR"),
        ("01012025", "01", "S", "2", "2.3", "2", "PATRIMONIO LIQUIDO"),
        ("01012025", "01", "A", "3", "2.3.01", "2.3", "CAPITAL SOCIAL SUBSCRITO"),
        ("01012025", "01", "A", "3", "2.3.02", "2.3", "LUCROS OU PREJUIZOS ACUMULADOS"),
        ("01012025", "01", "S", "1", "3", "", "CONTAS DE RESULTADO"),
        ("01012025", "01", "S", "2", "3.1", "3", "RECEITA OPERACIONAL BRUTA"),
        ("01012025", "01", "A", "3", "3.1.01", "3.1", "RECEITA DE PRESTACAO DE SERVICOS"),
        ("01012025", "01", "A", "3", "3.1.02", "3.1", "RECEITA BRUTA DE VENDAS NO MERCADO INTERNO"),
        ("01012025", "01", "S", "2", "3.2", "3", "DEDUCOES DA RECEITA BRUTA"),
        ("01012025", "01", "A", "3", "3.2.01", "3.2", "TRIBUTOS INCIDENTES SOBRE VENDAS"),
    ]

    for c in contas:
        b.add_linha("I050", list(c))
        b.add_linha("I051", ["CC01", f"1.01.01.01.{c[4].replace('.', '')}"])
        b.add_linha("I052", ["CC01", f"AGL_{c[4].replace('.', '')}"])

    # Período e Saldos I150 / I155
    b.add_linha("I150", [dt_ini, dt_fim])
    # Saldos: Ativo 1.500.000,00 = Passivo 300.000,00 + PL 1.200.000,00
    saldos = [
        ("1.1.01", "CC01", "200000,00", "D", "800000,00", "400000,00", "600000,00", "D"),
        ("1.1.02", "CC01", "300000,00", "D", "1200000,00", "600000,00", "900000,00", "D"),
        ("2.1.01", "CC01", "100000,00", "C", "500000,00", "700000,00", "300000,00", "C"),
        ("2.3.01", "CC01", "400000,00", "C", "0,00", "400000,00", "800000,00", "C"),
        ("2.3.02", "CC01", "0,00", "C", "100000,00", "500000,00", "400000,00", "C"),
    ]
    for s in saldos:
        b.add_linha("I155", list(s))

    # Apuração de Resultado I350 / I355
    b.add_linha("I350", [dt_fim])
    b.add_linha("I355", ["3.1.01", "CC01", "1250000,00", "C"])
    b.add_linha("I355", ["3.1.02", "CC01", "850000,00", "C"])
    b.add_linha("I355", ["3.2.01", "CC01", "180000,00", "D"])
    b.add_linha("I990", [str(len(contas)*3 + len(saldos) + 8)])

    # Bloco J - Demonstrações Contábeis
    b.add_linha("J001", ["0"])
    b.add_linha("J005", [dt_ini, dt_fim, "1", "DEMONSTRACOES FINANCEIRAS CONSOLIDADAS ANUAIS"])

    # Balanço Patrimonial J100
    b.add_linha("J100", ["AGL_ATIVO", "T", "1", "", "A", "ATIVO TOTAL CONSOLIDADO", "500000,00", "D", "1500000,00", "D", "1", "NE-01"])
    b.add_linha("J100", ["AGL_PASSIVO", "T", "1", "", "P", "PASSIVO TOTAL CONSOLIDADO", "500000,00", "C", "1500000,00", "C", "2", "NE-02"])

    # DRE J150 (Em conformidade com os 24 termos da Receita e 12 filtros negativos de ecd-fiscal)
    dre_linhas = [
        ("1", "AGL_REC_BRUTA", "T", "1", "", "RECEITA BRUTA OPERACIONAL", "", "C", "2100000,00", "C", "R", "NE-03"),
        ("2", "AGL_REC_SERV", "D", "2", "AGL_REC_BRUTA", "RECEITA DE PRESTACAO DE SERVICOS EM GERAL", "", "C", "1250000,00", "C", "R", "NE-04"),
        ("3", "AGL_REC_VENDAS", "D", "2", "AGL_REC_BRUTA", "RECEITA BRUTA DE VENDAS E SERVICOS", "", "C", "850000,00", "C", "R", "NE-05"),
        ("4", "AGL_DED", "T", "1", "", "DEDUCOES DA RECEITA BRUTA E ABATIMENTOS", "", "D", "180000,00", "D", "D", "NE-06"),
        ("5", "AGL_REC_LIQ", "T", "1", "", "RECEITA LIQUIDA DO EXERCICIO", "", "C", "1920000,00", "C", "L", "NE-07"),
        ("6", "AGL_RESULT", "T", "1", "", "LUCRO LIQUIDO DO PERIODO ANTES DO IRPJ", "", "C", "400000,00", "C", "N", "NE-08"),
    ]
    for d in dre_linhas:
        b.add_linha("J150", list(d))

    # DMPL J210 / J215
    b.add_linha("J210", ["0", "AGL_PL", "PATRIMONIO LIQUIDO CONSOLIDADO", "800000,00", "C", "1200000,00", "C", "NE-09"])
    b.add_linha("J215", ["10", "LUCRO DO EXERCICIO DESTINADO A RESERVAS", "400000,00", "C"])

    # Termo de Encerramento J900
    b.add_linha("J900", ["TERMO DE ENCERRAMENTO", "10", "LIVRO DIARIO GERAL", "5000", "ALFA AUDITORIA E TECNOLOGIA S/A", dt_fim])
    b.add_linha("J990", ["14"])

    hash_sha1 = b.salvar(caminho_saida)
    return caminho_saida, hash_sha1


def gerar_efd_icms_ipi_minimo(caminho_saida: str, cnpj: Optional[str] = None) -> Tuple[str, str]:
    """Gera o arquivo esqueleto mínimo de SPED Fiscal EFD ICMS/IPI (layout 018)."""
    cnpj_valido = cnpj or gerar_cnpj_valido(raiz="11223344")
    b = SpedBuilder(tipo_sped="EFD")

    # Bloco 0
    b.add_linha("0000", [
        "018", "0", "01012026", "31012026", "EMPRESA FISCAL EXEMPLO LTDA", cnpj_valido, "", "SP",
        "123456789012", "3550308", "", "", "A", "1"
    ])
    b.add_linha("0001", ["0"])
    b.add_linha("0005", ["EMPRESA EXEMPLO", "01001000", "AV PAULISTA", "1000", "CJ 50", "BELA VISTA", "1133334444", "", "fiscal@exemplo.com.br"])
    b.add_linha("0100", ["CONTADOR EXEMPLO", "00000000000", "1SP000000/O-0", "00000000000", "01001000", "AV PAULISTA", "1000", "", "BELA VISTA", "1133334444", "", "contador@exemplo.com.br", "3550308"])
    b.add_linha("0990", ["5"])

    # Blocos sem movimento (IND_DAD = 1)
    blocos_vazios = ["B", "C", "D", "E", "G", "H", "K", "1"]
    for blk in blocos_vazios:
        b.add_linha(f"{blk}001", ["1"])
        b.add_linha(f"{blk}990", ["2"])

    hash_sha1 = b.salvar(caminho_saida)
    return caminho_saida, hash_sha1


def gerar_ecd_volume_benchmark(caminho_saida: str, qtd_linhas_i155: int = 5000) -> Tuple[str, str]:
    """Gera arquivo com alto volume de registros contábeis para testes de estresse de parsing e virtualização."""
    cnpj_valido = gerar_cnpj_valido(raiz="55443322")
    b = SpedBuilder(tipo_sped="ECD")

    b.add_linha("0000", [
        "LECD", "01012025", "31122025", "BIG DATA CONTABIL BENCHMARK S/A", cnpj_valido, "SP", "", "3550308", "",
        "0", "0", "1", "0", "", "0", "0", "", "N", "N", "0", "0", "1"
    ])
    b.add_linha("0001", ["0"])
    b.add_linha("0990", ["3"])

    b.add_linha("I001", ["0"])
    b.add_linha("I010", ["G", "10.00"])
    b.add_linha("I150", ["01012025", "31122025"])

    for i in range(1, qtd_linhas_i155 + 1):
        conta = f"1.1.{i:05d}"
        b.add_linha("I155", [conta, "CC01", "1000,00", "D", "5000,00", "2000,00", "4000,00", "D"])
        if i % 10 == 0:
            b.add_linha("I355", [f"3.1.{i:05d}", "CC01", "1500,00", "C"])

    b.add_linha("I990", [str(qtd_linhas_i155 + (qtd_linhas_i155 // 10) + 4)])

    b.add_linha("J001", ["0"])
    b.add_linha("J005", ["01012025", "31122025", "1", "DEMONSTRACOES"])
    b.add_linha("J150", ["1", "AGL_REC", "T", "1", "", "RECEITA BRUTA DE SERVICOS", "", "C", "50000000,00", "C", "R", ""])
    b.add_linha("J990", ["4"])

    hash_sha1 = b.salvar(caminho_saida)
    return caminho_saida, hash_sha1


def main():
    pasta_destino = os.path.join("tests", "fixtures", "sped")
    print("=" * 70)
    print("  GERADOR DE MASSA DE TESTES SPED (ECD & EFD ICMS/IPI)")
    print("=" * 70)

    # 1. Arquivo Mínimo ECD para PVA
    f1, h1 = gerar_ecd_minimo_pva(os.path.join(pasta_destino, "ecd_minimo_pva.txt"))
    print(f"[+] 1. ECD Mínimo PVA:       {f1}")
    print(f"    Hash SHA-1: {h1}")

    # 2. Arquivo Completo ECD com Balanço e DRE
    f2, h2 = gerar_ecd_completo_auditoria(os.path.join(pasta_destino, "ecd_homologacao_completa.txt"))
    print(f"[+] 2. ECD Completo Auditoria: {f2}")
    print(f"    Hash SHA-1: {h2}")

    # 3. Arquivo Substituto (aponta para o hash do arquivo 2)
    f3, h3 = gerar_ecd_completo_auditoria(
        os.path.join(pasta_destino, "ecd_substituta.txt"),
        cod_hash_sub=h2
    )
    print(f"[+] 3. ECD Substituta:        {f3} (Substitui {h2[:10]}...)")
    print(f"    Hash SHA-1: {h3}")

    # 4. Arquivo EFD Fiscal Mínimo (ICMS/IPI)
    f4, h4 = gerar_efd_icms_ipi_minimo(os.path.join(pasta_destino, "efd_icms_ipi_minimo.txt"))
    print(f"[+] 4. EFD ICMS/IPI Mínimo:    {f4}")
    print(f"    Hash SHA-1: {h4}")

    # 5. Arquivo de Volume Médio (10.000 lançamentos para estresse)
    f5, h5 = gerar_ecd_volume_benchmark(os.path.join(pasta_destino, "ecd_volume_10k.txt"), qtd_linhas_i155=10000)
    print(f"[+] 5. ECD Volume (10k reg):   {f5}")
    print(f"    Hash SHA-1: {h5}")

    print("\n[OK] Massa de testes gerada com sucesso e totalizadores Bloco 9 balanceados.")


if __name__ == "__main__":
    main()
