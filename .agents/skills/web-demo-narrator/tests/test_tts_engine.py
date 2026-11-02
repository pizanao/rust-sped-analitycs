"""
Unit tests for VoiceStudio TTS Engine client.
Tests healthcheck, voice discovery, synthesis caching, audio normalization, and ffprobe durations.
"""

from pathlib import Path
import pytest
import wave

from scripts.schema import DemoScript, DemoConfig, Scene, Action
from scripts.tts_engine import VoiceStudioClient, VoiceStudioError


def test_offline_mode_healthcheck():
    """Verifies that offline_mode reports healthy status without network calls."""
    client = VoiceStudioClient(offline_mode=True)
    health = client.check_health()
    assert health["status"] == "ok"
    assert health["device"] == "offline-mock"
    assert client.check_voice_available("bf91cd6a") is True


def test_get_audio_duration_with_synthetic_wav(synthetic_wav_factory, tmp_path: Path):
    """Verifies exact duration measurement with ffprobe on synthetic audio."""
    wav_path = synthetic_wav_factory(
        filename="one_point_five.wav",
        duration_seconds=1.5,
        sample_rate=48000,
        output_dir=tmp_path,
    )
    client = VoiceStudioClient(offline_mode=True)
    duration = client.get_audio_duration(wav_path)
    assert abs(duration - 1.5) < 0.05, f"Expected ~1.5s duration, got {duration}s"


def test_get_audio_duration_nonexistent_file_raises_error():
    """Verifies that inspecting a non-existent file raises FileNotFoundError."""
    client = VoiceStudioClient(offline_mode=True)
    with pytest.raises(FileNotFoundError):
        client.get_audio_duration("non_existent_audio_path_9999.wav")


def test_normalize_audio_standardizes_format(synthetic_wav_factory, tmp_path: Path):
    """Verifies that normalize_audio converts audio to 48kHz 24-bit mono."""
    # Create 16-bit 22050Hz input
    src_wav = synthetic_wav_factory(
        filename="input_22k.wav",
        duration_seconds=1.0,
        sample_rate=22050,
        output_dir=tmp_path,
    )
    norm_wav = tmp_path / "normalized_48k.wav"

    client = VoiceStudioClient(offline_mode=True)
    client.normalize_audio(src_wav, norm_wav)

    assert norm_wav.exists()
    assert norm_wav.stat().st_size > 0

    info = client.get_audio_info(norm_wav)
    assert info["codec"] == "pcm_s24le"
    assert info["sample_rate"] == 48000
    assert info["channels"] == 1
    assert info["bits_per_raw_sample"] == 24


def test_synthesize_segment_offline_mode(tmp_path: Path):
    """Verifies that synthesize_segment in offline mode creates a valid WAV."""
    out_file = tmp_path / "offline_speech.wav"
    client = VoiceStudioClient(offline_mode=True)
    res_path = client.synthesize_segment("Olá mundo, este é um teste de áudio.", out_file)

    assert Path(res_path).exists()
    duration = client.get_audio_duration(res_path)
    assert duration > 1.0


def test_synthesize_empty_narration_raises_error(tmp_path: Path):
    """Verifies that empty narration strings are rejected."""
    client = VoiceStudioClient(offline_mode=True)
    with pytest.raises(VoiceStudioError, match="empty narration"):
        client.synthesize_segment("", tmp_path / "empty.wav")


def test_synthesize_script_caching(tmp_path: Path):
    """Verifies that synthesize_script sets audio paths, measures durations, and leverages cache."""
    script = DemoScript(
        title="Cache Test Demo",
        description="Testing audio caching",
        config=DemoConfig(temp_dir=str(tmp_path / "temp")),
        scenes=[
            Scene(
                id="s1",
                title="Scene 1",
                narration="Primeira cena narrada para teste de síntese.",
                actions=[Action(action="wait", seconds=1.0)],
            ),
            Scene(
                id="s2",
                title="Scene 2",
                narration="Segunda cena com outro texto falado.",
                actions=[Action(action="wait", seconds=1.0)],
            ),
        ],
    )

    client = VoiceStudioClient(offline_mode=True)
    updated_script = client.synthesize_script(script)

    assert updated_script.scenes[0].audio_path is not None
    assert updated_script.scenes[0].audio_duration is not None
    assert updated_script.scenes[0].audio_duration > 0
    assert Path(updated_script.scenes[0].audio_path).exists()

    audio_path_first = updated_script.scenes[0].audio_path
    mtime_first = Path(audio_path_first).stat().st_mtime_ns

    # Re-run synthesis (should hit cache)
    client.synthesize_script(updated_script)
    mtime_second = Path(audio_path_first).stat().st_mtime_ns
    assert mtime_first == mtime_second, "Cache hit should not re-generate the audio file"
