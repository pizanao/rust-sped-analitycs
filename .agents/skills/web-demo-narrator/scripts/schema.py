"""
Schema and data models for web-demo-narrator scripts.
Strictly implements the contracts defined in PROJECT.md § Interface Contracts.
"""

from __future__ import annotations

from dataclasses import dataclass, field, asdict
from pathlib import Path
import math
from typing import List, Optional, Literal, Dict, Any, Union
import yaml

ActionType = Literal[
    "navigate",
    "click",
    "type",
    "wait",
    "select",
    "screenshot",
    "press",
    "fill",
    "hover",
    "scroll",
]

VALID_ACTION_TYPES = {
    "navigate",
    "click",
    "type",
    "wait",
    "select",
    "screenshot",
    "press",
    "fill",
    "hover",
    "scroll",
}


class SchemaValidationError(ValueError):
    """Raised when a demo script YAML fails validation."""
    pass


@dataclass
class Action:
    action: ActionType
    selector: Optional[str] = None
    text: Optional[str] = None
    url: Optional[str] = None
    seconds: Optional[float] = None
    key: Optional[str] = None
    value: Optional[str] = None
    highlight: bool = True
    x: Optional[int] = None
    y: Optional[int] = None
    direction: Optional[str] = None


@dataclass
class Scene:
    id: str
    title: str
    narration: str
    actions: List[Action] = field(default_factory=list)
    pause_after: float = 0.8
    audio_path: Optional[str] = None
    audio_duration: Optional[float] = None


@dataclass
class DemoConfig:
    viewport: Dict[str, int] = field(default_factory=lambda: {"width": 1920, "height": 1080})
    fps: int = 25
    voice_id: str = "bf91cd6a"
    model: str = "omnivoice"
    base_url: str = "http://localhost:3900"
    output_dir: str = "output"
    temp_dir: str = ".temp_demo"


@dataclass
class DemoScript:
    title: str
    description: str
    config: DemoConfig
    scenes: List[Scene]


def _safe_float(val: Any, field_name: str, context: str, min_val: Optional[float] = None) -> float:
    """Converts a value to float, ensuring bounds and wrapping raw conversion exceptions."""
    try:
        f_val = float(val)
    except (ValueError, TypeError) as exc:
        raise SchemaValidationError(f"Invalid numeric value for '{field_name}' in '{context}': {val}") from exc
    if min_val is not None and f_val < min_val:
        raise SchemaValidationError(f"Field '{field_name}' in '{context}' must be >= {min_val}, got {f_val}")
    return f_val


def _safe_int(
    val: Any,
    field_name: str,
    context: str,
    min_val: Optional[int] = None,
    max_val: Optional[int] = None,
) -> int:
    """Converts a value to int, ensuring bounds and wrapping raw conversion exceptions."""
    try:
        i_val = int(val)
    except (ValueError, TypeError) as exc:
        raise SchemaValidationError(f"Invalid integer for '{field_name}' in '{context}': {val}") from exc
    if min_val is not None and i_val < min_val:
        raise SchemaValidationError(f"Field '{field_name}' in '{context}' must be >= {min_val}, got {i_val}")
    if max_val is not None and i_val > max_val:
        raise SchemaValidationError(f"Field '{field_name}' in '{context}' must be <= {max_val}, got {i_val}")
    return i_val


def _parse_action(raw_act: Dict[str, Any], index: int, parent_id: str) -> Action:
    if not isinstance(raw_act, dict):
        raise SchemaValidationError(f"Action at index {index} in '{parent_id}' must be a dictionary.")

    action_name = raw_act.get("action")
    if not action_name:
        raise SchemaValidationError(f"Action at index {index} in '{parent_id}' is missing required 'action' field.")

    if not isinstance(action_name, str) or action_name not in VALID_ACTION_TYPES:
        raise SchemaValidationError(
            f"Invalid action '{action_name}' in '{parent_id}'. Must be one of: {sorted(VALID_ACTION_TYPES)}"
        )

    # Validate action-specific mandatory parameters
    if action_name == "navigate":
        url = raw_act.get("url")
        if not url or not isinstance(url, str) or not url.strip():
            raise SchemaValidationError(f"'navigate' action in '{parent_id}' requires 'url' parameter.")

    elif action_name in {"click", "hover"}:
        selector = raw_act.get("selector")
        if not selector or not isinstance(selector, str) or not selector.strip():
            raise SchemaValidationError(f"'{action_name}' action in '{parent_id}' requires 'selector' parameter.")

    elif action_name in {"type", "fill"}:
        selector = raw_act.get("selector")
        if not selector or not isinstance(selector, str) or not selector.strip():
            raise SchemaValidationError(f"'{action_name}' action in '{parent_id}' requires 'selector' parameter.")
        if raw_act.get("text") is None:
            raise SchemaValidationError(f"'{action_name}' action in '{parent_id}' requires 'text' parameter.")

    elif action_name == "wait":
        if raw_act.get("seconds") is None:
            raise SchemaValidationError(f"'wait' action in '{parent_id}' requires 'seconds' parameter.")

    elif action_name == "press":
        key = raw_act.get("key")
        if not key or not isinstance(key, str) or not key.strip():
            raise SchemaValidationError(f"'press' action in '{parent_id}' requires 'key' parameter.")

    elif action_name == "select":
        selector = raw_act.get("selector")
        if not selector or not isinstance(selector, str) or not selector.strip():
            raise SchemaValidationError(f"'select' action in '{parent_id}' requires 'selector' parameter.")
        if raw_act.get("value") is None:
            raise SchemaValidationError(f"'select' action in '{parent_id}' requires 'value' parameter.")

    elif action_name == "scroll":
        if raw_act.get("x") is None and raw_act.get("y") is None:
            raise SchemaValidationError(f"'scroll' action in '{parent_id}' requires 'x' or 'y' coordinate.")

    seconds = (
        _safe_float(raw_act["seconds"], "seconds", parent_id, min_val=0.0)
        if raw_act.get("seconds") is not None
        else None
    )
    x = _safe_int(raw_act["x"], "x", parent_id) if raw_act.get("x") is not None else None
    y = _safe_int(raw_act["y"], "y", parent_id) if raw_act.get("y") is not None else None

    return Action(
        action=action_name,  # type: ignore[arg-type]
        selector=str(raw_act["selector"]) if raw_act.get("selector") is not None else None,
        text=str(raw_act["text"]) if raw_act.get("text") is not None else None,
        url=str(raw_act["url"]) if raw_act.get("url") is not None else None,
        seconds=seconds,
        key=str(raw_act["key"]) if raw_act.get("key") is not None else None,
        value=str(raw_act["value"]) if raw_act.get("value") is not None else None,
        highlight=bool(raw_act.get("highlight", True)),
        x=x,
        y=y,
    )


def _parse_config(raw_cfg: Optional[Dict[str, Any]], raw_root: Dict[str, Any]) -> DemoConfig:
    cfg = DemoConfig()
    if raw_cfg is not None and not isinstance(raw_cfg, dict):
        raise SchemaValidationError(f"'config' must be a dictionary, got {type(raw_cfg).__name__}")
    raw_cfg = raw_cfg or {}

    # Check top-level aliases from raw_root
    if "base_url" in raw_root and "base_url" not in raw_cfg:
        cfg.base_url = str(raw_root["base_url"])
    if "output" in raw_root and "output_dir" not in raw_cfg:
        cfg.output_dir = str(Path(raw_root["output"]).parent) or "output"

    # Viewport
    vp = raw_cfg.get("viewport") or raw_root.get("viewport")
    if isinstance(vp, dict):
        w = _safe_int(vp.get("width", 1920), "viewport.width", "config", min_val=1)
        h = _safe_int(vp.get("height", 1080), "viewport.height", "config", min_val=1)
        cfg.viewport = {
            "width": w,
            "height": h,
        }
    elif vp is not None:
        raise SchemaValidationError(f"'viewport' must be a dictionary, got {type(vp).__name__}")

    # Voice config sub-dict
    voice = raw_cfg.get("voice") or raw_root.get("voice")
    if isinstance(voice, dict):
        if "profile_id" in voice:
            cfg.voice_id = str(voice["profile_id"])
        elif "voice_id" in voice:
            cfg.voice_id = str(voice["voice_id"])
        if "model" in voice:
            cfg.model = str(voice["model"])
        if "backend_url" in voice:
            cfg.base_url = str(voice["backend_url"])

    # Direct config overrides
    if "fps" in raw_cfg:
        cfg.fps = _safe_int(raw_cfg["fps"], "fps", "config", min_val=1, max_val=120)
    elif "fps" in raw_root:
        cfg.fps = _safe_int(raw_root["fps"], "fps", "config", min_val=1, max_val=120)
    if "voice_id" in raw_cfg:
        cfg.voice_id = str(raw_cfg["voice_id"])
    if "model" in raw_cfg:
        cfg.model = str(raw_cfg["model"])
    if "base_url" in raw_cfg:
        cfg.base_url = str(raw_cfg["base_url"])
    if "output_dir" in raw_cfg:
        cfg.output_dir = str(raw_cfg["output_dir"])
    if "temp_dir" in raw_cfg:
        cfg.temp_dir = str(raw_cfg["temp_dir"])

    return cfg


def load_demo_script(yaml_path: Union[str, Path]) -> DemoScript:
    """Loads a YAML file from disk, parses it, validates all fields, and returns DemoScript.

    Raises:
        FileNotFoundError: If the YAML file does not exist.
        SchemaValidationError: If YAML syntax is invalid or required fields are missing.
    """
    path = Path(yaml_path)
    if not path.is_file():
        raise FileNotFoundError(f"Demo script not found: {path}")

    try:
        with open(path, "r", encoding="utf-8") as f:
            data = yaml.safe_load(f)
    except yaml.YAMLError as exc:
        raise SchemaValidationError(f"YAML parsing error in {path}: {exc}") from exc

    if not isinstance(data, dict):
        raise SchemaValidationError(f"Demo script root must be a YAML dictionary, got {type(data).__name__}")

    title = data.get("title")
    if not title or not isinstance(title, str):
        raise SchemaValidationError("Demo script is missing required 'title' string.")

    description = data.get("description", "")
    if not isinstance(description, str):
        description = str(description)

    config = _parse_config(data.get("config"), data)

    scenes: List[Scene] = []
    seen_ids: set[str] = set()

    # Check if format uses 'scenes' or 'steps'
    if "scenes" in data:
        raw_scenes = data["scenes"]
        if not isinstance(raw_scenes, list) or len(raw_scenes) == 0:
            raise SchemaValidationError("'scenes' must be a non-empty list of scene definitions.")

        for i, s in enumerate(raw_scenes):
            if not isinstance(s, dict):
                raise SchemaValidationError(f"Scene at index {i} must be a dictionary.")

            s_id = s.get("id")
            if not s_id or not isinstance(s_id, str):
                raise SchemaValidationError(f"Scene at index {i} is missing required 'id' string.")

            if s_id in seen_ids:
                raise SchemaValidationError(f"Duplicate scene id '{s_id}' detected.")
            seen_ids.add(s_id)

            s_title = s.get("title", s_id)
            s_narration = s.get("narration")
            if s_narration is None or not isinstance(s_narration, str):
                raise SchemaValidationError(f"Scene '{s_id}' is missing required 'narration' string.")

            actions_list: List[Action] = []
            if "actions" in s:
                raw_actions = s["actions"]
                if not isinstance(raw_actions, list):
                    raise SchemaValidationError(f"Scene '{s_id}' 'actions' must be a list.")
                for a_idx, a in enumerate(raw_actions):
                    actions_list.append(_parse_action(a, a_idx, s_id))

            pause_after = _safe_float(s.get("pause_after", 0.8), "pause_after", s_id, min_val=0.0)
            audio_path = s.get("audio_path")
            audio_duration = (
                _safe_float(s["audio_duration"], "audio_duration", s_id, min_val=0.0)
                if s.get("audio_duration") is not None
                else None
            )

            scenes.append(
                Scene(
                    id=s_id,
                    title=str(s_title),
                    narration=s_narration.strip(),
                    actions=actions_list,
                    pause_after=pause_after,
                    audio_path=str(audio_path) if audio_path is not None else None,
                    audio_duration=audio_duration,
                )
            )

    elif "steps" in data:
        raw_steps = data["steps"]
        if not isinstance(raw_steps, list) or len(raw_steps) == 0:
            raise SchemaValidationError("'steps' must be a non-empty list of step definitions.")

        default_pause = 0.8
        voice_block = data.get("voice", {})
        if isinstance(voice_block, dict) and "pause_between_steps" in voice_block:
            default_pause = _safe_float(voice_block["pause_between_steps"], "pause_between_steps", "voice", min_val=0.0)

        for i, st in enumerate(raw_steps):
            if not isinstance(st, dict):
                raise SchemaValidationError(f"Step at index {i} must be a dictionary.")

            st_id = st.get("id")
            if not st_id or not isinstance(st_id, str):
                raise SchemaValidationError(f"Step at index {i} is missing required 'id' string.")

            if st_id in seen_ids:
                raise SchemaValidationError(f"Duplicate scene id '{st_id}' detected.")
            seen_ids.add(st_id)

            st_narration = st.get("narration")
            if st_narration is None or not isinstance(st_narration, str):
                raise SchemaValidationError(f"Step '{st_id}' is missing required 'narration' string.")

            action = _parse_action(st, 0, st_id)
            pause_after = _safe_float(st.get("pause_after", default_pause), "pause_after", st_id, min_val=0.0)

            scenes.append(
                Scene(
                    id=st_id,
                    title=str(st.get("title", st_id)),
                    narration=st_narration.strip(),
                    actions=[action],
                    pause_after=pause_after,
                )
            )
    else:
        raise SchemaValidationError("Demo script must contain either 'scenes' or 'steps' list.")

    return DemoScript(
        title=title,
        description=description,
        config=config,
        scenes=scenes,
    )


def dump_demo_script(script: DemoScript, yaml_path: Union[str, Path]) -> None:
    """Serializes a DemoScript object into clean YAML format and writes to disk."""
    path = Path(yaml_path)
    path.parent.mkdir(parents=True, exist_ok=True)

    data: Dict[str, Any] = {
        "title": script.title,
        "description": script.description,
        "config": {
            "viewport": script.config.viewport,
            "fps": script.config.fps,
            "voice_id": script.config.voice_id,
            "model": script.config.model,
            "base_url": script.config.base_url,
            "output_dir": script.config.output_dir,
            "temp_dir": script.config.temp_dir,
        },
        "scenes": [],
    }

    for sc in script.scenes:
        sc_dict: Dict[str, Any] = {
            "id": sc.id,
            "title": sc.title,
            "narration": sc.narration,
            "pause_after": sc.pause_after,
            "actions": [],
        }
        if sc.audio_path:
            sc_dict["audio_path"] = sc.audio_path
        if sc.audio_duration is not None:
            sc_dict["audio_duration"] = sc.audio_duration

        for act in sc.actions:
            act_dict: Dict[str, Any] = {"action": act.action}
            if act.selector is not None:
                act_dict["selector"] = act.selector
            if act.text is not None:
                act_dict["text"] = act.text
            if act.url is not None:
                act_dict["url"] = act.url
            if act.seconds is not None:
                act_dict["seconds"] = act.seconds
            if act.key is not None:
                act_dict["key"] = act.key
            if act.value is not None:
                act_dict["value"] = act.value
            if not act.highlight:
                act_dict["highlight"] = act.highlight
            if act.x is not None:
                act_dict["x"] = act.x
            if act.y is not None:
                act_dict["y"] = act.y
            sc_dict["actions"].append(act_dict)

        data["scenes"].append(sc_dict)

    with open(path, "w", encoding="utf-8") as f:
        yaml.dump(data, f, sort_keys=False, allow_unicode=True)


# Aliases for compatibility
load_and_validate_demo_yaml = load_demo_script
save_demo_yaml = dump_demo_script
