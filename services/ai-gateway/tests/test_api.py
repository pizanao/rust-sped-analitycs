"""
Testes de integração para as rotas da API FastAPI do AI Gateway.
"""

from fastapi.testclient import TestClient


def test_health_endpoint(client: TestClient):
    """Garante que o endpoint de health check responda 200 OK com metadados."""
    response = client.get("/health")
    assert response.status_code == 200
    data = response.json()
    assert data["status"] == "ok"
    assert data["service"] == "ai-gateway"
    assert "llm_provider" in data


def test_chat_analitico_endpoint(client: TestClient):
    """Garante que a rota POST /api/ai/chat responda com o ChatResponse completo."""
    payload = {
        "message": "Qual é a situação cadastral e faturamento da empresa?",
        "cnpj_context": "00000000000191",
        "conversation_id": "conv-api-test"
    }
    response = client.post("/api/ai/chat", json=payload)
    assert response.status_code == 200
    data = response.json()
    assert "relatorio_markdown" in data
    assert len(data["relatorio_markdown"]) > 0
    assert "mensagem" in data
    assert data["mensagem"] == data["relatorio_markdown"]
    assert "alertas" in data
    assert "graficos" in data
    assert data["conversation_id"] == "conv-api-test"


def test_chat_analitico_frontend_payload(client: TestClient):
    """Garante que a rota aceite o contrato exato enviado pela SPA React ('mensagem' e 'contexto')."""
    payload = {
        "mensagem": "Gere um gráfico comparativo de faturamento",
        "contexto": {"cnpj": "00000000000191"},
        "conversation_id": "conv-frontend-test"
    }
    response = client.post("/api/ai/chat", json=payload)
    assert response.status_code == 200
    data = response.json()
    assert "mensagem" in data
    assert len(data["graficos"]) >= 1
    assert data["chart"] is not None
    assert "chart_type" in data["chart"]


def test_chat_analitico_empty_message_422(client: TestClient):
    """Garante que mensagens vazias resultem em erro de validação 422."""
    payload = {"message": "   "}
    response = client.post("/api/ai/chat", json=payload)
    assert response.status_code == 422


def test_chat_analitico_route_alias(client: TestClient):
    """Valida o alias de rota /ai/chat sem o prefixo /api."""
    payload = {
        "mensagem": "Consulta geral",
        "conversation_id": "conv-alias-test"
    }
    response = client.post("/ai/chat", json=payload)
    assert response.status_code == 200
    assert "mensagem" in response.json()


def test_chat_analitico_stream_post_endpoint(client: TestClient):
    """Valida o endpoint POST SSE /api/ai/chat/stream."""
    payload = {
        "message": "Gere uma análise com gráfico de receita.",
        "cnpj_context": "00000000000191",
        "conversation_id": "conv-stream-post"
    }
    response = client.post("/api/ai/chat/stream", json=payload)
    assert response.status_code == 200
    assert "text/event-stream" in response.headers.get("content-type", "")

    # Checa eventos transmitidos
    content = response.text
    assert "event: start" in content
    assert "event: final" in content
    assert "event: done" in content


def test_chat_analitico_stream_get_endpoint(client: TestClient):
    """Valida o endpoint GET SSE /api/ai/chat/stream para clientes EventSource."""
    params = {
        "message": "Quais são os sócios e coligadas?",
        "cnpj_context": "00000000000191",
        "conversation_id": "conv-stream-get"
    }
    response = client.get("/api/ai/chat/stream", params=params)
    assert response.status_code == 200
    assert "text/event-stream" in response.headers.get("content-type", "")
    content = response.text
    assert "event: start" in content
    assert "event: done" in content


def test_chat_analitico_stream_get_with_mensagem_param(client: TestClient):
    """Valida o endpoint GET SSE com parâmetro em português '?mensagem='."""
    params = {
        "mensagem": "Consulta streaming com alias",
        "conversation_id": "conv-stream-get-pt"
    }
    response = client.get("/api/ai/chat/stream", params=params)
    assert response.status_code == 200
    assert "event: start" in response.text
