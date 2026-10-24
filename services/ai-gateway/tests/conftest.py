"""
Configuração e fixtures globais para testes do AI Gateway com pytest.
"""

import os
import pytest
from fastapi.testclient import TestClient

# Garante ambiente de teste sem chamadas externas acidentais
os.environ["LLM_PROVIDER"] = "mock"
os.environ["ECD_BACKEND_URL"] = "http://127.0.0.1:59999"
os.environ["ECD_BACKEND_TIMEOUT"] = "0.5"

from app.config import get_settings
import app.client as backend_client_module
from app.main import app


@pytest.fixture(autouse=True)
def reset_settings_cache():
    get_settings.cache_clear()
    backend_client_module._global_backend_client = None
    yield
    get_settings.cache_clear()
    backend_client_module._global_backend_client = None


@pytest.fixture
def client():
    with TestClient(app) as test_client:
        yield test_client
