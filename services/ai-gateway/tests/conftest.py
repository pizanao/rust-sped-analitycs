"""
Configuração e fixtures globais para testes do AI Gateway com pytest.
"""

import os
import pytest
from fastapi.testclient import TestClient

# Garante ambiente de teste sem chamadas externas acidentais
os.environ["LLM_PROVIDER"] = "mock"
os.environ["ECD_BACKEND_URL"] = "http://localhost:3000"
os.environ["ECD_BACKEND_TIMEOUT"] = "1.0"

from app.config import get_settings
from app.main import app


@pytest.fixture(autouse=True)
def reset_settings_cache():
    get_settings.cache_clear()
    yield
    get_settings.cache_clear()


@pytest.fixture
def client():
    with TestClient(app) as test_client:
        yield test_client
