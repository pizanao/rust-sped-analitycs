"""
Modelo LLM simulado para testes unitários e execução offline sem chaves de API.
Suporta chamadas ReAct e Tool Calling gerando contratos Recharts completos.
"""

from typing import Any, List, Optional
import json
from langchain_core.language_models.chat_models import BaseChatModel
from langchain_core.messages import AIMessage, BaseMessage, HumanMessage, ToolMessage
from langchain_core.outputs import ChatGeneration, ChatResult


class MockFiscalChatModel(BaseChatModel):
    """
    LLM simulado que inspeciona o histórico de mensagens e responde de forma
    determinística com relatórios fiscais ou invocação de ferramentas.
    """
    bound_tools: list = []

    def _generate(
        self,
        messages: List[BaseMessage],
        stop: Optional[List[str]] = None,
        **kwargs: Any
    ) -> ChatResult:
        # Se a última mensagem for ToolMessage, sintetiza a resposta final
        has_tool_response = any(isinstance(m, ToolMessage) for m in messages)

        if has_tool_response:
            # Resposta após execução de ferramentas
            relatorio = (
                "### Relatório de Auditoria Contábil & Fiscal\n\n"
                "A análise automatizada das escriturações contábeis e da Rede CNPJ identificou:\n"
                "- **Receita Bruta Apurada**: R$ 12.500.000,00\n"
                "- **Situação Cadastral**: ATIVA\n"
                "- **Indicadores Setoriais**: Faturamento alinhado à média das empresas do mesmo CNAE.\n"
                "- **Alertas de Conformidade**: Identificados vínculos com pessoas expostas politicamente (PEP).\n\n"
                "Os demonstrativos foram consolidados e as séries temporais estão disponíveis no dashboard."
            )
            return ChatResult(generations=[ChatGeneration(message=AIMessage(content=relatorio))])

        # Se o usuário perguntou sobre gráficos, receita ou DRE, chama a ferramenta construir_grafico_dashboard
        user_text = ""
        for m in reversed(messages):
            if isinstance(m, HumanMessage):
                user_text = str(m.content).lower()
                break

        if any(w in user_text for w in ["gráfico", "grafico", "dashboard", "receita", "faturamento", "dre", "comparativo"]):
            tool_call = {
                "name": "construir_grafico_dashboard",
                "args": {
                    "chart_type": "bar",
                    "title": "Evolução da Receita Bruta (2021-2023)",
                    "x_key": "periodo",
                    "y_keys": ["Receita Bruta (R$)"],
                    "data": [
                        {"periodo": "2021", "Receita Bruta (R$)": 9500000.0},
                        {"periodo": "2022", "Receita Bruta (R$)": 11200000.0},
                        {"periodo": "2023", "Receita Bruta (R$)": 12500000.0},
                    ],
                    "labels": ["2021", "2022", "2023"],
                    "datasets": [
                        {
                            "label": "Receita Bruta (R$)",
                            "data": [9500000.0, 11200000.0, 12500000.0],
                            "backgroundColor": "#3b82f6"
                        }
                    ],
                    "description": "Crescimento contínuo apurado no Registro J150 da ECD."
                },
                "id": "call_mock_chart_01",
                "type": "tool_call"
            }
            return ChatResult(generations=[ChatGeneration(
                message=AIMessage(content="", tool_calls=[tool_call])
            )])

        if any(w in user_text for w in ["socio", "sócio", "societario", "societário", "rede", "grc", "sanções"]):
            tool_call = {
                "name": "analisar_grafo_societario",
                "args": {"cnpj_ou_cpf": "00000000000191", "camadas": 2},
                "id": "call_mock_graph_01",
                "type": "tool_call"
            }
            return ChatResult(generations=[ChatGeneration(
                message=AIMessage(content="", tool_calls=[tool_call])
            )])

        # Resposta padrão analítica direta
        resposta = (
            "### Análise Fiscal e Contábil\n\n"
            "Consulta processada pelo Copiloto de IA da plataforma Rust Sped Analytics.\n"
            "O ambiente está operacional e pronto para inspecionar blocos SPED (0000, J100, J150 e I155) e redes de relacionamento."
        )
        return ChatResult(generations=[ChatGeneration(message=AIMessage(content=resposta))])

    async def _agenerate(
        self,
        messages: List[BaseMessage],
        stop: Optional[List[str]] = None,
        **kwargs: Any
    ) -> ChatResult:
        return self._generate(messages, stop, **kwargs)

    @property
    def _llm_type(self) -> str:
        return "mock-fiscal"

    def bind_tools(self, tools: Any, **kwargs: Any) -> Any:
        self.bound_tools = tools
        return self
