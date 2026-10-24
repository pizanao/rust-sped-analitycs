"""
Testes de contrato e validação estrita de esquemas JSON de gráficos (ChartSpec).
Garante compatibilidade total com o componente DynamicChartRenderer do frontend React SPA,
formato canônico Recharts e isolamento de concorrência por ContextVar.
"""

import asyncio
import json
import pytest
from pydantic import ValidationError

from app.agent.fiscal_agent import run_agent, run_agent_stream
from app.models import ChartSpec, ChatQuery, ChatResponse, DatasetSpec
from app.tools.graph_tools import (
    construir_grafico_dashboard,
    reset_collected_charts,
    get_collected_charts,
)


def test_chart_spec_valid():
    """Valida a construção direta de um ChartSpec com formato de séries (Chart.js / legada)."""
    chart = ChartSpec(
        tipo="bar",
        titulo="Evolução de Faturamento",
        labels=["2021", "2022", "2023"],
        datasets=[
            DatasetSpec(
                label="Receita Bruta",
                data=[1000000.0, 1500000.0, 2000000.0],
                backgroundColor="#10b981",
                borderColor="#059669"
            )
        ],
        descricao="Crescimento anual consistente."
    )
    dumped = chart.model_dump()
    assert dumped["tipo"] == "bar"
    assert dumped["chart_type"] == "bar"
    assert dumped["titulo"] == "Evolução de Faturamento"
    assert dumped["title"] == "Evolução de Faturamento"
    assert len(dumped["labels"]) == 3
    assert len(dumped["datasets"]) == 1
    assert dumped["datasets"][0]["data"] == [1000000.0, 1500000.0, 2000000.0]

    # Valida que o array tabular Recharts (data) foi gerado automaticamente
    assert "data" in dumped
    assert len(dumped["data"]) == 3
    assert dumped["data"][0]["Receita Bruta"] == 1000000.0
    assert dumped["x_key"] == "label"
    assert dumped["y_keys"] == ["Receita Bruta"]


def test_chart_spec_recharts_native_schema():
    """Valida a construção direta de um ChartSpec com o formato canônico Recharts."""
    chart = ChartSpec(
        chart_type="area",
        title="Receita Acumulada",
        description="Evolução semestral",
        x_key="semestre",
        y_keys=["receita_total"],
        colors=["#2563eb"],
        data=[
            {"semestre": "2023-1", "receita_total": 4500000.0},
            {"semestre": "2023-2", "receita_total": 6200000.0},
        ]
    )
    assert chart.chart_type == "area"
    assert chart.tipo == "area"
    assert chart.title == "Receita Acumulada"
    assert chart.titulo == "Receita Acumulada"
    assert chart.x_key == "semestre"
    assert chart.y_keys == ["receita_total"]
    assert len(chart.data) == 2
    assert chart.labels == ["2023-1", "2023-2"]
    assert len(chart.datasets) == 1
    assert chart.datasets[0].data == [4500000.0, 6200000.0]


def test_chart_spec_bidirectional_conversion():
    """Valida sincronização bidirecional entre data tabular e series datasets."""
    # Origem: Recharts data
    chart1 = ChartSpec(
        chart_type="line",
        title="Comparativo",
        x_key="ano",
        y_keys=["a", "b"],
        data=[
            {"ano": "2021", "a": 10.0, "b": 20.0},
            {"ano": "2022", "a": 15.0, "b": 25.0},
        ]
    )
    assert chart1.labels == ["2021", "2022"]
    assert len(chart1.datasets) == 2
    assert chart1.datasets[0].label == "a"
    assert chart1.datasets[0].data == [10.0, 15.0]

    # Origem: labels + datasets
    chart2 = ChartSpec(
        tipo="bar",
        titulo="Inverso",
        labels=["X", "Y"],
        datasets=[DatasetSpec(label="valor", data=[100.0, 200.0])]
    )
    assert chart2.x_key == "label"
    assert chart2.y_keys == ["valor"]
    assert len(chart2.data) == 2
    assert chart2.data[0]["label"] == "X"
    assert chart2.data[0]["valor"] == 100.0


@pytest.mark.parametrize("tipo", ["bar", "line", "pie", "area"])
def test_chart_spec_all_supported_types(tipo):
    """Valida os 4 tipos de gráficos aceitos pelo DynamicChartRenderer."""
    chart = ChartSpec(
        tipo=tipo,
        titulo=f"Gráfico do tipo {tipo}",
        labels=["A", "B"],
        datasets=[DatasetSpec(label="Série", data=[10.5, 20.0])]
    )
    assert chart.tipo == tipo
    assert chart.chart_type == tipo


def test_chart_spec_invalid_tipo():
    """Garante que tipos não suportados pelo DynamicChartRenderer sejam rejeitados."""
    with pytest.raises(ValidationError):
        ChartSpec(
            tipo="scatter_3d_invalid",  # Não suportado
            titulo="Gráfico Inválido",
            labels=["A"],
            datasets=[DatasetSpec(label="X", data=[1.0])]
        )


def test_chart_spec_negative_and_zero_values():
    """Valida gráficos com prejuízos contábeis (valores negativos) e valores zerados."""
    chart = ChartSpec(
        tipo="bar",
        titulo="Resultado Líquido do Exercício",
        labels=["1º Tri", "2º Tri", "3º Tri", "4º Tri"],
        datasets=[
            DatasetSpec(
                label="Lucro / Prejuízo",
                data=[-450000.00, 0.0, 120000.50, 890000.00],
                backgroundColor="#ef4444"
            )
        ],
        descricao="Recuperação no 2º semestre."
    )
    assert chart.datasets[0].data[0] == -450000.00
    assert chart.datasets[0].data[1] == 0.0
    assert chart.data[0]["Lucro / Prejuízo"] == -450000.00
    assert chart.data[1]["Lucro / Prejuízo"] == 0.0


def test_chart_spec_multiple_datasets():
    """Valida múltiplos datasets (ex: Receita vs Custo vs Lucro)."""
    chart = ChartSpec(
        tipo="line",
        titulo="DRE Trienal",
        labels=["2021", "2022", "2023"],
        datasets=[
            DatasetSpec(label="Receita Bruta", data=[1000.0, 1200.0, 1500.0]),
            DatasetSpec(label="Custos", data=[600.0, 700.0, 850.0]),
            DatasetSpec(label="Lucro", data=[400.0, 500.0, 650.0]),
        ]
    )
    assert len(chart.datasets) == 3
    assert chart.datasets[2].label == "Lucro"
    assert len(chart.data) == 3
    assert chart.data[0]["Custos"] == 600.0
    assert chart.data[0]["Lucro"] == 400.0


def test_tool_construir_grafico_dashboard_success():
    """Valida a invocação da ferramenta construir_grafico_dashboard com parâmetros legados."""
    reset_collected_charts()
    result_str = construir_grafico_dashboard.invoke({
        "tipo": "line",
        "titulo": "Receita vs Despesas",
        "labels": ["Jan", "Fev", "Mar"],
        "datasets": [
            {"label": "Receita", "data": [100.0, 120.0, 130.0], "borderColor": "#3b82f6"},
            {"label": "Despesas", "data": [80.0, 90.0, 85.0], "borderColor": "#ef4444"},
        ],
        "descricao": "Demonstrativo trimestral."
    })
    
    parsed = json.loads(result_str)
    assert parsed["tipo"] == "line"
    assert parsed["chart_type"] == "line"
    assert parsed["titulo"] == "Receita vs Despesas"
    assert parsed["title"] == "Receita vs Despesas"
    assert len(parsed["datasets"]) == 2
    assert parsed["datasets"][0]["data"] == [100.0, 120.0, 130.0]
    assert len(parsed["data"]) == 3

    # Verifica se o gráfico foi coletado no contexto
    collected = get_collected_charts()
    assert len(collected) == 1
    assert collected[0].titulo == "Receita vs Despesas"


def test_tool_construir_grafico_dashboard_recharts_native():
    """Valida invocação da ferramenta construir_grafico_dashboard com formato Recharts nativo."""
    reset_collected_charts()
    result_str = construir_grafico_dashboard.invoke({
        "chart_type": "bar",
        "title": "Faturamento por Trimestre",
        "x_key": "trimestre",
        "y_keys": ["faturamento"],
        "data": [
            {"trimestre": "T1", "faturamento": 50000.0},
            {"trimestre": "T2", "faturamento": 75000.0},
        ],
        "description": "Dados apurados"
    })
    parsed = json.loads(result_str)
    assert parsed["chart_type"] == "bar"
    assert parsed["title"] == "Faturamento por Trimestre"
    assert len(parsed["data"]) == 2
    assert parsed["data"][0]["trimestre"] == "T1"
    assert parsed["labels"] == ["T1", "T2"]


def test_tool_construir_grafico_dashboard_invalid():
    """Valida tratamento de erro na ferramenta ao passar tipos incompatíveis."""
    result_str = construir_grafico_dashboard.invoke({
        "tipo": "tipo_invalido",
        "titulo": "Erro",
        "labels": ["A"],
        "datasets": [{"label": "Série", "data": ["valor_nao_numerico"]}]
    })
    parsed = json.loads(result_str)
    assert parsed.get("valido") is False
    assert "erro" in parsed


def test_chat_query_portuguese_and_context_dict():
    """Valida que ChatQuery aceita payload com chaves em português ('mensagem' e 'contexto')."""
    query = ChatQuery(
        mensagem="Auditar escrituração do exercício",
        contexto={"cnpj": "00.000.000/0001-91"}
    )
    assert query.message == "Auditar escrituração do exercício"
    assert query.mensagem == "Auditar escrituração do exercício"
    assert query.cnpj_context == "00.000.000/0001-91"


def test_chat_query_empty_rejection():
    """Valida que ChatQuery rejeita mensagens vazias ou apenas espaços."""
    with pytest.raises(ValidationError):
        ChatQuery(message="   ")


def test_chat_response_syncs_mensagem_and_chart():
    """Valida que ChatResponse sincroniza 'mensagem' e 'chart' para compatibilidade com o frontend."""
    chart = ChartSpec(
        chart_type="bar",
        title="DRE Anual",
        labels=["2022", "2023"],
        datasets=[DatasetSpec(label="Receita", data=[100.0, 200.0])]
    )
    resp = ChatResponse(
        relatorio_markdown="### Parecer Concluído",
        graficos=[chart]
    )
    assert resp.mensagem == "### Parecer Concluído"
    assert resp.chart is not None
    assert resp.chart.title == "DRE Anual"
    assert resp.chart.chart_type == "bar"


@pytest.mark.asyncio
async def test_agent_generates_valid_chat_response_schema():
    """
    Testa a execução do agente fiscal verificando o contrato do ChatResponse
    e garantindo que a resposta contenha Markdown e gráficos em conformidade com o schema.
    """
    query = ChatQuery(
        message="Apresente um gráfico comparativo da receita bruta nos últimos 3 anos.",
        cnpj_context="00000000000191",
        conversation_id="conv-test-123"
    )
    response = await run_agent(query)

    assert isinstance(response, ChatResponse)
    assert response.conversation_id == "conv-test-123"
    assert len(response.relatorio_markdown) > 0
    assert response.mensagem == response.relatorio_markdown
    assert "###" in response.relatorio_markdown  # Formatação Markdown
    
    # Valida que gerou pelo menos um gráfico e que ele obedece ao ChartSpec Recharts
    assert len(response.graficos) >= 1
    assert response.chart is not None
    for chart in response.graficos:
        assert isinstance(chart, ChartSpec)
        assert chart.chart_type in ["bar", "line", "pie", "area"]
        assert len(chart.data) > 0
        assert chart.x_key != ""
        assert len(chart.y_keys) > 0


@pytest.mark.asyncio
async def test_agent_stream_generates_sse_events():
    """Valida a emissão completa de eventos SSE pelo run_agent_stream."""
    query = ChatQuery(
        message="Quero o faturamento com gráfico e análise de risco.",
        cnpj_context="00000000000191",
        conversation_id="conv-sse-test"
    )
    events = []
    async for chunk in run_agent_stream(query):
        events.append(chunk)

    joined = "".join(events)
    assert "event: start" in joined
    assert "event: final" in joined
    assert "event: done" in joined

    # Procura o evento final e valida JSON do ChatResponse
    for line in joined.split("\n\n"):
        if line.startswith("event: final"):
            data_line = [l for l in line.split("\n") if l.startswith("data: ")][0]
            raw_json = data_line[len("data: "):]
            parsed_resp = ChatResponse.model_validate_json(raw_json)
            assert isinstance(parsed_resp, ChatResponse)
            assert len(parsed_resp.relatorio_markdown) > 0
            assert parsed_resp.mensagem == parsed_resp.relatorio_markdown


@pytest.mark.asyncio
async def test_agent_corporate_graph_alerts():
    """Valida que o agente levanta alertas sancionatórios quando questionado sobre sócios/rede."""
    query = ChatQuery(
        message="Analise a rede societária e sanções desta empresa.",
        cnpj_context="00000000000191",
        conversation_id="conv-grc-test"
    )
    response = await run_agent(query)
    assert len(response.alertas) >= 1
    assert any("PEP" in a or "CEIS" in a or "Societária" in a or "Vínculo" in a for a in response.alertas)
    assert "analisar_grafo_societario" in response.ferramentas_executadas


@pytest.mark.asyncio
async def test_concurrent_agent_execution():
    """
    Testa execução assíncrona concorrente garantindo isolamento por ContextVar.
    Duas requisições simultâneas não devem sofrer interferência ou perda de gráficos.
    """
    query1 = ChatQuery(
        message="Gere um gráfico de receita",
        conversation_id="conv-concurrent-1"
    )
    query2 = ChatQuery(
        message="Gere um gráfico de faturamento",
        conversation_id="conv-concurrent-2"
    )

    resp1, resp2 = await asyncio.gather(
        run_agent(query1),
        run_agent(query2)
    )

    assert resp1.conversation_id == "conv-concurrent-1"
    assert resp2.conversation_id == "conv-concurrent-2"
    assert len(resp1.graficos) >= 1
    assert len(resp2.graficos) >= 1
