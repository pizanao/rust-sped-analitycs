"""
Ferramentas de auditoria contábil e fiscal para agentes LangChain / LangGraph.
"""

import json
from typing import Optional
from langchain_core.tools import tool
from app.client import get_backend_client


@tool
async def buscar_dados_cadastrais(cnpj: str) -> str:
    """
    Consulta os dados cadastrais oficiais da Receita Federal do Brasil (RFB) para um CNPJ.
    Retorna razão social, identificador matriz/filial, CNAE principal e situação cadastral.
    """
    clean_cnpj = "".join(filter(str.isdigit, cnpj))
    if len(clean_cnpj) != 14:
        return json.dumps({
            "erro": f"CNPJ inválido: '{cnpj}'. O CNPJ deve conter exatamente 14 dígitos numéricos.",
            "valido": False
        }, ensure_ascii=False)

    client = get_backend_client()
    dados = await client.get_dados_cadastrais(clean_cnpj)
    return json.dumps(dados, ensure_ascii=False)


@tool
async def apurar_demonstrativo_dre(cnpj: str, ano: Optional[int] = None) -> str:
    """
    Recupera a apuração da DRE (Demonstração do Resultado do Exercício) da escrituração SPED ECD.
    Retorna Receita Bruta apurada, Receita de Serviços, Outras Receitas, deduções e resultado do exercício.
    """
    clean_cnpj = "".join(filter(str.isdigit, cnpj))
    if len(clean_cnpj) != 14:
        return json.dumps({
            "erro": f"CNPJ inválido: '{cnpj}'. O CNPJ deve conter exatamente 14 dígitos numéricos.",
            "valido": False
        }, ensure_ascii=False)

    client = get_backend_client()
    dados = await client.get_demonstrativo_dre(clean_cnpj, ano=ano)
    return json.dumps(dados, ensure_ascii=False)
