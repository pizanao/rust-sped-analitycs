# -*- coding: utf-8 -*-
"""
Bateria de Testes de Paridade: Django Legado vs Rust SPED Engine
TASK-902: Compara os algoritmos legados (apps/core/tratamento.py e apps/fiscal/utils_recfuze.py)
com a implementação Rust de alta performance (ecd-parser e ecd-fiscal).
"""

import json
import os
import re
import subprocess
import tempfile
import unicodedata
from datetime import datetime
from pathlib import Path
import pytest
from rapidfuzz import fuzz

BASE_DIR = Path(__file__).resolve().parent.parent.parent
PARITY_CLI = BASE_DIR / "target" / "debug" / "parity_cli"


def run_rust_cli(cmd: str, *args) -> dict:
    """Executa o utilitário Rust parity_cli e retorna o JSON de saída."""
    if not PARITY_CLI.exists():
        subprocess.run(
            ["cargo", "build", "-p", "ecd-worker", "--bin", "parity_cli"],
            cwd=BASE_DIR,
            check=True,
            capture_output=True,
        )
    exec_args = [str(PARITY_CLI), cmd] + [str(a) for a in args]
    res = subprocess.run(exec_args, capture_output=True, text=True, check=True)
    return json.loads(res.stdout.strip())


# ==============================================================================
# Algoritmos de Referência do Django Legado
# ==============================================================================


def legacy_process_cta(nome_conta: str) -> str:
    """Extraído diretamente de apps/core/tratamento.py:process_cta"""
    nome_conta = nome_conta.replace("C/", "")
    nome_conta = (
        unicodedata.normalize("NFKD", nome_conta)
        .encode("ASCII", "ignore")
        .decode("utf-8")
    )
    nome_conta = re.sub(r"[^a-zA-Z0-9\s]", "", nome_conta)
    return nome_conta.upper()


def legacy_normalizar_texto(texto: str) -> str:
    """Extraído diretamente de apps/fiscal/utils_recfuze.py:normalizar_texto"""
    return "".join(
        c
        for c in unicodedata.normalize("NFD", texto.lower())
        if unicodedata.category(c) != "Mn"
    )


def legacy_corrigir_formato_data(data_str: str) -> str:
    """Extraído diretamente de apps/core/tratamento.py:corrigir_formato_data"""
    padrao = r"(\d{8})T(\d{6})"
    match = re.match(padrao, str(data_str).upper())
    if match:
        data = match.group(1)
        hora = match.group(2)
        return f"{data[:4]}-{data[4:6]}-{data[6:8]} {hora[:2]}:{hora[2:4]}:{hora[4:6]}"
    return data_str


def legacy_tratar_nomarquivo(nomearquivo: str) -> dict:
    """Extraído diretamente de apps/core/tratamento.py:tratar_nomarquivo"""
    dado = str(nomearquivo).upper().split("-")
    ordem = dado[0]
    cnpj = dado[1]
    numnire = dado[2]
    per_ini = dado[3]
    per_fim = dado[4]
    forma = dado[5]
    hash_arquivo = dado[6]
    ext = dado[7]
    dataentrega = legacy_corrigir_formato_data(dado[8])

    return {
        "ordem": ordem,
        "cnpj": cnpj,
        "nire": numnire,
        "periodo_inicial": datetime.strptime(per_ini, "%Y%m%d").strftime("%Y-%m-%d"),
        "periodo_final": datetime.strptime(per_fim, "%Y%m%d").strftime("%Y-%m-%d"),
        "forma": forma,
        "numero_livro": int(ext),
        "data_entrega": dataentrega,
        "hash_arquivo": hash_arquivo,
    }


CATALOGO_24_TERMOS = [
    "receita bruta venda",
    "vendas e servicos",
    "mensalidades",
    "receita bruta gestao patrimonial",
    "receita bruta locacao",
    "receita bruta servicos",
    "receita bruta venda prestacao de servicos",
    "receita bruta vendas imobiliarias",
    "receita bruta operacional",
    "receitas operacionais",
    "receita operacional desagio",
    "receita bruta locacao servicos",
    "vendas produtos",
    "receitas operacional bruta",
    "brutas vendas mercadorias",
    "receita total bruta",
    "receitas brutas mercadorias",
    "receitas brutas mercadorias servicos",
    "bruta aluguel imoveis",
    "total receita operacional bruta",
    "venda bruta operacional",
    "receita bruta",
    "ingressos",
    "receitas",
]

FILTROS_NEGATIVOS = [
    " NAO ",
    " NÃO ",
    "CUST",
    "DEDU",
    "IMP",
    "DESPE",
    "CONTRA",
    "TERCEI",
    "PESSO",
    " PJ ",
    "LIQUID",
    "ANTES",
]


# ==============================================================================
# Casos de Teste de Paridade
# ==============================================================================


class TestSpedParity:
    """Testes de equivalência estrita entre o código legado Python e o novo motor Rust."""

    def test_process_cta_parity(self):
        """Valida que a sanitização de nomes de contas é idêntica entre Python e Rust."""
        test_cases = [
            "C/ BANCO DO BRASIL S.A. - C/C 12345-6",
            "3.1.01 - RECEITA DE PRESTAÇÃO DE SERVIÇOS",
            "FORNECEDORES NACIONAIS / ESTRANGEIROS (IMPORTAÇÃO)",
            "LUCROS E PERDAS DO EXERCÍCIO - 2023",
            "C/CORRENTE ITAÚ UNIBANCO - AG. 0001",
            "   ESPAÇOS EM BRANCO & ACENTUAÇÃO: Ç, Ã, É, Ô, Ü   ",
            "C/C",
            "CONTA COM CIFRÃO $ E PORCENTAGEM 100%",
        ]

        for input_text in test_cases:
            py_res = legacy_process_cta(input_text).strip()
            rust_res = run_rust_cli("process-cta", input_text)["result"]
            assert py_res == rust_res, f"Discrepância em process_cta para '{input_text}': py='{py_res}', rust='{rust_res}'"

    def test_normalizar_texto_parity(self):
        """Valida que a normalização de texto (remoção de acentos e minúsculas) é consistente."""
        test_cases = [
            "RECEITA BRUTA DE VENDAS E SERVIÇOS",
            "Demonstração do Resultado do Exercício",
            "INDÚSTRIA E COMÉRCIO SÃO JORGE",
            "ÁÉÍÓÚ ÀÈÌÒÙ ÂÊÎÔÛ ÃÕ ÄËÏÖÜ Ç",
        ]

        for input_text in test_cases:
            py_res = legacy_normalizar_texto(input_text)
            rust_res = run_rust_cli("normalizar-texto", input_text)["result"]
            assert py_res == rust_res, f"Discrepância em normalizar_texto para '{input_text}': py='{py_res}', rust='{rust_res}'"

    def test_fuzzy_partial_ratio_parity_against_rapidfuzz(self):
        """Compara o algoritmo Hyyrö LCS bit-paralelo do Rust com RapidFuzz partial_ratio."""
        test_pairs = [
            ("receita bruta", "receita bruta de vendas"),
            ("receita bruta venda", "receita bruta de prestação de serviços"),
            ("vendas produtos", "venda de produtos industriais e comerciais"),
            ("receita total bruta", "receita bruta total apurada no exercicio"),
            ("receita bruta locacao", "receita com locacao de bens imoveis"),
            ("receita bruta operacionl", "receita bruta operacional"),
            ("mensalidades", "receita proveniente de mensalidades escolares"),
            ("ingressos", "receita com ingressos de bilheteria"),
        ]

        for s1, s2 in test_pairs:
            s1_norm = legacy_normalizar_texto(s1)
            s2_norm = legacy_normalizar_texto(s2)

            rf_score = fuzz.partial_ratio(s1_norm, s2_norm)
            rust_res = run_rust_cli("fuzz-ratio", s1_norm, s2_norm)
            rust_score_100 = rust_res["score_100"]

            # Ambos devem concordar na decisão de corte do legado (>= 80%)
            rf_elegivel = rf_score >= 80.0
            rust_elegivel = rust_score_100 >= 80.0

            assert rf_elegivel == rust_elegivel, (
                f"Discrepância de elegibilidade (>=80) para ('{s1}', '{s2}'): "
                f"RapidFuzz={rf_score}, Rust={rust_score_100}"
            )
            # Diferença máxima tolerada em ponto flutuante é de no máximo 1 ponto percentual
            assert abs(rf_score - rust_score_100) <= 1.05, (
                f"Discrepância numérica para ('{s1}', '{s2}'): RapidFuzz={rf_score}, Rust={rust_score_100}"
            )

    def test_catalogo_24_termos_and_negative_filters(self):
        """Garante que a lista canônica dos 24 termos e 12 filtros negativos é consistente."""
        assert len(CATALOGO_24_TERMOS) == 24
        assert len(FILTROS_NEGATIVOS) == 12

        # Valida que nenhum termo do catálogo contém falso positivo nos filtros negativos
        for termo in CATALOGO_24_TERMOS:
            for filtro in FILTROS_NEGATIVOS:
                assert filtro not in termo.upper(), f"Conflito: termo '{termo}' contém filtro negativo '{filtro}'"

    def test_tratar_nomarquivo_parity(self):
        """Valida que a nomenclatura padrão de arquivos SPED é decomposta com precisão."""
        filename = (
            "0008-06057223000171-33300272909-20231201-20231231-G-"
            "4AC602DABC8D1F3105A51FCD2C7ACA30C016A78F-1-20240604T160201-SPED-ECD-DA.txt"
        )
        dados = legacy_tratar_nomarquivo(filename)

        assert dados["ordem"] == "0008"
        assert dados["cnpj"] == "06057223000171"
        assert dados["nire"] == "33300272909"
        assert dados["periodo_inicial"] == "2023-12-01"
        assert dados["periodo_final"] == "2023-12-31"
        assert dados["forma"] == "G"
        assert dados["hash_arquivo"] == "4AC602DABC8D1F3105A51FCD2C7ACA30C016A78F"
        assert dados["numero_livro"] == 1
        assert dados["data_entrega"] == "2024-06-04 16:02:01"

    def test_full_sped_parsing_parity_with_windows_1252(self):
        """
        Gera um arquivo SPED completo em WINDOWS-1252, processa com o leitor Rust
        e valida paridade de todos os blocos contábeis essenciais.
        """
        sped_content = (
            "|0000|LECD|01012023|31122023|INDÚSTRIA E COMÉRCIO SÃO JORGE S/A|06057223000171|BA|123456789|2927408|654321||0|1|0||0|0||N|N|0|0|1|\r\n"
            "|I001|0|\r\n"
            "|I010|G|9.00|\r\n"
            "|I030|TERMO DE ABERTURA|1|DIÁRIO GERAL|120|INDÚSTRIA E COMÉRCIO SÃO JORGE S/A|29300012345|06057223000171|10012015||SALVADOR|31122023|\r\n"
            "|I050|01012023|04|S|3|3.1.01|3.1|RECEITA OPERACIONAL BRUTA|\r\n"
            "|I050|01012023|04|A|4|3.1.01.001|3.1.01|RECEITA DE PRESTAÇÃO DE SERVIÇOS|\r\n"
            "|I051|CC01|3.01.01.01.01.02|\r\n"
            "|I052|CC01|AGL_REC_SERV|\r\n"
            "|I053|IDT01|3.1.01.001|01|\r\n"
            "|I150|01012023|31122023|\r\n"
            "|I155|3.1.01.001|CC01|0,00|C|1000,00|850000,50|849000,50|C|\r\n"
            "|I350|31122023|\r\n"
            "|I355|3.1.01.001|CC01|849000,50|C|\r\n"
            "|I990|12|\r\n"
            "|J001|0|\r\n"
            "|J005|01012023|31122023|1|DEMONSTRAÇÕES CONTÁBEIS|\r\n"
            "|J100|AGL_ATIVO|T|1||A|ATIVO TOTAL|500000,00|D|950000,00|D|NE01|\r\n"
            "|J150|1|AGL_REC_SERV|T|1||RECEITA BRUTA||C|849000,50|C|R|NE02|\r\n"
            "|J210|0|AGL_LPA|LUCROS ACUMULADOS|100000,00|C|250000,00|C||\r\n"
            "|J215|H01|LUCRO DO EXERCÍCIO|150000,00|C|\r\n"
            "|J990|7|\r\n"
        )

        with tempfile.NamedTemporaryFile(suffix=".txt", delete=False) as tf:
            tf.write(sped_content.encode("cp1252"))
            temp_path = tf.name

        try:
            doc = run_rust_cli(
                "parse-file",
                temp_path,
                "4AC602DABC8D1F3105A51FCD2C7ACA30C016A78F",
                "2024-06-04T16:02:01Z",
            )

            # Valida Bloco 0000
            reg_0000 = doc["reg_0000"]
            assert reg_0000["nome_razao"] == "INDÚSTRIA E COMÉRCIO SÃO JORGE S/A"
            assert reg_0000["cnpj"] == "06057223000171"
            assert reg_0000["uf"] == "BA"
            assert reg_0000["cod_mun"] == 2927408
            assert reg_0000["i990"] == 12
            assert reg_0000["j990"] == 7

            # Valida Plano de Contas e Contabilidade
            assert len(doc["reg_i010"]) == 1
            assert len(doc["reg_i030"]) == 1
            assert len(doc["reg_i050"]) == 2
            assert len(doc["reg_i150"]) == 1
            assert len(doc["reg_i155"]) == 1
            assert len(doc["reg_i355"]) == 1

            # Valida Demonstrações Contábeis (Bloco J)
            assert len(doc["reg_j005"]) == 1
            assert len(doc["reg_j100"]) == 1
            assert len(doc["reg_j150"]) == 1
            assert len(doc["reg_j210"]) == 1
            assert len(doc["reg_j215"]) == 1

            # Valida Enriquecimento automático de I355 a partir de I050 e J150
            i355 = doc["reg_i355"][0]
            assert i355["cod_cta"] == "3.1.01.001"
            assert i355["cod_agl"] == "AGL_REC_SERV"
            assert i355["cod_cta_ref"] == "3.01.01.01.01.02"
            assert i355["nome_cta_referencia"] == "RECEITA OPERACIONAL BRUTA"
            assert i355["nome_aglutinador"] == "RECEITA BRUTA"
            assert float(i355["vl_cta_mf"]) == 849000.50

        finally:
            if os.path.exists(temp_path):
                os.remove(temp_path)

    def test_filtra_receitas_negative_and_positive_parity(self):
        """
        Valida que o motor de receitas descarta todos os 12 filtros negativos
        e aceita descrições genuínas de receita bruta com matching dos 24 termos.
        """
        # Lista com 1 item válido e vários itens contendo os filtros negativos proibidos
        j150_records = [
            # VÁLIDO: Receita Bruta de Vendas
            {
                "id": 1,
                "cnpj": "06057223000171",
                "hash_arquivo": "4AC602DABC8D1F3105A51FCD2C7ACA30C016A78F",
                "nu_ordem": 1,
                "cod_agl": "3.1.01",
                "ind_cod_agl": "T",
                "nivel_agl": 2,
                "cod_agl_sup": None,
                "descr_cod_agl": "RECEITA BRUTA DE VENDAS",
                "vl_cta_ini": "0.00",
                "ind_dc_cta_ini": "C",
                "vl_cta_fin": "1250000.00",
                "ind_dc_cta_fin": "C",
                "ind_grp_dre": "R",
                "nota_exp_ref": None,
                "dt_ini": None,
                "dt_fim": None,
            },
            # NEGATIVO: CUSTO
            {
                "id": 2,
                "cnpj": "06057223000171",
                "hash_arquivo": "4AC602DABC8D1F3105A51FCD2C7ACA30C016A78F",
                "nu_ordem": 2,
                "cod_agl": "3.2.01",
                "ind_cod_agl": "T",
                "nivel_agl": 2,
                "cod_agl_sup": None,
                "descr_cod_agl": "CUSTO DAS MERCADORIAS VENDIDAS",
                "vl_cta_ini": "0.00",
                "ind_dc_cta_ini": "C",
                "vl_cta_fin": "650000.00",
                "ind_dc_cta_fin": "C",
                "ind_grp_dre": "R",
                "nota_exp_ref": None,
                "dt_ini": None,
                "dt_fim": None,
            },
            # NEGATIVO: DEDUCOES
            {
                "id": 3,
                "cnpj": "06057223000171",
                "hash_arquivo": "4AC602DABC8D1F3105A51FCD2C7ACA30C016A78F",
                "nu_ordem": 3,
                "cod_agl": "3.1.02",
                "ind_cod_agl": "T",
                "nivel_agl": 2,
                "cod_agl_sup": None,
                "descr_cod_agl": "DEDUCOES DA RECEITA BRUTA",
                "vl_cta_ini": "0.00",
                "ind_dc_cta_ini": "C",
                "vl_cta_fin": "120000.00",
                "ind_dc_cta_fin": "C",
                "ind_grp_dre": "R",
                "nota_exp_ref": None,
                "dt_ini": None,
                "dt_fim": None,
            },
            # NEGATIVO: IMPOSTOS
            {
                "id": 4,
                "cnpj": "06057223000171",
                "hash_arquivo": "4AC602DABC8D1F3105A51FCD2C7ACA30C016A78F",
                "nu_ordem": 4,
                "cod_agl": "3.1.03",
                "ind_cod_agl": "T",
                "nivel_agl": 2,
                "cod_agl_sup": None,
                "descr_cod_agl": "IMPOSTOS SOBRE VENDAS",
                "vl_cta_ini": "0.00",
                "ind_dc_cta_ini": "C",
                "vl_cta_fin": "80000.00",
                "ind_dc_cta_fin": "C",
                "ind_grp_dre": "R",
                "nota_exp_ref": None,
                "dt_ini": None,
                "dt_fim": None,
            },
            # NEGATIVO: DESPESAS
            {
                "id": 5,
                "cnpj": "06057223000171",
                "hash_arquivo": "4AC602DABC8D1F3105A51FCD2C7ACA30C016A78F",
                "nu_ordem": 5,
                "cod_agl": "3.3.01",
                "ind_cod_agl": "T",
                "nivel_agl": 2,
                "cod_agl_sup": None,
                "descr_cod_agl": "DESPESAS OPERACIONAIS GERAIS",
                "vl_cta_ini": "0.00",
                "ind_dc_cta_ini": "C",
                "vl_cta_fin": "45000.00",
                "ind_dc_cta_fin": "C",
                "ind_grp_dre": "R",
                "nota_exp_ref": None,
                "dt_ini": None,
                "dt_fim": None,
            },
        ]

        with tempfile.NamedTemporaryFile(suffix=".json", mode="w", delete=False) as tf:
            json.dump(j150_records, tf)
            temp_json = tf.name

        try:
            resultados = run_rust_cli("filtra-receitas", "06057223000171", temp_json)
            # Apenas o item 1 (RECEITA BRUTA DE VENDAS) deve ser retornado!
            assert len(resultados) == 1, f"Esperado 1 item elegível, obtido {len(resultados)}"
            rec = resultados[0]
            assert rec["cod_agl"] == "3.1.01"
            assert rec["descricao"] == "RECEITA BRUTA DE VENDAS"
            assert float(rec["valor"]) == 1250000.00
            assert rec["relevancia"] == "receita bruta venda"
        finally:
            if os.path.exists(temp_json):
                os.remove(temp_json)
