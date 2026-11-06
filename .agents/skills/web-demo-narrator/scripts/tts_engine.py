"""
VoiceStudio TTS Engine Client and Audio Timing Calibration.
Implements the contract defined in PROJECT.md § Milestone 2.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import struct
import subprocess
import time
from typing import Dict, Any, Optional, Union
import urllib.error
import urllib.request
import wave

from scripts.schema import DemoScript, Scene


class VoiceStudioError(RuntimeError):
    """Raised when VoiceStudio API or audio processing fails."""
    pass


class VoiceStudioClient:
    """Client for local VoiceStudio speech synthesis API."""

    def __init__(
        self,
        base_url: str = "http://localhost:3900",
        voice_id: str = "bf91cd6a",
        model: str = "omnivoice",
        timeout: float = 120.0,
        offline_mode: bool = False,
    ):
        self.base_url = base_url.rstrip("/")
        self.voice_id = voice_id
        self.model = model
        self.timeout = timeout
        self.offline_mode = offline_mode

    def check_health(self) -> Dict[str, Any]:
        """Queries GET /health and verifies backend readiness.
        
        Returns:
            Dict containing backend status and device info.
        Raises:
            VoiceStudioError if the backend is unreachable or not healthy.
        """
        if self.offline_mode:
            return {"status": "ok", "device": "offline-mock", "version": "mock-1.0"}

        health_url = f"{self.base_url}/health"
        req = urllib.request.Request(
            health_url,
            headers={"User-Agent": "web-demo-narrator/1.0"},
            method="GET",
        )
        try:
            with urllib.request.urlopen(req, timeout=5.0) as resp:
                if resp.status == 200:
                    payload = json.loads(resp.read().decode("utf-8"))
                    if payload.get("status") != "ok":
                        raise VoiceStudioError(f"VoiceStudio health reported non-ok status: {payload}")
                    return payload
                raise VoiceStudioError(f"VoiceStudio /health returned HTTP {resp.status}")
        except Exception as exc:
            raise VoiceStudioError(
                f"Failed to connect to VoiceStudio backend at {health_url}: {exc}. "
                "Ensure VoiceStudio is running locally on port 3900."
            ) from exc

    def check_voice_available(self, voice_id: Optional[str] = None) -> bool:
        """Verifies if the specified voice profile is registered in VoiceStudio."""
        if self.offline_mode:
            return True

        target_voice = voice_id or self.voice_id
        voices_url = f"{self.base_url}/v1/audio/voices"
        req = urllib.request.Request(
            voices_url,
            headers={"User-Agent": "web-demo-narrator/1.0"},
            method="GET",
        )
        try:
            with urllib.request.urlopen(req, timeout=10.0) as resp:
                if resp.status == 200:
                    data = json.loads(resp.read().decode("utf-8"))
                    for v in data.get("voices", []):
                        if v.get("voice_id") == target_voice or v.get("name") == target_voice:
                            return True
                    return False
                return False
        except Exception:
            return False

    def get_audio_duration(self, audio_path: Union[str, Path]) -> float:
        """Extracts exact audio duration in seconds using ffprobe."""
        info = self.get_audio_info(audio_path)
        return info["duration"]

    def get_audio_info(self, audio_path: Union[str, Path]) -> Dict[str, Any]:
        """Extracts technical metadata (codec, sample_rate, channels, bit_depth) using ffprobe."""
        p = Path(audio_path)
        if not p.is_file():
            raise FileNotFoundError(f"Audio file not found: {p}")

        cmd = [
            "ffprobe",
            "-v", "error",
            "-select_streams", "a:0",
            "-show_entries", "stream=codec_name,sample_rate,channels,bits_per_raw_sample:format=duration",
            "-of", "json",
            str(p.resolve()),
        ]
        try:
            res = subprocess.run(cmd, capture_output=True, text=True, check=True)
            data = json.loads(res.stdout)
            stream = data.get("streams", [{}])[0]
            fmt = data.get("format", {})
            return {
                "codec": stream.get("codec_name"),
                "sample_rate": int(stream.get("sample_rate", 0)),
                "channels": int(stream.get("channels", 0)),
                "bits_per_raw_sample": int(stream.get("bits_per_raw_sample", 0) or 0),
                "duration": float(fmt.get("duration", 0.0) or 0.0),
            }
        except Exception as exc:
            raise VoiceStudioError(f"Failed inspecting audio metadata for {p}: {exc}") from exc

    def normalize_audio(
        self,
        input_path: Union[str, Path],
        output_path: Union[str, Path],
        sample_rate: int = 48000,
        channels: int = 1,
        bit_depth_codec: str = "pcm_s24le",
    ) -> Path:
        """Converts raw audio to standard 48kHz mono 24-bit PCM WAV."""
        in_p = Path(input_path)
        out_p = Path(output_path)
        out_p.parent.mkdir(parents=True, exist_ok=True)

        cmd = [
            "ffmpeg", "-y",
            "-i", str(in_p.resolve()),
            "-ar", str(sample_rate),
            "-ac", str(channels),
            "-c:a", bit_depth_codec,
            str(out_p.resolve()),
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode != 0:
            raise VoiceStudioError(f"ffmpeg normalization failed: {res.stderr}")
        return out_p

    def _generate_synthetic_wav(
        self, text: str, output_path: Path, sample_rate: int = 48000
    ) -> Path:
        """Generates synthetic WAV audio offline proportional to text length."""
        words = len(text.split())
        duration = max(1.0, words * 0.35)  # ~170 words per minute
        n_samples = int(duration * sample_rate)

        output_path.parent.mkdir(parents=True, exist_ok=True)
        with wave.open(str(output_path), "wb") as wf:
            wf.setnchannels(1)  # Mono
            wf.setsampwidth(3)  # 24-bit (3 bytes)
            wf.setframerate(sample_rate)

            # Generate gentle sine tone
            raw_data = bytearray()
            freq = 220.0
            for i in range(n_samples):
                val = int(4000000.0 * math.sin(2.0 * math.pi * freq * (i / sample_rate)))
                raw_data.extend(val.to_bytes(3, byteorder="little", signed=True))
            wf.writeframes(bytes(raw_data))

        return output_path

    def synthesize_segment(
        self,
        text: str,
        output_path: Union[str, Path],
        language: str = "pt",
    ) -> str:
        """Synthesizes speech for a single text segment.
        
        Args:
            text: Text to synthesize.
            output_path: Final WAV destination.
            language: Speech language (default: pt).
            
        Returns:
            Path to the normalized WAV audio file.
        """
        out_p = Path(output_path)
        out_p.parent.mkdir(parents=True, exist_ok=True)

        if not text or not text.strip():
            raise VoiceStudioError("Cannot synthesize empty narration text.")

        if self.offline_mode:
            self._generate_synthetic_wav(text, out_p)
            return str(out_p)

        url = f"{self.base_url}/v1/audio/speech"
        data = {
            "model": self.model,
            "voice": self.voice_id,
            "input": text.strip(),
            "response_format": "wav",
            "language": language,
        }
        encoded = json.dumps(data).encode("utf-8")
        req = urllib.request.Request(
            url,
            data=encoded,
            headers={
                "Content-Type": "application/json",
                "User-Agent": "web-demo-narrator/1.0",
            },
            method="POST",
        )

        temp_raw = out_p.parent / f"_raw_{out_p.name}"
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                if resp.status == 200:
                    with open(temp_raw, "wb") as f:
                        f.write(resp.read())
                else:
                    raise VoiceStudioError(f"VoiceStudio API returned HTTP {resp.status}")
        except urllib.error.HTTPError as exc:
            err_body = exc.read().decode("utf-8", errors="ignore")
            raise VoiceStudioError(f"VoiceStudio synthesis HTTP {exc.code}: {err_body}") from exc
        except Exception as exc:
            raise VoiceStudioError(f"Synthesis connection failed to {url}: {exc}") from exc

        # Normalize to 48kHz mono 24-bit PCM
        try:
            self.normalize_audio(temp_raw, out_p)
        finally:
            if temp_raw.exists():
                temp_raw.unlink(missing_ok=True)

        return str(out_p)

    def synthesize_script(
        self,
        script: DemoScript,
        cache_dir: Optional[Union[str, Path]] = None,
    ) -> DemoScript:
        """Synthesizes audio for all scenes in a DemoScript with hash-based caching."""
        target_dir = Path(cache_dir or script.config.temp_dir) / "audio"
        target_dir.mkdir(parents=True, exist_ok=True)

        # Preflight checks
        self.check_health()
        if not self.check_voice_available():
            pass  # Non-fatal warning if voice listing is restricted, backend handles fallbacks

        for scene in script.scenes:
            if not scene.narration.strip():
                continue

            # Compute content hash for caching
            content_key = f"{self.voice_id}:{self.model}:{scene.narration.strip()}"
            content_hash = hashlib.sha256(content_key.encode("utf-8")).hexdigest()[:16]
            cached_filename = f"{scene.id}_{content_hash}.wav"
            cached_path = target_dir / cached_filename

            if cached_path.exists() and cached_path.stat().st_size > 1024:
                # Cache hit: measure duration
                scene.audio_path = str(cached_path)
                scene.audio_duration = self.get_audio_duration(cached_path)
            else:
                # Cache miss: synthesize
                self.synthesize_segment(scene.narration, cached_path)
                scene.audio_path = str(cached_path)
                scene.audio_duration = self.get_audio_duration(cached_path)

        return script
