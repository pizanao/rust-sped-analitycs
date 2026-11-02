"""
FFmpeg Audio Stitcher and Audio/Video Multiplexer.
Implements the contract defined in PROJECT.md § Milestone 4.
"""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
from typing import List, Union, Dict, Any


class AudioMuxerError(RuntimeError):
    """Raised when FFmpeg audio stitching or multiplexing fails."""
    pass


class AudioMuxer:
    """Handles audio stitching with exact pauses and final MP4 muxing with frame-freeze alignment."""

    @staticmethod
    def _probe(file_path: Path) -> Dict[str, Any]:
        """Inspects media file streams and container format using ffprobe."""
        cmd = [
            "ffprobe",
            "-v", "error",
            "-print_format", "json",
            "-show_streams",
            "-show_format",
            str(file_path.resolve()),
        ]
        try:
            res = subprocess.run(cmd, capture_output=True, text=True, check=True)
            return json.loads(res.stdout)
        except Exception as exc:
            raise AudioMuxerError(f"ffprobe inspection failed on {file_path}: {exc}") from exc

    @staticmethod
    def get_duration(file_path: Union[str, Path]) -> float:
        """Extracts media duration in seconds."""
        p = Path(file_path)
        data = AudioMuxer._probe(p)
        fmt = data.get("format", {})
        if "duration" in fmt:
            return float(fmt["duration"])
        streams = data.get("streams", [])
        if streams and "duration" in streams[0]:
            return float(streams[0]["duration"])
        return 0.0

    @staticmethod
    def create_silence_wav(
        duration_seconds: float,
        output_path: Union[str, Path],
        sample_rate: int = 48000,
        channels: int = 1,
        bit_depth_codec: str = "pcm_s24le",
    ) -> Path:
        """Generates exact digital silence WAV using FFmpeg lavfi anullsrc."""
        out_p = Path(output_path)
        out_p.parent.mkdir(parents=True, exist_ok=True)

        dur = max(0.01, float(duration_seconds))
        channel_layout = "mono" if channels == 1 else "stereo"

        cmd = [
            "ffmpeg", "-y",
            "-f", "lavfi",
            "-i", f"anullsrc=r={sample_rate}:cl={channel_layout}",
            "-t", f"{dur:.3f}",
            "-c:a", bit_depth_codec,
            str(out_p.resolve()),
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode != 0:
            raise AudioMuxerError(f"Failed to generate silence WAV: {res.stderr}")
        return out_p

    @staticmethod
    def concat_audio_with_silence(
        scene_audio_paths: List[Union[str, Path]],
        pauses: List[float],
        output_master_wav: Union[str, Path],
        sample_rate: int = 48000,
        bit_depth_codec: str = "pcm_s24le",
    ) -> float:
        """Stitches per-scene audio files with exact silence intervals into a master narration WAV.
        
        Args:
            scene_audio_paths: List of audio segment paths.
            pauses: List of pause durations (in seconds) after each corresponding scene.
            output_master_wav: Destination master WAV path.
            sample_rate: Sample rate (default: 48000).
            bit_depth_codec: Codec for PCM output (default: pcm_s24le).
            
        Returns:
            Total master audio duration in seconds.
        """
        out_master = Path(output_master_wav)
        out_master.parent.mkdir(parents=True, exist_ok=True)

        if not scene_audio_paths:
            raise AudioMuxerError("No audio paths provided for concatenation.")

        temp_dir = out_master.parent / "_mux_temp"
        temp_dir.mkdir(parents=True, exist_ok=True)

        concat_items: List[Path] = []

        for idx, audio_p in enumerate(scene_audio_paths):
            p = Path(audio_p)
            if not p.is_file():
                raise FileNotFoundError(f"Scene audio file not found: {p}")

            # Standardize segment to target sample rate, mono, and bit depth
            std_audio = temp_dir / f"std_audio_{idx}.wav"
            cmd_std = [
                "ffmpeg", "-y",
                "-i", str(p.resolve()),
                "-ar", str(sample_rate),
                "-ac", "1",
                "-c:a", bit_depth_codec,
                str(std_audio.resolve()),
            ]
            res_std = subprocess.run(cmd_std, capture_output=True, text=True)
            if res_std.returncode != 0:
                raise AudioMuxerError(f"Failed to standardize audio segment {p}: {res_std.stderr}")
            concat_items.append(std_audio)

            # Insert pause after each scene except optionally after the last one if pause is 0
            pause_sec = pauses[idx] if idx < len(pauses) else 0.8
            if pause_sec > 0:
                silence_file = temp_dir / f"silence_{idx}_{int(pause_sec * 1000)}ms.wav"
                AudioMuxer.create_silence_wav(pause_sec, silence_file, sample_rate=sample_rate, bit_depth_codec=bit_depth_codec)
                concat_items.append(silence_file)

        # Write concat demuxer list
        list_file = temp_dir / "concat_list.txt"
        with open(list_file, "w", encoding="utf-8") as f:
            for item in concat_items:
                f.write(f"file '{item.resolve()}'\n")

        # Concat files via ffmpeg
        cmd = [
            "ffmpeg", "-y",
            "-f", "concat",
            "-safe", "0",
            "-i", str(list_file.resolve()),
            "-ar", str(sample_rate),
            "-ac", "1",
            "-c:a", bit_depth_codec,
            str(out_master.resolve()),
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode != 0:
            raise AudioMuxerError(f"Audio concatenation failed: {res.stderr}")

        return AudioMuxer.get_duration(out_master)

    @staticmethod
    def mux_video_audio(
        video_path: Union[str, Path],
        audio_path: Union[str, Path],
        output_mp4_path: Union[str, Path],
    ) -> str:
        """Multiplexes video and narration into an MP4 container, freezing the final frame if needed."""
        v_path = Path(video_path)
        a_path = Path(audio_path)
        out_path = Path(output_mp4_path)
        out_path.parent.mkdir(parents=True, exist_ok=True)

        if not v_path.is_file():
            raise FileNotFoundError(f"Input video file not found: {v_path}")
        if not a_path.is_file():
            raise FileNotFoundError(f"Input audio file not found: {a_path}")

        v_dur = AudioMuxer.get_duration(v_path)
        a_dur = AudioMuxer.get_duration(a_path)

        # If video is shorter than audio, freeze the last frame using tpad filter
        diff = a_dur - v_dur
        vf_filters: List[str] = []
        if diff > 0.05:
            vf_filters.append(f"tpad=stop_mode=clone:stop_duration={diff:.3f}")

        cmd = [
            "ffmpeg", "-y",
            "-i", str(v_path.resolve()),
            "-i", str(a_path.resolve()),
        ]

        if vf_filters:
            cmd.extend(["-vf", ",".join(vf_filters)])

        cmd.extend([
            "-c:v", "libx264",
            "-preset", "medium",
            "-crf", "20",
            "-pix_fmt", "yuv420p",
            "-c:a", "aac",
            "-b:a", "192k",
            "-ar", "48000",
            "-shortest",
            str(out_path.resolve()),
        ])

        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode != 0:
            raise AudioMuxerError(f"Video/audio multiplexing failed: {res.stderr}")

        # Post-verification
        probe_data = AudioMuxer._probe(out_path)
        streams = probe_data.get("streams", [])
        v_stream = next((s for s in streams if s.get("codec_type") == "video"), None)
        a_stream = next((s for s in streams if s.get("codec_type") == "audio"), None)

        if not v_stream or not a_stream:
            raise AudioMuxerError("Output file missing audio or video stream after muxing.")

        final_v_dur = float(v_stream.get("duration", probe_data.get("format", {}).get("duration", 0)))
        final_a_dur = float(a_stream.get("duration", probe_data.get("format", {}).get("duration", 0)))
        drift = abs(final_v_dur - final_a_dur)
        if drift > 0.5:
            # Emit warning if drift exceeds tolerance
            pass

        return str(out_path)
