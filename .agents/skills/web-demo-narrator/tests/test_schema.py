"""
Unit tests for script schema and YAML validation.
Tests strict contract compliance for DemoScript, Scene, Action, and DemoConfig.
"""

from pathlib import Path
import pytest
import yaml

from scripts.schema import (
    Action,
    Scene,
    DemoConfig,
    DemoScript,
    SchemaValidationError,
    load_demo_script,
    dump_demo_script,
    load_and_validate_demo_yaml,
    save_demo_yaml,
)


def test_load_valid_scenes_script(tmp_path: Path):
    """Verifies that a full YAML script with scenes loads correctly."""
    yaml_content = """
title: "Demonstração do Painel"
description: "Roteiro com múltiplas cenas e ações"
config:
  viewport:
    width: 1920
    height: 1080
  fps: 30
  voice_id: "bf91cd6a"
  model: "omnivoice"
  base_url: "http://localhost:3900"
  output_dir: "output"
  temp_dir: ".temp_demo"

scenes:
  - id: "scene_01"
    title: "Abertura"
    narration: "Olá a todos! Sejam bem-vindos à demonstração."
    pause_after: 1.0
    actions:
      - action: "navigate"
        url: "http://localhost:5173"
      - action: "wait"
        seconds: 1.5

  - id: "scene_02"
    title: "Busca e Filtro"
    narration: "Digitamos o termo de busca e confirmamos no botão."
    actions:
      - action: "type"
        selector: "input[name='search']"
        text: "Relatório"
        highlight: true
      - action: "click"
        selector: "#btn-filtrar"
        highlight: true
"""
    yaml_file = tmp_path / "valid_demo.yaml"
    yaml_file.write_text(yaml_content, encoding="utf-8")

    script = load_demo_script(yaml_file)

    assert isinstance(script, DemoScript)
    assert script.title == "Demonstração do Painel"
    assert script.description == "Roteiro com múltiplas cenas e ações"
    assert script.config.viewport == {"width": 1920, "height": 1080}
    assert script.config.fps == 30
    assert script.config.voice_id == "bf91cd6a"
    assert len(script.scenes) == 2

    # Scene 1 checks
    s1 = script.scenes[0]
    assert s1.id == "scene_01"
    assert s1.title == "Abertura"
    assert s1.pause_after == 1.0
    assert len(s1.actions) == 2
    assert s1.actions[0].action == "navigate"
    assert s1.actions[0].url == "http://localhost:5173"
    assert s1.actions[1].action == "wait"
    assert s1.actions[1].seconds == 1.5

    # Scene 2 checks
    s2 = script.scenes[1]
    assert s2.id == "scene_02"
    assert s2.pause_after == 0.8  # default
    assert len(s2.actions) == 2
    assert s2.actions[0].action == "type"
    assert s2.actions[0].selector == "input[name='search']"
    assert s2.actions[0].text == "Relatório"
    assert s2.actions[1].action == "click"
    assert s2.actions[1].selector == "#btn-filtrar"


def test_load_valid_steps_script(tmp_path: Path):
    """Verifies that step-based YAML (flat format) is parsed and converted to scenes."""
    yaml_content = """
version: "1.0"
title: "Demonstração Step-based"
base_url: "http://localhost:5173"
voice:
  profile_id: "bf91cd6a"
  model: "omnivoice"
  pause_between_steps: 1.2
steps:
  - id: "step_01"
    action: "navigate"
    url: "/"
    narration: "Acessamos a página inicial."
  - id: "step_02"
    action: "click"
    selector: "#btn-login"
    narration: "Clicamos em entrar."
    highlight: false
"""
    yaml_file = tmp_path / "steps_demo.yaml"
    yaml_file.write_text(yaml_content, encoding="utf-8")

    script = load_demo_script(yaml_file)

    assert script.title == "Demonstração Step-based"
    assert script.config.base_url == "http://localhost:5173"
    assert script.config.voice_id == "bf91cd6a"
    assert len(script.scenes) == 2

    sc1 = script.scenes[0]
    assert sc1.id == "step_01"
    assert sc1.narration == "Acessamos a página inicial."
    assert sc1.pause_after == 1.2
    assert len(sc1.actions) == 1
    assert sc1.actions[0].action == "navigate"
    assert sc1.actions[0].url == "/"

    sc2 = script.scenes[1]
    assert sc2.id == "step_02"
    assert sc2.actions[0].action == "click"
    assert sc2.actions[0].selector == "#btn-login"
    assert sc2.actions[0].highlight is False


def test_default_config_values(tmp_path: Path):
    """Verifies that DemoConfig provides sensible defaults when fields are omitted."""
    minimal_yaml = """
title: "Minimal Demo"
scenes:
  - id: "s1"
    narration: "Texto de teste"
"""
    yaml_file = tmp_path / "minimal.yaml"
    yaml_file.write_text(minimal_yaml, encoding="utf-8")

    script = load_demo_script(yaml_file)

    assert script.config.viewport == {"width": 1920, "height": 1080}
    assert script.config.fps == 25
    assert script.config.voice_id == "bf91cd6a"
    assert script.config.model == "omnivoice"
    assert script.config.output_dir == "output"
    assert script.config.temp_dir == ".temp_demo"
    assert script.scenes[0].pause_after == 0.8
    assert script.scenes[0].actions == []


def test_missing_file_raises_error():
    """Verifies that non-existent YAML file raises FileNotFoundError."""
    with pytest.raises(FileNotFoundError):
        load_demo_script("/non/existent/path/demo.yaml")


def test_empty_or_non_dict_yaml_raises_error(tmp_path: Path):
    """Verifies that empty file or non-dict YAML root raises SchemaValidationError."""
    empty_file = tmp_path / "empty.yaml"
    empty_file.write_text("", encoding="utf-8")

    with pytest.raises(SchemaValidationError, match="Demo script root must be a YAML dictionary"):
        load_demo_script(empty_file)

    list_file = tmp_path / "list.yaml"
    list_file.write_text("- item1\n- item2", encoding="utf-8")

    with pytest.raises(SchemaValidationError, match="Demo script root must be a YAML dictionary"):
        load_demo_script(list_file)


def test_syntax_error_yaml_raises_error(tmp_path: Path):
    """Verifies that malformed YAML syntax raises SchemaValidationError."""
    bad_yaml = tmp_path / "bad.yaml"
    bad_yaml.write_text("title: [unclosed list", encoding="utf-8")

    with pytest.raises(SchemaValidationError, match="YAML parsing error"):
        load_demo_script(bad_yaml)


def test_missing_title_raises_error(tmp_path: Path):
    """Verifies that missing title raises SchemaValidationError."""
    no_title_yaml = tmp_path / "no_title.yaml"
    no_title_yaml.write_text("scenes: [{id: s1, narration: test}]", encoding="utf-8")

    with pytest.raises(SchemaValidationError, match="missing required 'title'"):
        load_demo_script(no_title_yaml)


def test_missing_scenes_and_steps_raises_error(tmp_path: Path):
    """Verifies that YAML without scenes or steps raises SchemaValidationError."""
    no_scenes_yaml = tmp_path / "no_scenes.yaml"
    no_scenes_yaml.write_text("title: 'Teste Sem Cenas'", encoding="utf-8")

    with pytest.raises(SchemaValidationError, match="must contain either 'scenes' or 'steps'"):
        load_demo_script(no_scenes_yaml)


def test_invalid_action_type_raises_error(tmp_path: Path):
    """Verifies that invalid action names raise SchemaValidationError."""
    bad_action_yaml = tmp_path / "bad_action.yaml"
    bad_action_yaml.write_text("""
title: "Invalid Action Demo"
scenes:
  - id: "s1"
    narration: "Ação inválida"
    actions:
      - action: "explode_database"
""", encoding="utf-8")

    with pytest.raises(SchemaValidationError, match="Invalid action 'explode_database'"):
        load_demo_script(bad_action_yaml)


def test_dump_and_reload_roundtrip(tmp_path: Path):
    """Verifies that serialization via dump_demo_script preserves script data on reload."""
    original = DemoScript(
        title="Roundtrip Test",
        description="Descrição de teste",
        config=DemoConfig(
            viewport={"width": 1280, "height": 720},
            fps=30,
            voice_id="custom_voice",
            model="custom_model",
            base_url="http://test.local",
            output_dir="out_custom",
            temp_dir=".tmp_custom",
        ),
        scenes=[
            Scene(
                id="scene_01",
                title="Cena 1",
                narration="Texto gravado na primeira cena.",
                pause_after=1.5,
                actions=[
                    Action(
                        action="navigate",
                        url="http://test.local/login",
                    ),
                    Action(
                        action="type",
                        selector="input#username",
                        text="admin",
                        highlight=True,
                    ),
                ],
            )
        ],
    )

    dump_path = tmp_path / "serialized.yaml"
    dump_demo_script(original, dump_path)

    reloaded = load_demo_script(dump_path)

    assert reloaded.title == original.title
    assert reloaded.description == original.description
    assert reloaded.config.viewport == original.config.viewport
    assert reloaded.config.fps == original.config.fps
    assert reloaded.config.voice_id == original.config.voice_id
    assert len(reloaded.scenes) == 1
    assert reloaded.scenes[0].id == "scene_01"
    assert reloaded.scenes[0].narration == original.scenes[0].narration
    assert reloaded.scenes[0].pause_after == 1.5
    assert len(reloaded.scenes[0].actions) == 2
    assert reloaded.scenes[0].actions[0].action == "navigate"
    assert reloaded.scenes[0].actions[1].selector == "input#username"


def test_aliases_work_identically(tmp_path: Path):
    """Verifies that load_and_validate_demo_yaml and save_demo_yaml aliases work."""
    yaml_file = tmp_path / "alias_test.yaml"
    yaml_file.write_text("title: 'Alias'\nscenes: [{id: s1, narration: n1}]", encoding="utf-8")

    script = load_and_validate_demo_yaml(yaml_file)
    assert script.title == "Alias"

    out_file = tmp_path / "alias_out.yaml"
    save_demo_yaml(script, out_file)
    assert out_file.exists()


def test_load_sample_demo_yaml():
    """Verifies that the reference sample_demo.yaml loads and validates cleanly."""
    sample_path = Path(__file__).resolve().parent.parent / "examples" / "sample_demo.yaml"
    if sample_path.exists():
        script = load_demo_script(sample_path)
        assert script.title == "Demonstração do Painel Administrativo VoiceStudio"
        assert len(script.scenes) == 3
        assert script.config.viewport["width"] == 1920
        assert script.config.viewport["height"] == 1080
        assert script.config.voice_id == "bf91cd6a"
        assert script.scenes[0].actions[0].action == "navigate"
        assert script.scenes[1].actions[0].action == "type"
        assert script.scenes[1].actions[1].action == "click"


def test_action_coordinates_and_keys(tmp_path: Path):
    """Verifies that scroll (x, y) and key press actions parse properly."""
    content = """
title: "Advanced Actions Demo"
scenes:
  - id: "sc_advanced"
    narration: "Ações avançadas de teclado e rolagem."
    actions:
      - action: "scroll"
        x: 0
        y: 400
      - action: "press"
        key: "Enter"
      - action: "select"
        selector: "#dropdown"
        value: "op_01"
      - action: "screenshot"
"""
    yaml_file = tmp_path / "advanced.yaml"
    yaml_file.write_text(content, encoding="utf-8")

    script = load_demo_script(yaml_file)
    acts = script.scenes[0].actions
    assert len(acts) == 4
    assert acts[0].action == "scroll"
    assert acts[0].x == 0
    assert acts[0].y == 400
    assert acts[1].action == "press"
    assert acts[1].key == "Enter"
    assert acts[2].action == "select"
    assert acts[2].selector == "#dropdown"
    assert acts[2].value == "op_01"
    assert acts[3].action == "screenshot"


# ---------------------------------------------------------------------------
# 5. Adversarial Bounds & Type Hardening Regression Tests
# ---------------------------------------------------------------------------


def test_negative_bounds_validation_raises_error(tmp_path: Path):
    """Verifies that negative durations, negative pauses, and invalid bounds raise SchemaValidationError."""
    # Negative pause_after in scene
    bad_scene_pause = tmp_path / "bad_scene_pause.yaml"
    bad_scene_pause.write_text("title: 'T'\nscenes: [{id: 's1', narration: 'N', pause_after: -1.0}]", encoding="utf-8")
    with pytest.raises(SchemaValidationError, match="must be >= 0"):
        load_demo_script(bad_scene_pause)

    # Negative seconds in wait action
    bad_wait = tmp_path / "bad_wait.yaml"
    bad_wait.write_text("""
title: "T"
scenes:
  - id: "s1"
    narration: "N"
    actions:
      - action: "wait"
        seconds: -5.0
""", encoding="utf-8")
    with pytest.raises(SchemaValidationError, match="must be >= 0"):
        load_demo_script(bad_wait)

    # Invalid FPS (0 and negative)
    bad_fps = tmp_path / "bad_fps.yaml"
    bad_fps.write_text("title: 'T'\nconfig: {fps: 0}\nscenes: [{id: 's1', narration: 'N'}]", encoding="utf-8")
    with pytest.raises(SchemaValidationError, match="must be >= 1"):
        load_demo_script(bad_fps)

    # Viewport non-positive dimensions
    bad_vp = tmp_path / "bad_vp.yaml"
    bad_vp.write_text("title: 'T'\nconfig: {viewport: {width: 0, height: 1080}}\nscenes: [{id: 's1', narration: 'N'}]", encoding="utf-8")
    with pytest.raises(SchemaValidationError, match="must be >= 1"):
        load_demo_script(bad_vp)


def test_duplicate_scene_ids_raises_error(tmp_path: Path):
    """Verifies that duplicate scene or step IDs raise SchemaValidationError."""
    dup_yaml = tmp_path / "dup.yaml"
    dup_yaml.write_text("""
title: "Duplicate IDs"
scenes:
  - id: "scene_01"
    narration: "Primeira cena"
  - id: "scene_01"
    narration: "Segunda cena com id repetido"
""", encoding="utf-8")
    with pytest.raises(SchemaValidationError, match="Duplicate scene id 'scene_01' detected"):
        load_demo_script(dup_yaml)


def test_action_mandatory_parameters_validation(tmp_path: Path):
    """Verifies that incomplete action definitions raise descriptive SchemaValidationError."""
    # wait without seconds
    f1 = tmp_path / "f1.yaml"
    f1.write_text("title: 'T'\nscenes: [{id: 's1', narration: 'N', actions: [{action: 'wait'}]}]", encoding="utf-8")
    with pytest.raises(SchemaValidationError, match="'wait' action in 's1' requires 'seconds' parameter"):
        load_demo_script(f1)

    # navigate without url
    f2 = tmp_path / "f2.yaml"
    f2.write_text("title: 'T'\nscenes: [{id: 's1', narration: 'N', actions: [{action: 'navigate'}]}]", encoding="utf-8")
    with pytest.raises(SchemaValidationError, match="'navigate' action in 's1' requires 'url' parameter"):
        load_demo_script(f2)

    # click without selector
    f3 = tmp_path / "f3.yaml"
    f3.write_text("title: 'T'\nscenes: [{id: 's1', narration: 'N', actions: [{action: 'click'}]}]", encoding="utf-8")
    with pytest.raises(SchemaValidationError, match="'click' action in 's1' requires 'selector' parameter"):
        load_demo_script(f3)

    # type without text
    f4 = tmp_path / "f4.yaml"
    f4.write_text("title: 'T'\nscenes: [{id: 's1', narration: 'N', actions: [{action: 'type', selector: '#in'}]}]", encoding="utf-8")
    with pytest.raises(SchemaValidationError, match="'type' action in 's1' requires 'text' parameter"):
        load_demo_script(f4)


def test_safe_numeric_type_error_wrapping(tmp_path: Path):
    """Verifies that raw ValueError and TypeError are wrapped into SchemaValidationError."""
    bad_types = tmp_path / "bad_types.yaml"
    bad_types.write_text("""
title: "Type Wrapping Test"
scenes:
  - id: "s1"
    narration: "N"
    actions:
      - action: "wait"
        seconds: "five"
""", encoding="utf-8")
    with pytest.raises(SchemaValidationError, match="Invalid numeric value for 'seconds' in 's1'"):
        load_demo_script(bad_types)

