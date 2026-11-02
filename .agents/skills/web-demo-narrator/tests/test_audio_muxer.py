"""
Unit tests for FFmpeg Audio Stitcher and Multiplexer.
"""

from pathlib import Path
import subprocess
import pytest

from scripts.audio_muxer import AudioMuxer, AudioMuxerError


def test_create_silence_wav(tmp_path: Path):
    """Verifies generation of exact silence WAV using FFmpeg lavfi."""
    silence_file = tmp_path / "silence_1s.wav"
    AudioMuxer.create_silence_wav(1.0, silence_file)

    assert silence_file.exists()
    dur = AudioMuxer.get_duration(silence_file)
    assert abs(dur - 1.0) < 0.05


def test_concat_audio_with_silence(synthetic_wav_factory, tmp_path: Path):
    """Verifies stitching multiple WAVs with intercalated silence intervals."""
    w1 = synthetic_wav_factory("part1.wav", duration_seconds=1.0, output_dir=tmp_path)
    w2 = synthetic_wav_factory("part2.wav", duration_seconds=2.0, output_dir=tmp_path)

    master_wav = tmp_path / "master.wav"
    total_dur = AudioMuxer.concat_audio_with_silence(
        scene_audio_paths=[w1, w2],
        pauses=[0.5, 0.5],
        output_master_wav=master_wav,
    )

    assert master_wav.exists()
    # Expected: 1.0 (w1) + 0.5 (pause1) + 2.0 (w2) + 0.5 (pause2) = 4.0s
    assert abs(total_dur - 4.0) < 0.1
    file_dur = AudioMuxer.get_duration(master_wav)
    assert abs(file_dur - 4.0) < 0.1


def test_mux_video_audio(synthetic_wav_factory, tmp_path: Path):
    """Verifies multiplexing video and audio into a valid MP4 with H.264 and AAC."""
    # Create test video (1s solid color 1920x1080)
    raw_video = tmp_path / "test_video.mp4"
    cmd = [
        "ffmpeg", "-y",
        "-f", "lavfi",
        "-i", "color=c=blue:s=1920x1080:d=1.5:r=25",
        "-c:v", "libx264",
        "-pix_fmt", "yuv420p",
        str(raw_video),
    ]
    subprocess.run(cmd, check=True, capture_output=True)

    # Create test audio (2.0s sine wave) - slightly longer than video to test tpad frame clone
    test_audio = synthetic_wav_factory("test_narration.wav", duration_seconds=2.0, output_dir=tmp_path)

    output_mp4 = tmp_path / "final_muxed.mp4"
    res = AudioMuxer.mux_video_audio(raw_video, test_audio, output_mp4)

    assert Path(res).exists()
    assert Path(res).stat().st_size > 1024

    probe = AudioMuxer._probe(output_mp4)
    streams = probe.get("streams", [])
    v_stream = next(s for s in streams if s.get("codec_type") == "video")
    a_stream = next(s for s in streams if s.get("codec_type") == "audio")

    assert v_stream.get("codec_name") == "h264"
    assert v_stream.get("width") == 1920
    assert v_stream.get("height") == 1080
    assert a_stream.get("codec_name") == "aac"
    assert int(a_stream.get("sample_rate", 0)) == 48000

    v_dur = float(v_stream.get("duration", probe.get("format", {}).get("duration", 0)))
    a_dur = float(a_stream.get("duration", probe.get("format", {}).get("duration", 0)))
    assert abs(v_dur - a_dur) < 0.5
