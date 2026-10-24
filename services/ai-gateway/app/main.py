"""
AI Gateway com LangChain & LangGraph para Rust Sped Analytics.
Fornece agentes de IA com Tool Calling sobre o backend Rust e geração de dashboards Recharts em tempo real.
"""

from contextlib import asynccontextmanager
from typing import Optional
from fastapi import FastAPI, Query
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse

from app.agent.fiscal_agent import run_agent, run_agent_stream
from app.client import get_backend_client
from app.config import get_settings
from app.models import ChartSpec, ChatQuery, ChatResponse, DatasetSpec


@asynccontextmanager
async def lifespan(app: FastAPI):
    # Inicialização de recursos
    client = get_backend_client()
    yield
    # Finalização de conexões HTTP
    await client.close()


settings = get_settings()

app = FastAPI(
    title="Rust Sped Analytics AI Gateway",
    version="0.1.0",
    description="Gateway de Agentes de IA com LangChain e ferramentas de auditoria contábil, fiscal e societária.",
    lifespan=lifespan,
)

# Configuração de CORS para permitir acesso direto do frontend React
app.add_middleware(
    CORSMiddleware,
    allow_origins=settings.cors_origins,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.get("/health")
def health():
    return {
        "status": "ok",
        "service": "ai-gateway",
        "framework": "LangChain/LangGraph",
        "llm_provider": settings.llm_provider,
        "backend_url": settings.backend_url,
    }


@app.post("/api/ai/chat", response_model=ChatResponse)
@app.post("/ai/chat", response_model=ChatResponse, include_in_schema=False)
async def chat_analitico(query: ChatQuery):
    """
    Executa raciocínio ReAct do Agente Fiscal com Tool Calling e retorna
    o relatório Markdown consolidado, alertas e especificações de gráficos Recharts.
    """
    return await run_agent(query)


@app.post("/api/ai/chat/stream")
@app.post("/ai/chat/stream", include_in_schema=False)
async def chat_analitico_stream_post(query: ChatQuery):
    """
    Endpoint de streaming SSE (Server-Sent Events) via POST para o Copiloto de IA.
    Transmite tokens, chamadas de ferramentas e payload de gráficos em tempo real.
    """
    return StreamingResponse(
        run_agent_stream(query),
        media_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "Connection": "keep-alive",
            "X-Accel-Buffering": "no",
        }
    )


@app.get("/api/ai/chat/stream")
@app.get("/ai/chat/stream", include_in_schema=False)
async def chat_analitico_stream_get(
    message: Optional[str] = Query(None, description="Pergunta ou instrução do auditor"),
    mensagem: Optional[str] = Query(None, description="Pergunta ou instrução do auditor (alias)"),
    cnpj_context: Optional[str] = Query(None, description="CNPJ sob contexto"),
    conversation_id: Optional[str] = Query(None, description="ID da conversa")
):
    """
    Endpoint SSE via GET para conexão facilitada com o cliente EventSource padrão dos navegadores.
    """
    resolved_msg = message or mensagem or ""
    query = ChatQuery(
        message=resolved_msg,
        cnpj_context=cnpj_context,
        conversation_id=conversation_id
    )
    return StreamingResponse(
        run_agent_stream(query),
        media_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "Connection": "keep-alive",
            "X-Accel-Buffering": "no",
        }
    )
