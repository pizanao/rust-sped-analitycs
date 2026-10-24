"""
Catálogo de ferramentas LangChain para auditoria contábil, fiscal e GRC.
"""

from app.tools.fiscal_tools import (
    buscar_dados_cadastrais,
    apurar_demonstrativo_dre,
)
from app.tools.graph_tools import (
    analisar_grafo_societario,
    consultar_indicadores_setor,
    construir_grafico_dashboard,
    get_collected_charts,
    reset_collected_charts,
)

ALL_TOOLS = [
    buscar_dados_cadastrais,
    apurar_demonstrativo_dre,
    analisar_grafo_societario,
    consultar_indicadores_setor,
    construir_grafico_dashboard,
]

__all__ = [
    "buscar_dados_cadastrais",
    "apurar_demonstrativo_dre",
    "analisar_grafo_societario",
    "consultar_indicadores_setor",
    "construir_grafico_dashboard",
    "get_collected_charts",
    "reset_collected_charts",
    "ALL_TOOLS",
]
