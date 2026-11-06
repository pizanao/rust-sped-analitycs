"""
Tests for Playwright Browser Runner with Audio-Driven Cadence and Spotlight Highlight.
"""

import json
from pathlib import Path
import subprocess
import pytest

from scripts.schema import DemoScript, DemoConfig, Scene, Action
from scripts.browser_runner import BrowserRunner, BrowserRunnerError


def _probe_video_dimensions(file_path: Path) -> dict:
    """Uses ffprobe to inspect video width, height, and duration."""
    cmd = [
        "ffprobe",
        "-v", "quiet",
        "-print_format", "json",
        "-show_streams",
        "-show_format",
        str(file_path),
    ]
    res = subprocess.run(cmd, capture_output=True, text=True, check=True)
    data = json.loads(res.stdout)
    video_stream = next((s for s in data.get("streams", []) if s.get("codec_type") == "video"), {})
    return {
        "width": int(video_stream.get("width", 0)),
        "height": int(video_stream.get("height", 0)),
        "duration": float(data.get("format", {}).get("duration", 0.0)),
    }


def test_browser_runner_records_1080p_video(test_page_path: Path, tmp_path: Path):
    """Verifies that record_demo generates a 1920x1080 video with audio-driven timing."""
    output_video = tmp_path / "recorded_demo.webm"
    temp_dir = tmp_path / "temp_run"

    script = DemoScript(
        title="Browser Runner Test",
        description="Testing video recording and highlight",
        config=DemoConfig(
            viewport={"width": 1920, "height": 1080},
            temp_dir=str(temp_dir),
        ),
        scenes=[
            Scene(
                id="sc_nav",
                title="Navegação",
                narration="Abrindo página de teste.",
                audio_duration=1.0,
                pause_after=0.5,
                actions=[
                    Action(action="navigate", url=test_page_path.as_uri()),
                ],
            ),
            Scene(
                id="sc_type",
                title="Busca",
                narration="Digitando busca.",
                audio_duration=1.0,
                pause_after=0.5,
                actions=[
                    Action(action="type", selector="input[name='search']", text="Vendas 2026", highlight=True),
                    Action(action="click", selector="#btn-search", highlight=True),
                ],
            ),
        ],
    )

    runner = BrowserRunner(config=script.config, headless=True)
    result_path = runner.record_demo(script, output_video)

    assert Path(result_path).exists()
    assert Path(result_path).stat().st_size > 1024

    # Verify video properties using ffprobe
    props = _probe_video_dimensions(output_video)
    assert props["width"] == 1920, f"Expected 1920 width, got {props['width']}"
    assert props["height"] == 1080, f"Expected 1080 height, got {props['height']}"
    assert props["duration"] >= 2.5, f"Expected at least ~3s duration, got {props['duration']}s"

    # Verify timeline log was generated
    timeline_file = temp_dir / "timeline.json"
    assert timeline_file.exists()
    timeline_data = json.loads(timeline_file.read_text(encoding="utf-8"))
    assert len(timeline_data["scenes"]) == 2


def test_browser_runner_missing_selector_raises_error(test_page_path: Path, tmp_path: Path):
    """Verifies that executing an invalid action raises BrowserRunnerError."""
    script = DemoScript(
        title="Invalid Action Test",
        description="Testing error handling",
        config=DemoConfig(temp_dir=str(tmp_path / "temp")),
        scenes=[
            Scene(
                id="sc_err",
                title="Error Scene",
                narration="Ação sem seletor.",
                actions=[
                    Action(action="click", selector=None),
                ],
            ),
        ],
    )

    runner = BrowserRunner(config=script.config, headless=True)
    with pytest.raises(BrowserRunnerError, match="missing 'selector'"):
        runner.record_demo(script, tmp_path / "err.webm")
