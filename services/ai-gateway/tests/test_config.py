"""
Testes de configuração, Settings e fábrica de LLM.
"""

from app.config import Settings, get_llm, get_settings
from app.mock_llm import MockFiscalChatModel


def test_settings_defaults():
    """Valida valores padrão das configurações."""
    settings = Settings()
    assert settings.backend_url == "http://localhost:3000"
    assert settings.temperature == 0.0
    assert "llama-3.3" in settings.groq_model
    assert "gpt-4o" in settings.openai_model


def test_get_llm_mock_fallback():
    """Valida que get_llm retorna MockFiscalChatModel na ausência de chaves."""
    settings = Settings(llm_provider="mock")
    llm = get_llm(settings=settings)
    assert isinstance(llm, MockFiscalChatModel)


def test_get_llm_groq_instantiation():
    """Valida tentativa de instanciação de ChatGroq quando chave está presente."""
    settings = Settings(llm_provider="groq", groq_api_key="gsk-test-key-fake")
    llm = get_llm(settings=settings)
    assert type(llm).__name__ == "ChatGroq"


def test_get_llm_openai_instantiation():
    """Valida tentativa de instanciação de ChatOpenAI quando chave está presente."""
    settings = Settings(llm_provider="openai", openai_api_key="sk-test-key-fake")
    llm = get_llm(settings=settings)
    assert type(llm).__name__ == "ChatOpenAI"
