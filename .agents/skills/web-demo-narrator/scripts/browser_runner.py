"""
Playwright Browser Runner with Audio-Driven Cadence and Spotlight Highlight.
Implements the contract defined in PROJECT.md § Milestone 3.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import time
from typing import Optional, Union, Dict, Any

from playwright.sync_api import sync_playwright, Page, BrowserContext

from scripts.schema import DemoScript, DemoConfig, Scene, Action


class BrowserRunnerError(RuntimeError):
    """Raised when browser automation or recording fails."""
    pass


class BrowserRunner:
    """Executes demo web actions in Chromium with audio-driven timing and video capture."""

    def __init__(
        self,
        config: Optional[DemoConfig] = None,
        headless: bool = True,
        action_timeout: float = 10000.0,
    ):
        self.config = config or DemoConfig()
        self.headless = headless
        self.action_timeout = action_timeout

    def _resolve_url(self, raw_url: str) -> str:
        """Resolves raw URLs, converting local file paths to file:// URIs."""
        if raw_url.startswith(("http://", "https://", "file://", "about:")):
            return raw_url

        # Check local path relative to current dir or script dir
        p = Path(raw_url)
        if p.exists():
            return p.resolve().as_uri()

        # If base_url is configured, prepend it
        base = self.config.base_url.rstrip("/")
        if base.startswith("http"):
            rel = raw_url.lstrip("/")
            return f"{base}/{rel}"

        return raw_url

    def _inject_highlight(self, page: Page, selector: str) -> None:
        """Injects a glowing pulse outline on the target element to guide viewer focus."""
        js_code = """
        (sel) => {
            try {
                const el = document.querySelector(sel);
                if (el) {
                    el.scrollIntoView({ behavior: 'smooth', block: 'center' });
                    el.style.transition = 'box-shadow 0.25s ease-in-out, transform 0.25s ease-in-out';
                    el.style.boxShadow = '0 0 0 4px #3b82f6, 0 0 20px rgba(59, 130, 246, 0.6)';
                    el.setAttribute('data-demo-highlighted', 'true');
                }
            } catch (e) {}
        }
        """
        try:
            page.evaluate(js_code, selector)
        except Exception:
            pass

    def _clear_highlights(self, page: Page) -> None:
        """Removes highlight styling from previously highlighted elements."""
        js_code = """
        () => {
            try {
                document.querySelectorAll('[data-demo-highlighted="true"]').forEach((el) => {
                    el.style.boxShadow = '';
                    el.style.transition = '';
                    el.removeAttribute('data-demo-highlighted');
                });
            } catch (e) {}
        }
        """
        try:
            page.evaluate(js_code)
        except Exception:
            pass

    def _execute_action(self, page: Page, action: Action, parent_id: str) -> None:
        """Executes a single declarative action on the Playwright page."""
        act_name = action.action

        if act_name == "navigate":
            if not action.url:
                raise BrowserRunnerError(f"'navigate' action in '{parent_id}' missing 'url'")
            resolved_url = self._resolve_url(action.url)
            page.goto(resolved_url, wait_until="domcontentloaded", timeout=self.action_timeout)

        elif act_name == "click":
            if not action.selector:
                raise BrowserRunnerError(f"'click' action in '{parent_id}' missing 'selector'")
            loc = page.locator(action.selector).first
            loc.wait_for(state="visible", timeout=self.action_timeout)
            if action.highlight:
                self._inject_highlight(page, action.selector)
                page.wait_for_timeout(200)
            loc.click(timeout=self.action_timeout)

        elif act_name in {"type", "fill"}:
            if not action.selector:
                raise BrowserRunnerError(f"'{act_name}' action in '{parent_id}' missing 'selector'")
            text_val = action.text or ""
            loc = page.locator(action.selector).first
            loc.wait_for(state="visible", timeout=self.action_timeout)
            if action.highlight:
                self._inject_highlight(page, action.selector)
                page.wait_for_timeout(200)
            loc.click(timeout=self.action_timeout)
            # Type with a natural human delay between keystrokes
            loc.fill("")
            loc.type(text_val, delay=40)

        elif act_name == "wait":
            sec = action.seconds or 1.0
            page.wait_for_timeout(sec * 1000)

        elif act_name == "select":
            if not action.selector:
                raise BrowserRunnerError(f"'select' action in '{parent_id}' missing 'selector'")
            loc = page.locator(action.selector).first
            loc.wait_for(state="visible", timeout=self.action_timeout)
            if action.highlight:
                self._inject_highlight(page, action.selector)
            loc.select_option(value=action.value or action.text)

        elif act_name == "hover":
            if not action.selector:
                raise BrowserRunnerError(f"'hover' action in '{parent_id}' missing 'selector'")
            loc = page.locator(action.selector).first
            loc.wait_for(state="visible", timeout=self.action_timeout)
            if action.highlight:
                self._inject_highlight(page, action.selector)
            loc.hover()

        elif act_name == "press":
            if not action.key:
                raise BrowserRunnerError(f"'press' action in '{parent_id}' missing 'key'")
            page.keyboard.press(action.key)

        elif act_name == "scroll":
            x = action.x or 0
            y = action.y or 0
            page.evaluate(f"window.scrollBy({x}, {y})")

        elif act_name == "screenshot":
            # Just trigger layout flush/delay
            page.wait_for_timeout(200)

    def record_demo(
        self,
        script: DemoScript,
        output_video_path: Union[str, Path],
    ) -> str:
        """Executes all scenes with audio-driven timing & highlight, returns path to recorded video."""
        out_path = Path(output_video_path)
        out_path.parent.mkdir(parents=True, exist_ok=True)

        temp_dir = Path(script.config.temp_dir) / "video_raw"
        if temp_dir.exists():
            shutil.rmtree(temp_dir)
        temp_dir.mkdir(parents=True, exist_ok=True)

        vp_width = script.config.viewport.get("width", 1920)
        vp_height = script.config.viewport.get("height", 1080)

        timeline_log: list[dict[str, Any]] = []

        with sync_playwright() as p:
            browser = p.chromium.launch(headless=self.headless)
            context: BrowserContext = browser.new_context(
                viewport={"width": vp_width, "height": vp_height},
                record_video_dir=str(temp_dir),
                record_video_size={"width": vp_width, "height": vp_height},
            )
            page = context.new_page()
            page.set_default_timeout(self.action_timeout)

            total_start_time = time.time()

            try:
                for scene_idx, scene in enumerate(script.scenes):
                    scene_start = time.time()
                    self._clear_highlights(page)

                    # 1. Execute all actions defined for this scene
                    action_start = time.time()
                    for act_idx, act in enumerate(scene.actions):
                        self._execute_action(page, act, scene.id)
                    action_duration = time.time() - action_start

                    # 2. Audio-driven cadence calculation
                    audio_dur = float(scene.audio_duration or 0.0)
                    pause_after = float(scene.pause_after or 0.8)

                    # Frame hold time: ensure screen stays visible while voice speaks
                    remaining_hold = max(0.0, audio_dur - action_duration) + pause_after
                    if remaining_hold > 0:
                        page.wait_for_timeout(remaining_hold * 1000)

                    scene_total_time = time.time() - scene_start
                    timeline_log.append({
                        "scene_id": scene.id,
                        "actions_count": len(scene.actions),
                        "action_duration": round(action_duration, 3),
                        "audio_duration": round(audio_dur, 3),
                        "pause_after": round(pause_after, 3),
                        "scene_total_time": round(scene_total_time, 3),
                    })

                # Grace buffer before closing context
                page.wait_for_timeout(500)

            finally:
                video_obj = page.video
                context.close()
                browser.close()

            # Retrieve recorded video file
            recorded_file = None
            if video_obj:
                recorded_path = video_obj.path()
                if Path(recorded_path).exists():
                    recorded_file = Path(recorded_path)

            if not recorded_file:
                # Find any webm in temp_dir
                candidates = list(temp_dir.glob("*.webm"))
                if candidates:
                    recorded_file = candidates[0]

            if not recorded_file or not recorded_file.exists():
                raise BrowserRunnerError("Playwright closed without producing a video recording.")

            # Copy recorded file to final output_video_path
            shutil.copy2(recorded_file, out_path)

            # Write timeline report
            timeline_path = Path(script.config.temp_dir) / "timeline.json"
            with open(timeline_path, "w", encoding="utf-8") as f:
                json.dump(
                    {
                        "total_duration": round(time.time() - total_start_time, 3),
                        "scenes": timeline_log,
                    },
                    f,
                    indent=2,
                )

        return str(out_path)
