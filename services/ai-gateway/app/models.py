"""
Modelos de dados (schemas Pydantic) para o AI Gateway.
Define contratos de entrada, saída e especificações estritas de gráficos Recharts
com total compatibilidade para DynamicChartRenderer (frontend SPA) e formato canônico.
"""

from typing import Any, Dict, List, Literal, Optional, Union
from pydantic import BaseModel, ConfigDict, Field, model_validator


class DatasetSpec(BaseModel):
    label: str = Field(description="Rótulo da série de dados (ex: 'Receita Bruta 2023')")
    data: List[Union[float, int]] = Field(description="Lista de valores numéricos correspondentes a cada label")
    backgroundColor: Optional[str] = Field(default=None, description="Cor de preenchimento CSS/Hex")
    borderColor: Optional[str] = Field(default=None, description="Cor da borda CSS/Hex")


class ChartSpec(BaseModel):
    model_config = ConfigDict(populate_by_name=True, extra="allow")

    # Contrato canônico Recharts (usado por DynamicChartRenderer.tsx no React)
    chart_type: Literal["bar", "line", "pie", "area"] = Field(
        default="bar",
        description="Tipo de gráfico suportado pelo DynamicChartRenderer ('bar', 'line', 'pie', 'area')"
    )
    title: str = Field(
        default="Gráfico Analítico",
        description="Título do gráfico exibido no painel analítico"
    )
    description: Optional[str] = Field(
        default=None,
        description="Texto descritivo ou nota de auditoria do gráfico"
    )
    x_key: str = Field(
        default="label",
        description="Chave do eixo X / categorias no array de dados"
    )
    y_keys: List[str] = Field(
        default_factory=list,
        description="Lista de chaves numéricas das séries a plotar no gráfico"
    )
    colors: Optional[List[str]] = Field(
        default=None,
        description="Cores das séries (hex/CSS)"
    )
    data: List[Dict[str, Any]] = Field(
        default_factory=list,
        description="Dados tabulares no formato Recharts: [{x_key: ..., y1: ..., y2: ...}]"
    )

    # Campos de compatibilidade para Chart.js / testes e schemas legados
    tipo: Optional[Literal["bar", "line", "pie", "area"]] = Field(
        default=None,
        description="Alias de compatibilidade para chart_type"
    )
    titulo: Optional[str] = Field(
        default=None,
        description="Alias de compatibilidade para title"
    )
    descricao: Optional[str] = Field(
        default=None,
        description="Alias de compatibilidade para description"
    )
    labels: List[str] = Field(
        default_factory=list,
        description="Lista de categorias ou rótulos do eixo X"
    )
    datasets: List[DatasetSpec] = Field(
        default_factory=list,
        description="Lista de séries estruturadas (DatasetSpec)"
    )

    @model_validator(mode="before")
    @classmethod
    def _harmonize_before(cls, values: Any) -> Any:
        if not isinstance(values, dict):
            return values

        # Mapeia tipo/chart_type
        c_type = values.get("chart_type") or values.get("tipo")
        if c_type:
            values["chart_type"] = c_type
            values["tipo"] = c_type

        # Mapeia titulo/title
        c_title = values.get("title") or values.get("titulo")
        if c_title:
            values["title"] = c_title
            values["titulo"] = c_title

        # Mapeia descricao/description
        c_desc = values.get("description") or values.get("descricao")
        if c_desc:
            values["description"] = c_desc
            values["descricao"] = c_desc

        labels = values.get("labels") or []
        datasets = values.get("datasets") or []
        data_rows = values.get("data") or []

        # Se temos labels + datasets mas data está vazio, constrói data tabular Recharts
        if labels and datasets and not data_rows:
            x_k = values.get("x_key") or "label"
            y_ks = []
            colors_list = list(values.get("colors") or [])
            built_rows: List[Dict[str, Any]] = []

            for i, lbl in enumerate(labels):
                row: Dict[str, Any] = {x_k: str(lbl)}
                for d in datasets:
                    if isinstance(d, dict):
                        ds_label = str(d.get("label", "Valor"))
                        ds_data = d.get("data", [])
                        bg_color = d.get("backgroundColor")
                    elif isinstance(d, DatasetSpec):
                        ds_label = d.label
                        ds_data = d.data
                        bg_color = d.backgroundColor
                    else:
                        continue

                    if ds_label not in y_ks:
                        y_ks.append(ds_label)
                        if bg_color and bg_color not in colors_list:
                            colors_list.append(bg_color)

                    val = ds_data[i] if i < len(ds_data) else 0.0
                    row[ds_label] = val
                built_rows.append(row)

            values["data"] = built_rows
            values["x_key"] = x_k
            values["y_keys"] = values.get("y_keys") or y_ks
            if colors_list and not values.get("colors"):
                values["colors"] = colors_list

        return values

    @model_validator(mode="after")
    def _harmonize_after(self) -> "ChartSpec":
        # Garante sincronização bidirecional
        self.tipo = self.chart_type
        self.titulo = self.title
        self.descricao = self.description

        # Se data foi fornecido diretamente mas labels/datasets estão vazios, popula labels/datasets
        if self.data and not self.labels:
            self.labels = [str(r.get(self.x_key, "")) for r in self.data]
            if not self.y_keys and self.data:
                # Extrai chaves numéricas do primeiro registro
                self.y_keys = [k for k, v in self.data[0].items() if k != self.x_key and isinstance(v, (int, float))]

            if not self.datasets:
                color_idx = 0
                for yk in self.y_keys:
                    c_color = self.colors[color_idx] if self.colors and color_idx < len(self.colors) else None
                    color_idx += 1
                    series_data = [
                        float(r.get(yk, 0.0)) if isinstance(r.get(yk), (int, float)) else 0.0
                        for r in self.data
                    ]
                    self.datasets.append(DatasetSpec(
                        label=yk,
                        data=series_data,
                        backgroundColor=c_color
                    ))

        return self


class ChatQuery(BaseModel):
    message: Optional[str] = Field(default=None, description="Mensagem ou pergunta do auditor em linguagem natural")
    mensagem: Optional[str] = Field(default=None, description="Alias em português para a pergunta do auditor")
    cnpj_context: Optional[str] = Field(default=None, description="CNPJ opcional sob contexto imediato de análise")
    contexto: Optional[Dict[str, Any]] = Field(default=None, description="Contexto adicional em dicionário (ex: {'cnpj': '...'})")
    conversation_id: Optional[str] = Field(default=None, description="Identificador único da conversa para histórico")

    @model_validator(mode="after")
    def _resolve_fields(self) -> "ChatQuery":
        content = (self.message or self.mensagem or "").strip()
        if not content:
            raise ValueError("O campo 'message' ou 'mensagem' é obrigatório e não pode ser vazio.")
        self.message = content
        self.mensagem = content

        if not self.cnpj_context and self.contexto and isinstance(self.contexto, dict):
            c = self.contexto.get("cnpj") or self.contexto.get("cnpj_context")
            if c:
                self.cnpj_context = str(c).strip()

        return self


class ChatResponse(BaseModel):
    relatorio_markdown: str = Field(default="", description="Parecer técnico ou resposta sintetizada em Markdown")
    mensagem: str = Field(default="", description="Mensagem principal do assistente (compatível com AiChatResponse)")
    chart: Optional[ChartSpec] = Field(default=None, description="Gráfico principal gerado para o DynamicChartRenderer")
    graficos: List[ChartSpec] = Field(default_factory=list, description="Lista de gráficos para renderização dinâmica")
    alertas: List[str] = Field(default_factory=list, description="Lista de riscos ou alertas sancionatórios identificados")
    ferramentas_executadas: List[str] = Field(default_factory=list, description="Lista de ferramentas invocadas durante o raciocínio")
    conversation_id: Optional[str] = Field(default=None, description="Identificador da sessão conversacional")

    @model_validator(mode="after")
    def _sync_fields(self) -> "ChatResponse":
        if not self.mensagem and self.relatorio_markdown:
            self.mensagem = self.relatorio_markdown
        if not self.relatorio_markdown and self.mensagem:
            self.relatorio_markdown = self.mensagem

        if self.chart is None and self.graficos:
            self.chart = self.graficos[0]
        if self.chart is not None and not self.graficos:
            self.graficos = [self.chart]

        return self
