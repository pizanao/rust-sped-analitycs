"""
Ferramentas de análise de grafos societários (GRC), indicadores setoriais
e gerador de dashboards Recharts para o AI Gateway.
Implementa isolamento por ContextVar para prevenir race conditions em requisições concorrentes.
"""

import contextvars
import json
import threading
from typing import Any, Dict, List, Optional
from langchain_core.tools import tool
from app.client import get_backend_client
from app.models import ChartSpec, DatasetSpec

# Isolamento por ContextVar para execução assíncrona por requisição
_collected_charts_ctx: contextvars.ContextVar[Optional[List[ChartSpec]]] = contextvars.ContextVar(
    "_collected_charts_ctx", default=None
)

# Store de fallback para execução síncrona / testes diretos
_fallback_charts_store: List[ChartSpec] = []
_fallback_lock = threading.Lock()


def get_collected_charts() -> List[ChartSpec]:
    """Retorna cópia da lista de gráficos gerados no contexto atual de execução."""
    ctx_store = _collected_charts_ctx.get()
    if ctx_store is not None:
        return list(ctx_store)
    with _fallback_lock:
        return list(_fallback_charts_store)


def reset_collected_charts():
    """Reinicia a lista de gráficos para a requisição/contexto atual."""
    _collected_charts_ctx.set([])
    with _fallback_lock:
        _fallback_charts_store.clear()


def record_collected_chart(chart: ChartSpec):
    """Registra um gráfico de forma isolada no contexto atual."""
    ctx_store = _collected_charts_ctx.get()
    if ctx_store is not None:
        ctx_store.append(chart)
    else:
        with _fallback_lock:
            _fallback_charts_store.append(chart)


@tool
async def analisar_grafo_societario(cnpj_ou_cpf: str, camadas: int = 2) -> str:
    """
    Percorre a Rede CNPJ corporativa a partir de uma raiz (CNPJ ou CPF) em até N camadas.
    Retorna sócios, participações coligadas e alertas sancionatórios (PEP, CEIS, CEPIM, CNEP, PGFN).
    """
    clean_id = "".join(filter(str.isalnum, cnpj_ou_cpf))
    if not clean_id:
        return json.dumps({
            "erro": "Identificador de entidade (CNPJ ou CPF) vazio ou inválido.",
            "valido": False
        }, ensure_ascii=False)

    camadas_efetivas = max(1, min(camadas, 5))
    client = get_backend_client()
    dados = await client.get_grafo_societario(raiz=clean_id, camadas=camadas_efetivas)
    return json.dumps(dados, ensure_ascii=False)


@tool
async def consultar_indicadores_setor(cnae: Optional[str] = None, uf: Optional[str] = None) -> str:
    """
    Recupera distribuições estatísticas e indicadores econômicos de empresas concorrentes
    no mesmo setor (CNAE) ou região (UF), incluindo distribuição de portes e regimes tributários.
    """
    client = get_backend_client()
    dados = await client.get_indicadores_setor(cnae=cnae, uf=uf)
    return json.dumps(dados, ensure_ascii=False)


@tool
def construir_grafico_dashboard(
    tipo: Optional[str] = None,
    titulo: Optional[str] = None,
    labels: Optional[List[str]] = None,
    datasets: Optional[List[Dict[str, Any]]] = None,
    descricao: Optional[str] = None,
    chart_type: Optional[str] = None,
    title: Optional[str] = None,
    x_key: Optional[str] = None,
    y_keys: Optional[List[str]] = None,
    data: Optional[List[Dict[str, Any]]] = None,
    colors: Optional[List[str]] = None,
    description: Optional[str] = None,
) -> str:
    """
    Gera e valida a especificação JSON estrita de um gráfico para renderização direta na SPA React via Recharts.
    Suporta o formato canônico Recharts (chart_type, title, x_key, y_keys, data) e formato de séries (tipo, titulo, labels, datasets).
    """
    try:
        resolved_type = chart_type or tipo
        resolved_title = title or titulo
        resolved_desc = description or descricao

        if not resolved_type or not resolved_title:
            return json.dumps({
                "erro": "Parâmetros obrigatórios ausentes: informe 'tipo' (ou 'chart_type') e 'titulo' (ou 'title').",
                "valido": False
            }, ensure_ascii=False)

        chart_payload: Dict[str, Any] = {
            "chart_type": resolved_type,
            "title": resolved_title,
            "description": resolved_desc,
        }

        if data is not None:
            chart_payload["data"] = data
            chart_payload["x_key"] = x_key or "label"
            chart_payload["y_keys"] = y_keys or []
            if colors:
                chart_payload["colors"] = colors
        elif labels is not None and datasets is not None:
            chart_payload["labels"] = labels
            chart_payload["datasets"] = datasets
            if x_key:
                chart_payload["x_key"] = x_key
            if y_keys:
                chart_payload["y_keys"] = y_keys
            if colors:
                chart_payload["colors"] = colors
        else:
            return json.dumps({
                "erro": "Dados do gráfico ausentes: informe 'data' tabular ou 'labels' com 'datasets'.",
                "valido": False
            }, ensure_ascii=False)

        chart = ChartSpec.model_validate(chart_payload)
        record_collected_chart(chart)
        return json.dumps(chart.model_dump(), ensure_ascii=False)
    except Exception as exc:
        return json.dumps({
            "erro": f"Falha ao validar especificação do gráfico: {str(exc)}",
            "valido": False
        }, ensure_ascii=False)
