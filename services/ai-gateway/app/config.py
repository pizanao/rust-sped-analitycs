"""
Configurações do AI Gateway com Pydantic Settings e fábrica de LLM.
"""

from functools import lru_cache
from typing import List, Optional
from langchain_core.language_models.chat_models import BaseChatModel
from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore"
    )

    # Backend Rust (Axum) API
    backend_url: str = Field(default="http://localhost:3000", alias="ECD_BACKEND_URL")
    backend_api_key: Optional[str] = Field(default=None, alias="ECD_BACKEND_API_KEY")
    backend_timeout: float = Field(default=10.0, alias="ECD_BACKEND_TIMEOUT")

    # LLM Settings
    llm_provider: str = Field(default="groq", alias="LLM_PROVIDER")  # "groq", "openai", "mock"
    
    # Groq
    groq_api_key: Optional[str] = Field(default=None, alias="GROQ_API_KEY")
    groq_model: str = Field(default="llama-3.3-70b-versatile", alias="GROQ_MODEL")

    # OpenAI
    openai_api_key: Optional[str] = Field(default=None, alias="OPENAI_API_KEY")
    openai_model: str = Field(default="gpt-4o", alias="OPENAI_MODEL")

    # Geral LLM
    temperature: float = Field(default=0.0, alias="LLM_TEMPERATURE")
    max_tokens: Optional[int] = Field(default=4096, alias="LLM_MAX_TOKENS")

    # CORS
    cors_origins: List[str] = Field(default=["*"], alias="CORS_ORIGINS")


@lru_cache()
def get_settings() -> Settings:
    return Settings()


def get_llm(
    settings: Optional[Settings] = None,
    provider_override: Optional[str] = None
) -> BaseChatModel:
    """
    Fábrica que instancia o cliente LLM com base na configuração ativa.
    Se nenhuma chave for fornecida ou o provedor for 'mock', utiliza MockFiscalChatModel.
    """
    s = settings or get_settings()
    provider = provider_override or s.llm_provider.lower()

    if provider == "groq" and s.groq_api_key:
        from langchain_groq import ChatGroq
        return ChatGroq(
            model=s.groq_model,
            groq_api_key=s.groq_api_key,
            temperature=s.temperature,
            max_tokens=s.max_tokens,
        )

    if provider == "openai" and s.openai_api_key:
        from langchain_openai import ChatOpenAI
        return ChatOpenAI(
            model=s.openai_model,
            api_key=s.openai_api_key,
            temperature=s.temperature,
            max_tokens=s.max_tokens,
        )

    from app.mock_llm import MockFiscalChatModel
    return MockFiscalChatModel()
