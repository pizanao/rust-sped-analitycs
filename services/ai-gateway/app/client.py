"""
Cliente HTTP assíncrono para comunicação com a API do backend Rust (Axum).
Fornece métodos tipados e resiliência contra falhas de conexão.
"""

from typing import Any, Dict, Optional
import httpx
from app.config import Settings, get_settings


class BackendClient:
    def __init__(self, settings: Optional[Settings] = None, client: Optional[httpx.AsyncClient] = None):
        self.settings = settings or get_settings()
        self._external_client = client
        self._internal_client: Optional[httpx.AsyncClient] = None

    async def get_client(self) -> httpx.AsyncClient:
        if self._external_client is not None:
            return self._external_client
        if self._internal_client is None or self._internal_client.is_closed:
            headers = {"Accept": "application/json"}
            if self.settings.backend_api_key:
                headers["Authorization"] = f"Bearer {self.settings.backend_api_key}"
            self._internal_client = httpx.AsyncClient(
                base_url=self.settings.backend_url,
                headers=headers,
                timeout=self.settings.backend_timeout,
            )
        return self._internal_client

    async def close(self):
        if self._internal_client and not self._internal_client.is_closed:
            await self._internal_client.aclose()

    async def get_dados_cadastrais(self, cnpj: str) -> Dict[str, Any]:
        """Consulta dados cadastrais de uma empresa no endpoint Axum."""
        clean_cnpj = "".join(filter(str.isdigit, cnpj))
        if not clean_cnpj:
            return {
                "cnpj": cnpj,
                "erro": "Identificador CNPJ vazio ou inválido.",
                "encontrado": False,
            }

        client = await self.get_client()
        try:
            resp = await client.get(f"/api/fiscal/consulta-cnpj/{clean_cnpj}")
            if resp.status_code == 200:
                return resp.json()
            return {
                "cnpj": clean_cnpj,
                "erro": f"Status {resp.status_code}: {resp.text}",
                "encontrado": False,
            }
        except (httpx.ConnectError, httpx.TimeoutException) as exc:
            is_matriz = (len(clean_cnpj) == 14 and clean_cnpj[8:12] == "0001") or "/0001" in cnpj
            return {
                "cnpj": clean_cnpj,
                "aviso": f"Backend Axum indisponível ({exc.__class__.__name__}). Retornando simulação cadastral para auditoria.",
                "encontrado": True,
                "razao_social": f"EMPRESA EXEMPLO AUDITADA {clean_cnpj[:8]} LTDA",
                "cnae": "6201-5/01 - Desenvolvimento de programas de computador sob encomenda",
                "situacao": "ATIVA",
                "tipo": "MATRIZ" if is_matriz else "FILIAL",
            }

    async def get_demonstrativo_dre(self, cnpj: str, ano: Optional[int] = None) -> Dict[str, Any]:
        """Recupera apuração de DRE da escrituração SPED no backend Axum."""
        clean_cnpj = "".join(filter(str.isdigit, cnpj))
        if not clean_cnpj:
            return {
                "cnpj": cnpj,
                "ano": ano,
                "erro": "Identificador CNPJ vazio ou inválido.",
                "encontrado": False,
            }

        client = await self.get_client()
        params = {}
        if ano:
            params["ano"] = ano
        try:
            resp = await client.get(f"/api/fiscal/consulta-cnpj/{clean_cnpj}", params=params)
            if resp.status_code == 200:
                data = resp.json()
                if isinstance(data, dict):
                    # Se for a ConsultaCnpjResponse com resultado_exercicio aninhado
                    resultado = data.get("resultado_exercicio")
                    if isinstance(resultado, dict):
                        return {
                            "cnpj": clean_cnpj,
                            "ano": ano,
                            "receita_bruta": resultado.get("receita_bruta", 0.0),
                            "receita_servico": resultado.get("receita_servico", 0.0),
                            "outras_receitas": resultado.get("outras_receitas", 0.0),
                            "receita_acumulada": resultado.get("receita_acumulada", 0.0),
                            "total_nota": resultado.get("total_nota", 0.0),
                            "periodo_inicial": resultado.get("periodo_inicial"),
                            "periodo_final": resultado.get("periodo_final"),
                            "reg_j150": data.get("reg_j150", []),
                            "razao_social": data.get("razao_social", ""),
                            "encontrado": True,
                        }
                return data
            return {
                "cnpj": clean_cnpj,
                "ano": ano,
                "erro": f"Status {resp.status_code}: {resp.text}",
                "encontrado": False,
            }
        except (httpx.ConnectError, httpx.TimeoutException) as exc:
            ano_ref = ano or 2023
            return {
                "cnpj": clean_cnpj,
                "ano": ano_ref,
                "aviso": f"Backend Axum indisponível ({exc.__class__.__name__}). Retornando simulação contábil DRE.",
                "receita_bruta": 12500000.00,
                "receita_servico": 10200000.00,
                "outras_receitas": 2300000.00,
                "deducoes": 1875000.00,
                "receita_liquida": 10625000.00,
                "resultado_exercicio": 1950000.00,
                "periodo_inicial": f"{ano_ref}-01-01",
                "periodo_final": f"{ano_ref}-12-31",
            }

    async def get_grafo_societario(self, raiz: str, camadas: int = 2) -> Dict[str, Any]:
        """Consulta rede societária e sanções no backend Axum."""
        clean_raiz = "".join(filter(str.isalnum, raiz))
        if not clean_raiz:
            return {
                "raiz": raiz,
                "camadas": camadas,
                "erro": "Identificador raiz vazio ou inválido.",
            }

        client = await self.get_client()
        params = {"raiz": clean_raiz, "camadas": camadas}
        try:
            resp = await client.get("/api/grc/rede", params=params)
            if resp.status_code == 200:
                return resp.json()
            return {
                "raiz": clean_raiz,
                "camadas": camadas,
                "erro": f"Status {resp.status_code}: {resp.text}",
            }
        except (httpx.ConnectError, httpx.TimeoutException) as exc:
            return {
                "raiz": clean_raiz,
                "camadas": camadas,
                "aviso": f"Backend Axum indisponível ({exc.__class__.__name__}). Retornando simulação de grafo societário.",
                "nodes": [
                    {"id": clean_raiz, "label": f"Entidade Principal ({clean_raiz})", "tipo": "PJ", "flags_sancionatorias": []},
                    {"id": "00000000191", "label": "Sócio Administrador A", "tipo": "PF", "flags_sancionatorias": ["PEP"]},
                    {"id": "11222333000199", "label": "Coligada Participações S/A", "tipo": "PJ", "flags_sancionatorias": ["CEIS"]},
                ],
                "links": [
                    {"source": "00000000191", "target": clean_raiz, "label": "Sócio-Administrador", "tipo_vinculo": "SOCIO"},
                    {"source": clean_raiz, "target": "11222333000199", "label": "Coligada 40%", "tipo_vinculo": "PARTICIPACAO"},
                ],
                "total_camadas": camadas,
                "alertas_encontrados": 2,
            }

    async def get_indicadores_setor(self, cnae: Optional[str] = None, uf: Optional[str] = None) -> Dict[str, Any]:
        """Consulta distribuições analíticas econômicas e setoriais."""
        client = await self.get_client()
        params = {}
        if cnae:
            params["cnae"] = cnae
        if uf:
            params["uf"] = uf
        try:
            resp = await client.get("/api/indicadores/resumo", params=params)
            if resp.status_code == 200:
                return resp.json()
            return {
                "cnae": cnae,
                "uf": uf,
                "erro": f"Status {resp.status_code}: {resp.text}",
            }
        except (httpx.ConnectError, httpx.TimeoutException) as exc:
            return {
                "cnae": cnae or "Geral",
                "uf": uf or "Brasil",
                "aviso": f"Backend Axum indisponível ({exc.__class__.__name__}). Retornando simulação setorial.",
                "total_empresas": 48200,
                "porte": [
                    {"categoria": "Microempresa (ME)", "quantidade": 28900, "percentual": 59.95},
                    {"categoria": "Empresa de Pequeno Porte (EPP)", "quantidade": 12100, "percentual": 25.10},
                    {"categoria": "Demais Portes", "quantidade": 7200, "percentual": 14.95},
                ],
                "regime_tributario": [
                    {"categoria": "Simples Nacional", "quantidade": 31000, "percentual": 64.31},
                    {"categoria": "Lucro Presumido", "quantidade": 13200, "percentual": 27.38},
                    {"categoria": "Lucro Real", "quantidade": 4000, "percentual": 8.31},
                ],
            }


_global_backend_client: Optional[BackendClient] = None

def get_backend_client() -> BackendClient:
    global _global_backend_client
    if _global_backend_client is None:
        _global_backend_client = BackendClient()
    return _global_backend_client
