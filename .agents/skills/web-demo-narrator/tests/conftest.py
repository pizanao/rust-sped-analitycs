"""Pytest configuration and shared fixtures for web-demo-narrator test suite."""

import os
import sys
import math
import struct
import wave
import subprocess
from pathlib import Path
from typing import Dict, Any, Generator, Callable

import pytest
import yaml

# Ensure skill root is in python sys.path for direct imports
SKILL_ROOT = Path(__file__).resolve().parent.parent
if str(SKILL_ROOT) not in sys.path:
    sys.path.insert(0, str(SKILL_ROOT))
SCRIPTS_DIR = SKILL_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))


def pytest_configure(config):
    """Register custom markers."""
    config.addinivalue_line(
        "markers", "integration: mark test as requiring external services or full build pipeline"
    )


@pytest.fixture(scope="session")
def skill_root() -> Path:
    """Returns the root directory of the web-demo-narrator skill."""
    return SKILL_ROOT


@pytest.fixture(scope="session")
def fixtures_dir(skill_root: Path) -> Path:
    """Returns the fixtures directory."""
    return skill_root / "fixtures"


@pytest.fixture(scope="session")
def examples_dir(skill_root: Path) -> Path:
    """Returns the examples directory."""
    return skill_root / "examples"


@pytest.fixture(scope="session")
def scripts_dir(skill_root: Path) -> Path:
    """Returns the scripts directory."""
    return skill_root / "scripts"


@pytest.fixture(scope="session")
def cli_path(scripts_dir: Path) -> Path:
    """Returns the path to scripts/cli.py."""
    return scripts_dir / "cli.py"


@pytest.fixture(scope="session")
def test_page_path(fixtures_dir: Path) -> Path:
    """Returns the absolute path to fixtures/test_page.html."""
    return fixtures_dir / "test_page.html"


@pytest.fixture(scope="session")
def test_page_url(test_page_path: Path) -> str:
    """Returns a file:// URI to the local test page fixture."""
    return test_page_path.as_uri()


@pytest.fixture(scope="session")
def sample_demo_yaml_path(examples_dir: Path) -> Path:
    """Returns the path to examples/sample_demo.yaml."""
    return examples_dir / "sample_demo.yaml"


@pytest.fixture
def sample_demo_dict(sample_demo_yaml_path: Path) -> Dict[str, Any]:
    """Loads and returns the sample demo YAML as a Python dict."""
    with open(sample_demo_yaml_path, "r", encoding="utf-8") as f:
        return yaml.safe_load(f)


@pytest.fixture
def synthetic_wav_factory(tmp_path: Path) -> Callable[..., Path]:
    """Creates a deterministic synthetic WAV audio file with requested duration.
    
    Generates standard 48kHz mono 16-bit or 24-bit PCM WAV for testing
    audio stitching and muxing without external network calls or TTS latency.
    """
    def _create_wav(
        filename: str = "test_audio.wav",
        duration_seconds: float = 1.0,
        sample_rate: int = 48000,
        frequency: float = 440.0,
        output_dir: Path = tmp_path,
    ) -> Path:
        out_file = output_dir / filename
        out_file.parent.mkdir(parents=True, exist_ok=True)
        
        n_samples = int(duration_seconds * sample_rate)
        # Generate 16-bit PCM sine wave
        with wave.open(str(out_file), "wb") as wf:
            wf.setnchannels(1)  # Mono
            wf.setsampwidth(2)  # 16-bit (2 bytes)
            wf.setframerate(sample_rate)
            
            # Write audio frames
            raw_data = bytearray()
            for i in range(n_samples):
                val = int(16000.0 * math.sin(2.0 * math.pi * frequency * (i / sample_rate)))
                raw_data.extend(struct.pack("<h", val))
            wf.writeframes(bytes(raw_data))
            
        return out_file

    return _create_wav


@pytest.fixture
def temp_demo_workspace(tmp_path: Path, skill_root: Path, test_page_path: Path) -> Path:
    """Sets up an isolated demo workspace with copied fixtures and config."""
    workspace = tmp_path / "demo_workspace"
    workspace.mkdir(parents=True, exist_ok=True)
    
    # Copy fixtures
    fixtures_dest = workspace / "fixtures"
    fixtures_dest.mkdir(parents=True, exist_ok=True)
    with open(test_page_path, "r", encoding="utf-8") as f_src:
        with open(fixtures_dest / "test_page.html", "w", encoding="utf-8") as f_dst:
            f_dst.write(f_src.read())
            
    # Create output and temp dirs
    (workspace / "output").mkdir(exist_ok=True)
    (workspace / ".temp_demo").mkdir(exist_ok=True)
    
    return workspace


@pytest.fixture
def run_cli(cli_path: Path, skill_root: Path):
    """Helper fixture to invoke the CLI script as a subprocess."""
    def _run(*args: str, check: bool = False, cwd: Path = skill_root) -> subprocess.CompletedProcess:
        env = os.environ.copy()
        python_path = str(skill_root)
        if "PYTHONPATH" in env:
            env["PYTHONPATH"] = f"{python_path}:{env['PYTHONPATH']}"
        else:
            env["PYTHONPATH"] = python_path

        cmd = [sys.executable, str(cli_path)] + list(args)
        return subprocess.run(
            cmd,
            cwd=str(cwd),
            env=env,
            capture_output=True,
            text=True,
            check=check,
        )

    return _run
