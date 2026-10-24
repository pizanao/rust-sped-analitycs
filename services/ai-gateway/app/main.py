"""
AI Gateway com LangChain & LangGraph para ECD Analytics.
Fornece agentes de IA com Tool Calling sobre o backend Rust e geração de dashboards em linguagem natural.
"""

from fastapi import FastAPI
from pydantic import BaseModel
from typing import List, Optional, Any

app = FastAPI(
    title="ECD Analytics AI Gateway",
    version="0.1.0",
    description="Gateway de Agentes de IA com LangChain e ferramentas de auditoria contábil e fiscal."
)

class ChatQuery(BaseModel):
    message: str
    cnpj_context: Optional[str] = None
    conversation_id: Optional[str] = None

class ChartSpec(BaseModel):
    tipo: str
    titulo: str
    labels: List[str]
    datasets: List[Any]

class ChatResponse(BaseModel):
    relatorio_markdown: str
    alertas: List[str] = []
    graficos: List[ChartSpec] = []

@app.get("/health")
def health():
    return {"status": "ok", "service": "ai-gateway", "framework": "LangChain/LangGraph"}

@app.post("/api/ai/chat", response_model=ChatResponse)
async def chat_analitico(query: ChatQuery):
    # Stub inicial que será conectado com os nós do LangGraph no plano de implementação
    return ChatResponse(
        relatorio_markdown=f"### Análise Fiscal Automática\nRecebido comando: *{query.message}*.\nOs agentes estão prontos para processar a consulta utilizando ferramentas fiscais.",
        alertas=["Verificação inicial executada com sucesso."],
        graficos=[]
    )
