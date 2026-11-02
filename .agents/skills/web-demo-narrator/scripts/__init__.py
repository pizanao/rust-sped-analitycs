"""
Web Demo Narrator core scripts package.
"""

from .schema import (
    Action,
    ActionType,
    Scene,
    DemoConfig,
    DemoScript,
    SchemaValidationError,
    load_demo_script,
    dump_demo_script,
    load_and_validate_demo_yaml,
    save_demo_yaml,
)
from .humanizer import (
    SpeechHumanizer,
    humanize_text,
    humanize_script,
    format_diff_table,
)

__all__ = [
    "Action",
    "ActionType",
    "Scene",
    "DemoConfig",
    "DemoScript",
    "SchemaValidationError",
    "load_demo_script",
    "dump_demo_script",
    "load_and_validate_demo_yaml",
    "save_demo_yaml",
    "SpeechHumanizer",
    "humanize_text",
    "humanize_script",
    "format_diff_table",
]
