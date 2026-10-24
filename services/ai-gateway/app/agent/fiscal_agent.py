"""
Agente ReAct com LangGraph para Auditoria Contábil, Fiscal e Inteligência Societária.
Orquestra chamadas a ferramentas contábeis, sintetiza relatórios em Markdown,
produz especificações de gráficos Recharts e suporta streaming SSE em tempo real.
"""

import json
from typing import Any, AsyncGenerator, Dict, List, Literal, Optional, Tuple
from langchain_core.language_models.chat_models import BaseChatModel
from langchain_core.messages import (
    AIMessage,
    BaseMessage,
    HumanMessage,
    SystemMessage,
    ToolMessage,
)
from langgraph.graph import StateGraph, MessagesState, START, END
from langgraph.prebuilt import ToolNode

from app.config import get_llm
from app.models import ChartSpec, ChatQuery, ChatResponse, DatasetSpec
from app.tools import (
    ALL_TOOLS,
    get_collected_charts,
    reset_collected_charts,
)

SYSTEM_PROMPT = """Você é o Agente Especialista em Auditoria Fiscal, Contábil e Inteligência Societária da plataforma ECD Analytics.
Sua missão é auxiliar auditores fiscais a analisar escriturações contábeis do SPED ECD (Blocos 0000, J100, J150 e I155),
apurar a evolução de Receita Bruta e margens contábeis, cruzar participações societárias na Rede CNPJ e apontar riscos de conformidade (PEP, CEIS, CNEP, CEPIM, PGFN).

Regras de Operação:
1. Use as ferramentas disponíveis quando precisar consultar dados reais:
   - `buscar_dados_cadastrais`: Consulta dados oficiais de empresas na Receita Federal (matriz/filial, razão social, CNAE, situação).
   - `apurar_demonstrativo_dre`: Obtém apuração de receitas brutas, de serviços e resultado da escrituração contábil.
   - `analisar_grafo_societario`: Percorre a rede societária e verifica vínculos com pessoas sancionadas ou expostas.
   - `consultar_indicadores_setor`: Consulta indicadores estatísticos e distribuição de empresas no mesmo CNAE/UF.
   - `construir_grafico_dashboard`: SEMPRE que houver séries temporais de receitas, comparação entre anos ou distribuição de portes, gere a especificação JSON estrita do gráfico para que o frontend React com Recharts renderize visualmente.
2. Seja preciso, técnico e fundamentado na legislação e boas práticas contábeis.
3. Destaque alertas e riscos de conformidade em uma seção destacada no seu parecer Markdown.
"""


def should_continue(state: MessagesState) -> Literal["tools", "__end__"]:
    """Determina se o agente precisa invocar ferramentas ou encerrar."""
    messages = state["messages"]
    last_message = messages[-1]
    if hasattr(last_message, "tool_calls") and last_message.tool_calls:
        return "tools"
    return "__end__"


def create_fiscal_graph(
    llm: Optional[BaseChatModel] = None,
    tools: Optional[List[Any]] = None
):
    """
    Constrói e compila o grafo ReAct com LangGraph.
    """
    ferramentas = tools or ALL_TOOLS
    model = llm or get_llm()

    if hasattr(model, "bind_tools"):
        model_with_tools = model.bind_tools(ferramentas)
    else:
        model_with_tools = model

    tool_node = ToolNode(ferramentas)

    async def call_model(state: MessagesState) -> Dict[str, List[BaseMessage]]:
        messages = list(state["messages"])
        # Garante a mensagem de sistema no início
        if not messages or not isinstance(messages[0], SystemMessage):
            messages.insert(0, SystemMessage(content=SYSTEM_PROMPT))
        
        response = await model_with_tools.ainvoke(messages)
        return {"messages": [response]}

    workflow = StateGraph(MessagesState)
    workflow.add_node("agent", call_model)
    workflow.add_node("tools", tool_node)

    workflow.add_edge(START, "agent")
    workflow.add_conditional_edges(
        "agent",
        should_continue,
        {"tools": "tools", "__end__": END}
    )
    workflow.add_edge("tools", "agent")

    return workflow.compile()


def _extrair_alertas_e_graficos(
    messages: List[BaseMessage],
    charts_coletados: List[ChartSpec]
) -> Tuple[str, List[str], List[ChartSpec], List[str]]:
    """Extrai texto Markdown final, alertas identificados, lista de gráficos deduplicados e ferramentas executadas."""
    markdown_report = ""
    alertas: List[str] = []
    graficos_finais: List[ChartSpec] = list(charts_coletados)
    ferramentas_executadas: List[str] = []

    def _chart_title(c: Any) -> str:
        if isinstance(c, ChartSpec):
            return c.title or c.titulo or ""
        if isinstance(c, dict):
            return c.get("title") or c.get("titulo") or ""
        return ""

    # Itera pelas mensagens para identificar conteúdo final e mensagens de ferramentas
    for m in messages:
        if isinstance(m, AIMessage):
            if getattr(m, "tool_calls", None):
                for tc in m.tool_calls:
                    t_name = tc.get("name")
                    if t_name and t_name not in ferramentas_executadas:
                        ferramentas_executadas.append(t_name)
            if m.content and isinstance(m.content, str):
                markdown_report = m.content

        elif isinstance(m, ToolMessage):
            t_name = getattr(m, "name", None)
            if t_name and t_name not in ferramentas_executadas:
                ferramentas_executadas.append(t_name)

            try:
                data = json.loads(str(m.content))
                if isinstance(data, dict):
                    # Checa alertas em grafos societários
                    if data.get("alertas_encontrados", 0) > 0:
                        msg_alerta = f"Rede Societária: Identificados {data['alertas_encontrados']} alertas sancionatórios (PEP/CEIS/CNEP)."
                        if msg_alerta not in alertas:
                            alertas.append(msg_alerta)

                    for node in data.get("nodes", []):
                        flags = node.get("flags_sancionatorias", [])
                        if flags:
                            alerta_item = f"Vínculo Sancionado: {node.get('label', node.get('id'))} possui flag(s) {', '.join(flags)}."
                            if alerta_item not in alertas:
                                alertas.append(alerta_item)

                    # Se foi chamada de gráfico via ToolMessage
                    has_chart_fields = (
                        ("chart_type" in data or "tipo" in data)
                        and ("title" in data or "titulo" in data)
                        and ("data" in data or "labels" in data)
                    )
                    if has_chart_fields:
                        try:
                            chart = ChartSpec.model_validate(data)
                            c_title = _chart_title(chart)
                            if not any(_chart_title(g) == c_title for g in graficos_finais):
                                graficos_finais.append(chart)
                        except Exception:
                            pass
            except Exception:
                pass

    # Tenta extrair blocos JSON de gráficos embutidos no Markdown caso a ferramenta não tenha sido chamada
    if "```json" in markdown_report:
        try:
            chunks = markdown_report.split("```json")
            for chunk in chunks[1:]:
                json_str = chunk.split("```")[0].strip()
                parsed = json.loads(json_str)
                if isinstance(parsed, dict):
                    has_chart_fields = (
                        ("chart_type" in parsed or "tipo" in parsed)
                        and ("title" in parsed or "titulo" in parsed)
                        and ("data" in parsed or "labels" in parsed)
                    )
                    if has_chart_fields:
                        chart = ChartSpec.model_validate(parsed)
                        c_title = _chart_title(chart)
                        if not any(_chart_title(g) == c_title for g in graficos_finais):
                            graficos_finais.append(chart)
        except Exception:
            pass

    # Se nenhum alerta foi levantado mas o texto menciona PEP/CEIS, adiciona nota
    if not alertas and any(w in markdown_report.lower() for w in ["pep", "ceis", "cnep", "sancion"]):
        alertas.append("Possível apontamento sancionatório identificado na fundamentação técnica.")

    if not markdown_report:
        markdown_report = "### Análise Fiscal Concluída\nNenhum detalhe adicional reportado pelo agente."

    return markdown_report, alertas, graficos_finais, ferramentas_executadas


async def run_agent(
    query: ChatQuery,
    llm: Optional[BaseChatModel] = None,
    tools: Optional[List[Any]] = None
) -> ChatResponse:
    """
    Executa o agente fiscal de ponta a ponta e retorna o ChatResponse estruturado.
    """
    reset_collected_charts()
    graph = create_fiscal_graph(llm=llm, tools=tools)

    user_prompt = query.message or ""
    if query.cnpj_context:
        user_prompt = f"[Contexto CNPJ: {query.cnpj_context}]\n{user_prompt}"

    initial_messages = [HumanMessage(content=user_prompt)]
    result = await graph.ainvoke({"messages": initial_messages})

    collected = get_collected_charts()
    markdown_report, alertas, graficos, ferramentas = _extrair_alertas_e_graficos(
        result["messages"],
        collected
    )

    return ChatResponse(
        relatorio_markdown=markdown_report,
        mensagem=markdown_report,
        alertas=alertas,
        graficos=graficos,
        ferramentas_executadas=ferramentas,
        conversation_id=query.conversation_id,
    )


async def run_agent_stream(
    query: ChatQuery,
    llm: Optional[BaseChatModel] = None,
    tools: Optional[List[Any]] = None
) -> AsyncGenerator[str, None]:
    """
    Executa o agente fiscal com transmissão SSE (Server-Sent Events) em tempo real.
    Gera eventos de thinking, tool_call, chart, token e o evento final com o ChatResponse.
    """
    reset_collected_charts()
    graph = create_fiscal_graph(llm=llm, tools=tools)

    user_prompt = query.message or ""
    if query.cnpj_context:
        user_prompt = f"[Contexto CNPJ: {query.cnpj_context}]\n{user_prompt}"

    initial_messages = [HumanMessage(content=user_prompt)]

    # Evento inicial SSE
    yield f"event: start\ndata: {json.dumps({'status': 'iniciado', 'conversation_id': query.conversation_id}, ensure_ascii=False)}\n\n"

    all_messages: List[BaseMessage] = list(initial_messages)

    try:
        async for update in graph.astream({"messages": initial_messages}, stream_mode="updates"):
            for node_name, node_output in update.items():
                messages = node_output.get("messages", [])
                for msg in messages:
                    all_messages.append(msg)
                    if isinstance(msg, AIMessage):
                        if getattr(msg, "tool_calls", None):
                            for tc in msg.tool_calls:
                                yield f"event: thinking\ndata: {json.dumps({'acao': 'executando_ferramenta', 'ferramenta': tc.get('name'), 'parametros': tc.get('args')}, ensure_ascii=False)}\n\n"
                        elif msg.content:
                            yield f"event: token\ndata: {json.dumps({'content': msg.content}, ensure_ascii=False)}\n\n"
                    elif isinstance(msg, ToolMessage):
                        yield f"event: tool_result\ndata: {json.dumps({'ferramenta': getattr(msg, 'name', 'ferramenta'), 'status': 'sucesso'}, ensure_ascii=False)}\n\n"
                        # Checa se gerou gráfico
                        try:
                            parsed_tool_data = json.loads(str(msg.content))
                            if isinstance(parsed_tool_data, dict) and ("chart_type" in parsed_tool_data or "tipo" in parsed_tool_data):
                                yield f"event: chart\ndata: {json.dumps(parsed_tool_data, ensure_ascii=False)}\n\n"
                        except Exception:
                            pass

        collected = get_collected_charts()
        markdown_report, alertas, graficos, ferramentas = _extrair_alertas_e_graficos(all_messages, collected)

        chat_response = ChatResponse(
            relatorio_markdown=markdown_report,
            mensagem=markdown_report,
            alertas=alertas,
            graficos=graficos,
            ferramentas_executadas=ferramentas,
            conversation_id=query.conversation_id,
        )

        yield f"event: final\ndata: {chat_response.model_dump_json()}\n\n"
        yield f"event: done\ndata: {json.dumps({'status': 'concluido'}, ensure_ascii=False)}\n\n"

    except Exception as exc:
        err_payload = json.dumps({"erro": f"Erro durante execução do agente: {str(exc)}"}, ensure_ascii=False)
        yield f"event: error\ndata: {err_payload}\n\n"
