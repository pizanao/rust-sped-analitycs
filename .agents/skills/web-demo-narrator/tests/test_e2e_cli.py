"""End-to-end and CLI acceptance tests for web-demo-narrator skill.

Covers Tiers 1-4 opaque-box requirements:
- Tier 1: CLI help, flags, fixture and YAML schema integrity
- Tier 2: Subcommand argument handling and error recovery
- Tier 3: Subcommand execution pipelines (humanize, synthesize, record)
- Tier 4: Full E2E build verification (MP4 container, H.264/AAC, drift < 0.5s)
"""

import json
import shutil
import subprocess
from pathlib import Path
from typing import Dict, Any

import pytest
import yaml

pytestmark = pytest.mark.filterwarnings("ignore::DeprecationWarning")


# ==============================================================================
# Helper Assertions
# ==============================================================================

def _skip_if_no_cli(cli_path: Path):
    """Skips the test if scripts/cli.py has not been implemented yet."""
    if not cli_path.exists():
        pytest.skip("scripts/cli.py is not yet implemented (Milestone 5)")


def _probe_media_file(file_path: Path) -> Dict[str, Any]:
    """Inspects a media file using ffprobe and returns stream details."""
    cmd = [
        "ffprobe",
        "-v", "quiet",
        "-print_format", "json",
        "-show_format",
        "-show_streams",
        str(file_path),
    ]
    result = subprocess.run(cmd, capture_output=True, text=True, check=True)
    return json.loads(result.stdout)


# ==============================================================================
# Tier 1: Fixtures, Schema and CLI Help Integrity
# ==============================================================================

def test_requirements_txt_contains_mandatory_dependencies(skill_root: Path):
    """Verifies requirements.txt contains all specified skill dependencies."""
    req_file = skill_root / "requirements.txt"
    assert req_file.exists(), "requirements.txt must exist at the skill root"

    content = req_file.read_text(encoding="utf-8").lower()
    expected_packages = ["playwright", "pyyaml", "requests", "rich", "pydantic", "pytest"]
    for pkg in expected_packages:
        assert pkg in content, f"Expected package '{pkg}' missing in requirements.txt"


def test_test_page_html_fixture_integrity(test_page_path: Path):
    """Verifies that fixtures/test_page.html exists, is valid HTML5, and offline-ready."""
    assert test_page_path.exists(), "fixtures/test_page.html must exist"
    html_content = test_page_path.read_text(encoding="utf-8")

    # Offline-ready: No external HTTP/HTTPS CDN references
    assert "https://" not in html_content, "test_page.html must not depend on external HTTPS resources"
    assert "http://" not in html_content, "test_page.html must not depend on external HTTP resources"

    # Mandatory selectors required by demo script and specifications
    required_selectors = [
        "input[name='search']",
        "btn-search",
        "btn-checkout",
        "btn-filtrar",
        "status-badge",
        "status-banner",
        "data-table",
        "hero-heading",
    ]
    for sel in required_selectors:
        assert sel in html_content, f"Required DOM selector or element ID '{sel}' not found in test_page.html"


def test_sample_demo_yaml_schema_conformance(sample_demo_yaml_path: Path, sample_demo_dict: Dict[str, Any]):
    """Verifies examples/sample_demo.yaml conforms strictly to PROJECT.md contracts."""
    assert sample_demo_yaml_path.exists(), "examples/sample_demo.yaml must exist"

    # Top-level required keys
    assert "title" in sample_demo_dict, "Sample demo missing 'title'"
    assert "config" in sample_demo_dict, "Sample demo missing 'config'"
    assert "scenes" in sample_demo_dict, "Sample demo missing 'scenes'"

    # Config validation
    cfg = sample_demo_dict["config"]
    assert "viewport" in cfg, "Config missing 'viewport'"
    assert cfg["viewport"].get("width") == 1920, "Viewport width must be 1920"
    assert cfg["viewport"].get("height") == 1080, "Viewport height must be 1080"
    assert cfg.get("voice_id") == "bf91cd6a", "Voice ID must default to 'bf91cd6a'"
    assert cfg.get("model") == "omnivoice", "Model must default to 'omnivoice'"

    # Scenes validation
    scenes = sample_demo_dict["scenes"]
    assert len(scenes) >= 3, "Sample demo should contain at least 3 scenes"

    valid_actions = {"navigate", "click", "type", "wait", "select", "screenshot", "press", "fill", "hover", "scroll"}
    for idx, scene in enumerate(scenes):
        assert "id" in scene, f"Scene #{idx} missing 'id'"
        assert "title" in scene, f"Scene #{idx} missing 'title'"
        assert "narration" in scene, f"Scene #{idx} missing 'narration'"
        assert len(scene["narration"].strip()) > 10, f"Scene #{idx} narration is too short"
        assert "actions" in scene, f"Scene #{idx} missing 'actions'"

        for a_idx, act in enumerate(scene["actions"]):
            assert "action" in act, f"Action #{a_idx} in scene {scene['id']} missing 'action' field"
            assert act["action"] in valid_actions, f"Unknown action type: {act['action']}"


def test_cli_main_help(cli_path: Path, run_cli):
    """Verifies that the CLI entrypoint responds to --help and lists all 4 subcommands."""
    _skip_if_no_cli(cli_path)
    res = run_cli("--help")
    assert res.returncode == 0, f"cli.py --help failed: {res.stderr}"
    output = res.stdout.lower()
    for subcmd in ["humanize", "synthesize", "record", "build"]:
        assert subcmd in output, f"Subcommand '{subcmd}' missing in CLI help output"


@pytest.mark.parametrize("subcmd", ["humanize", "synthesize", "record", "build"])
def test_cli_subcommands_help(cli_path: Path, run_cli, subcmd: str):
    """Verifies that each individual subcommand provides a detailed help menu."""
    _skip_if_no_cli(cli_path)
    res = run_cli(subcmd, "--help")
    assert res.returncode == 0, f"Subcommand {subcmd} --help failed: {res.stderr}"
    assert "usage:" in res.stdout.lower()


# ==============================================================================
# Tier 2: Argument Validation and Error Recovery
# ==============================================================================

def test_cli_missing_input_file_exits_with_error(cli_path: Path, run_cli):
    """Verifies running the CLI with a non-existent file produces a non-zero exit code."""
    _skip_if_no_cli(cli_path)
    res = run_cli("humanize", "non_existent_demo_script.yaml")
    assert res.returncode != 0, "CLI should fail on non-existent input script"


def test_cli_malformed_yaml_exits_with_error(cli_path: Path, run_cli, tmp_path: Path):
    """Verifies running CLI on corrupted YAML produces a clear validation failure."""
    _skip_if_no_cli(cli_path)
    bad_yaml = tmp_path / "bad.yaml"
    bad_yaml.write_text("invalid_yaml: [unclosed_bracket", encoding="utf-8")

    res = run_cli("humanize", str(bad_yaml))
    assert res.returncode != 0, "CLI should fail on malformed YAML syntax"


def test_cli_invalid_action_type_exits_with_error(cli_path: Path, run_cli, tmp_path: Path):
    """Verifies schema validation catches unknown action verbs."""
    _skip_if_no_cli(cli_path)
    invalid_script = tmp_path / "invalid_action.yaml"
    content = {
        "title": "Invalid Action Test",
        "description": "Tests schema rejection of illegal actions",
        "config": {"viewport": {"width": 1920, "height": 1080}},
        "scenes": [
            {
                "id": "scene_err",
                "title": "Err",
                "narration": "Texto teste",
                "actions": [{"action": "non_existent_super_action", "selector": "#btn"}],
            }
        ],
    }
    with open(invalid_script, "w", encoding="utf-8") as f:
        yaml.safe_dump(content, f)

    res = run_cli("build", str(invalid_script))
    assert res.returncode != 0, "CLI must reject unknown action types"


# ==============================================================================
# Tier 3: Subcommand Execution Flows
# ==============================================================================

def test_cli_humanize_preserves_technical_tokens(
    cli_path: Path, run_cli, tmp_path: Path, sample_demo_yaml_path: Path
):
    """Verifies `humanize` processes script while preserving technical selectors."""
    _skip_if_no_cli(cli_path)
    output_yaml = tmp_path / "humanized_demo.yaml"

    res = run_cli("humanize", str(sample_demo_yaml_path), "--output", str(output_yaml))
    assert res.returncode == 0, f"humanize failed: {res.stderr}"
    assert output_yaml.exists(), "Humanized output YAML was not created"

    with open(output_yaml, "r", encoding="utf-8") as f:
        data = yaml.safe_load(f)

    # Verify selectors remain intact
    selectors_in_output = []
    for scene in data.get("scenes", []):
        for act in scene.get("actions", []):
            if "selector" in act and act["selector"]:
                selectors_in_output.append(act["selector"])

    assert "input[name='search']" in selectors_in_output
    assert "#btn-search" in selectors_in_output
    assert "#btn-checkout" in selectors_in_output


def test_cli_humanize_diff_mode(cli_path: Path, run_cli, sample_demo_yaml_path: Path):
    """Verifies `humanize --diff` executes without throwing exceptions."""
    _skip_if_no_cli(cli_path)
    res = run_cli("humanize", str(sample_demo_yaml_path), "--diff")
    assert res.returncode == 0, f"humanize --diff failed: {res.stderr}"


# ==============================================================================
# Tier 4: End-to-End Build Integration Tests
# ==============================================================================

@pytest.mark.integration
def test_integration_full_demo_build_pipeline(
    cli_path: Path,
    run_cli,
    tmp_path: Path,
    sample_demo_dict: Dict[str, Any],
    test_page_path: Path,
):
    """End-to-end acceptance test: executes full build pipeline to generate final MP4.
    
    Verifies:
    1. Successful execution of `cli.py build`
    2. Generation of output/demo_final.mp4
    3. Media container validation (MP4)
    4. Video track validation (H.264, 1920x1080)
    5. Audio track validation (AAC, 48kHz mono or stereo)
    6. Audio/video duration drift < 0.5s
    """
    _skip_if_no_cli(cli_path)

    # Prepare localized demo script pointing directly to file:// test page
    workspace = tmp_path / "e2e_run"
    workspace.mkdir(parents=True, exist_ok=True)
    custom_script_path = workspace / "test_demo.yaml"
    output_mp4 = workspace / "demo_e2e_output.mp4"

    script_data = sample_demo_dict.copy()
    # Ensure navigation points to the local test page
    for scene in script_data.get("scenes", []):
        for act in scene.get("actions", []):
            if act.get("action") == "navigate":
                act["url"] = test_page_path.as_uri()

    with open(custom_script_path, "w", encoding="utf-8") as f:
        yaml.safe_dump(script_data, f)

    # Run build subcommand
    res = run_cli(
        "build",
        str(custom_script_path),
        "--output",
        str(output_mp4),
        "--headless",
        cwd=workspace,
    )
    assert res.returncode == 0, f"build failed:\nSTDOUT:\n{res.stdout}\nSTDERR:\n{res.stderr}"
    assert output_mp4.exists(), f"Output MP4 {output_mp4} was not generated"
    assert output_mp4.stat().st_size > 1024, "Output MP4 is empty or corrupted"

    # Inspect media streams using ffprobe
    probe = _probe_media_file(output_mp4)
    streams = probe.get("streams", [])
    format_info = probe.get("format", {})

    # Check container format
    assert "mp4" in format_info.get("format_name", "").lower(), "Output file must be an MP4 container"

    # Identify streams
    video_streams = [s for s in streams if s.get("codec_type") == "video"]
    audio_streams = [s for s in streams if s.get("codec_type") == "audio"]

    assert len(video_streams) >= 1, "Output MP4 has no video stream"
    assert len(audio_streams) >= 1, "Output MP4 has no audio stream"

    video = video_streams[0]
    audio = audio_streams[0]

    # Verify video properties
    assert video.get("codec_name") == "h264", f"Expected H.264 video, got: {video.get('codec_name')}"
    assert video.get("width") == 1920, f"Expected 1920 width, got: {video.get('width')}"
    assert video.get("height") == 1080, f"Expected 1080 height, got: {video.get('height')}"

    # Verify audio properties
    assert audio.get("codec_name") == "aac", f"Expected AAC audio, got: {audio.get('codec_name')}"
    assert int(audio.get("sample_rate", 0)) == 48000, f"Expected 48000Hz sample rate, got: {audio.get('sample_rate')}"

    # Verify temporal alignment (drift < 0.5s)
    video_duration = float(video.get("duration", format_info.get("duration", 0)))
    audio_duration = float(audio.get("duration", format_info.get("duration", 0)))
    drift = abs(video_duration - audio_duration)
    assert drift <= 0.5, f"Audio/video duration drift too high: {drift:.3f}s (video: {video_duration}s, audio: {audio_duration}s)"


@pytest.mark.integration
def test_integration_build_with_skip_humanize_flag(
    cli_path: Path,
    run_cli,
    tmp_path: Path,
    sample_demo_dict: Dict[str, Any],
    test_page_path: Path,
):
    """Verifies the --skip-humanize flag bypasses the speech humanizer during build."""
    _skip_if_no_cli(cli_path)

    workspace = tmp_path / "skip_humanize_run"
    workspace.mkdir(parents=True, exist_ok=True)
    custom_script_path = workspace / "skip_demo.yaml"
    output_mp4 = workspace / "skip_output.mp4"

    script_data = sample_demo_dict.copy()
    for scene in script_data.get("scenes", []):
        for act in scene.get("actions", []):
            if act.get("action") == "navigate":
                act["url"] = test_page_path.as_uri()

    with open(custom_script_path, "w", encoding="utf-8") as f:
        yaml.safe_dump(script_data, f)

    res = run_cli(
        "build",
        str(custom_script_path),
        "--skip-humanize",
        "--output",
        str(output_mp4),
        "--headless",
        cwd=workspace,
    )
    assert res.returncode == 0, f"build with --skip-humanize failed: {res.stderr}"
    assert output_mp4.exists(), "Output MP4 was not created"
