"""
Testes unitários para o catálogo de ferramentas do AI Gateway e cliente BackendClient.
"""

import json
import httpx
import pytest
from app.client import BackendClient
from app.config import Settings
from app.tools.fiscal_tools import buscar_dados_cadastrais, apurar_demonstrativo_dre
from app.tools.graph_tools import analisar_grafo_societario, consultar_indicadores_setor


@pytest.mark.asyncio
async def test_buscar_dados_cadastrais_valid_cnpj():
    """Valida busca cadastral com CNPJ formatado."""
    result = await buscar_dados_cadastrais.ainvoke({"cnpj": "00.000.000/0001-91"})
    data = json.loads(result)
    assert data["encontrado"] is True
    assert "razao_social" in data
    assert data["cnpj"] == "00000000000191"
    assert data["tipo"] == "MATRIZ"


@pytest.mark.asyncio
async def test_buscar_dados_cadastrais_filial_cnpj():
    """Valida identificação de filial."""
    result = await buscar_dados_cadastrais.ainvoke({"cnpj": "00.000.000/0002-72"})
    data = json.loads(result)
    assert data["encontrado"] is True
    assert data["tipo"] == "FILIAL"


@pytest.mark.asyncio
async def test_buscar_dados_cadastrais_invalid_cnpj():
    """Valida rejeição de CNPJ com tamanho inválido."""
    result = await buscar_dados_cadastrais.ainvoke({"cnpj": "12345"})
    data = json.loads(result)
    assert data.get("valido") is False
    assert "erro" in data


@pytest.mark.asyncio
async def test_apurar_demonstrativo_dre_success():
    """Valida recuperação de DRE e campos de receita contábil."""
    result = await apurar_demonstrativo_dre.ainvoke({"cnpj": "00000000000191", "ano": 2023})
    data = json.loads(result)
    assert data["cnpj"] == "00000000000191"
    assert data["ano"] == 2023
    assert "receita_bruta" in data
    assert data["receita_bruta"] > 0
    assert "receita_servico" in data
    assert "deducoes" in data


@pytest.mark.asyncio
async def test_apurar_demonstrativo_dre_invalid_cnpj():
    """Valida rejeição de CNPJ mal formatado na DRE."""
    result = await apurar_demonstrativo_dre.ainvoke({"cnpj": "cnpj_com_letras_incorreto"})
    data = json.loads(result)
    assert data.get("valido") is False
    assert "erro" in data


@pytest.mark.asyncio
async def test_apurar_demonstrativo_dre_with_nested_resultado_exercicio():
    """Valida extração correta de dados DRE quando o backend retorna ConsultaCnpjResponse com resultado aninhado."""
    def handler(request: httpx.Request):
        return httpx.Response(200, json={
            "cnpj": "11222333000199",
            "razao_social": "EMPRESA CONTABIL MODELO LTDA",
            "reg_j150": [{"cod_agl": "1.01", "descr_cod_agl": "Receita Vendas", "vl_cta": 5000000.0}],
            "resultado_exercicio": {
                "receita_bruta": 5000000.00,
                "receita_servico": 3000000.00,
                "outras_receitas": 200000.00,
                "receita_acumulada": 5200000.00,
                "total_nota": 4800000.00,
                "periodo_inicial": "2023-01-01",
                "periodo_final": "2023-12-31"
            }
        })

    transport = httpx.MockTransport(handler)
    mock_client = httpx.AsyncClient(transport=transport, base_url="http://testbackend")
    backend = BackendClient(settings=Settings(backend_url="http://testbackend"), client=mock_client)

    dre = await backend.get_demonstrativo_dre("11.222.333/0001-99", ano=2023)
    assert dre["encontrado"] is True
    assert dre["receita_bruta"] == 5000000.00
    assert dre["receita_servico"] == 3000000.00
    assert dre["razao_social"] == "EMPRESA CONTABIL MODELO LTDA"
    assert len(dre["reg_j150"]) == 1

    await backend.close()


@pytest.mark.asyncio
async def test_analisar_grafo_societario_success():
    """Valida análise da Rede CNPJ e extração de nós e arestas."""
    result = await analisar_grafo_societario.ainvoke({"cnpj_ou_cpf": "00000000000191", "camadas": 2})
    data = json.loads(result)
    assert data["raiz"] == "00000000000191"
    assert len(data["nodes"]) >= 2
    assert len(data["links"]) >= 1
    assert data["alertas_encontrados"] >= 0


@pytest.mark.asyncio
async def test_analisar_grafo_societario_invalid_id():
    """Valida identificador vazio."""
    result = await analisar_grafo_societario.ainvoke({"cnpj_ou_cpf": ""})
    data = json.loads(result)
    assert data.get("valido") is False
    assert "erro" in data


@pytest.mark.asyncio
async def test_consultar_indicadores_setor():
    """Valida retorno de agregações econômicas e regimes tributários."""
    result = await consultar_indicadores_setor.ainvoke({"cnae": "6201-5/01", "uf": "SP"})
    data = json.loads(result)
    assert "total_empresas" in data
    assert "porte" in data
    assert len(data["porte"]) > 0
    assert "regime_tributario" in data
    assert len(data["regime_tributario"]) > 0


@pytest.mark.asyncio
async def test_backend_client_live_http_200():
    """Valida BackendClient quando o Axum backend responde HTTP 200 real."""
    def handler(request: httpx.Request):
        if "consulta-cnpj" in str(request.url):
            return httpx.Response(200, json={
                "cnpj": "12345678000199",
                "razao_social": "EMPRESA REAL LTDA",
                "tipo": "MATRIZ",
                "encontrado": True,
            })
        if "rede" in str(request.url):
            return httpx.Response(200, json={
                "nodes": [{"id": "1", "label": "Real Node"}],
                "links": [],
                "alertas_encontrados": 0,
            })
        return httpx.Response(404, json={"detail": "Not found"})

    transport = httpx.MockTransport(handler)
    mock_client = httpx.AsyncClient(transport=transport, base_url="http://testbackend")
    backend = BackendClient(settings=Settings(backend_url="http://testbackend"), client=mock_client)

    cadastral = await backend.get_dados_cadastrais("12.345.678/0001-99")
    assert cadastral["razao_social"] == "EMPRESA REAL LTDA"
    assert cadastral["encontrado"] is True

    grafo = await backend.get_grafo_societario(raiz="12345678000199")
    assert len(grafo["nodes"]) == 1

    await backend.close()


@pytest.mark.asyncio
async def test_backend_client_http_404():
    """Valida tratamento quando o endpoint retorna erro HTTP (404/500)."""
    transport = httpx.MockTransport(lambda req: httpx.Response(404, text="Empresa não cadastrada"))
    mock_client = httpx.AsyncClient(transport=transport, base_url="http://testbackend")
    backend = BackendClient(settings=Settings(backend_url="http://testbackend"), client=mock_client)

    cadastral = await backend.get_dados_cadastrais("99999999000199")
    assert cadastral["encontrado"] is False
    assert "Status 404" in cadastral["erro"]

    await backend.close()
