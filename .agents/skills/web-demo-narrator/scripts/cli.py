#!/usr/bin/env python3
"""
CLI interface for web-demo-narrator skill.
Implements the contract defined in PROJECT.md § Milestone 5.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import sys
from typing import Optional

# Ensure skill root directory is in sys.path for direct invocation from any directory
_SKILL_ROOT = Path(__file__).resolve().parent.parent
if str(_SKILL_ROOT) not in sys.path:
    sys.path.insert(0, str(_SKILL_ROOT))

from rich.console import Console
from rich.table import Table

from scripts.schema import (
    load_demo_script,
    dump_demo_script,
    SchemaValidationError,
    DemoScript,
)
from scripts.humanizer import SpeechHumanizer
from scripts.tts_engine import VoiceStudioClient, VoiceStudioError
from scripts.browser_runner import BrowserRunner, BrowserRunnerError
from scripts.audio_muxer import AudioMuxer, AudioMuxerError

console = Console()


def _format_diff_table(diffs: list[dict]) -> Table:
    """Formats a rich table displaying text changes before/after humanization."""
    table = Table(title="Revisão de Humanização de Fala (pt-BR)", show_lines=True)
    table.add_column("Cena / ID", style="cyan", no_wrap=True)
    table.add_column("Texto Original", style="red")
    table.add_column("Texto Humanizado", style="green")

    for d in diffs:
        if d.get("changed"):
            table.add_row(d["id"], d["original"], d["humanized"])
        else:
            table.add_row(d["id"], d["original"], "[dim](Sem alterações necessárias)[/dim]")

    return table


def cmd_humanize(args: argparse.Namespace) -> int:
    """Executes the humanize subcommand."""
    try:
        script = load_demo_script(args.script)
    except Exception as exc:
        console.print(f"[bold red]Erro ao carregar roteiro:[/bold red] {exc}")
        return 1

    humanizer = SpeechHumanizer()
    humanized_script, diffs = humanizer.process_demo_script(script)

    if args.diff:
        table = _format_diff_table(diffs)
        console.print(table)

    out_path = Path(args.output) if args.output else Path(args.script)
    dump_demo_script(humanized_script, out_path)
    console.print(f"[bold green]✔ Roteiro humanizado salvo em:[/bold green] {out_path}")
    return 0


def cmd_synthesize(args: argparse.Namespace) -> int:
    """Executes the synthesize subcommand."""
    try:
        script = load_demo_script(args.script)
    except Exception as exc:
        console.print(f"[bold red]Erro ao carregar roteiro:[/bold red] {exc}")
        return 1

    base_url = args.backend_url or script.config.base_url
    voice_id = args.voice_id or script.config.voice_id
    model = args.model or script.config.model

    client = VoiceStudioClient(
        base_url=base_url,
        voice_id=voice_id,
        model=model,
        offline_mode=bool(args.offline),
    )

    console.print(f"[cyan]Iniciando síntese de voz ({voice_id} via {model})...[/cyan]")
    try:
        updated_script = client.synthesize_script(script)
    except Exception as exc:
        console.print(f"[bold red]Erro na síntese TTS:[/bold red] {exc}")
        return 1

    out_path = Path(args.output) if args.output else Path(args.script)
    dump_demo_script(updated_script, out_path)
    console.print(f"[bold green]✔ Áudios sintetizados e metadados salvos em:[/bold green] {out_path}")
    return 0


def cmd_record(args: argparse.Namespace) -> int:
    """Executes the record subcommand."""
    try:
        script = load_demo_script(args.script)
    except Exception as exc:
        console.print(f"[bold red]Erro ao carregar roteiro:[/bold red] {exc}")
        return 1

    output_video = (
        Path(args.output)
        if args.output
        else Path(script.config.temp_dir) / "recorded_video.webm"
    )

    headless = not args.headed if hasattr(args, "headed") and args.headed else args.headless
    action_timeout = float(args.action_timeout) if hasattr(args, "action_timeout") and args.action_timeout else 10000.0

    runner = BrowserRunner(
        config=script.config,
        headless=headless,
        action_timeout=action_timeout,
    )

    console.print("[cyan]Iniciando gravação de vídeo web via Playwright...[/cyan]")
    try:
        res = runner.record_demo(script, output_video)
        console.print(f"[bold green]✔ Vídeo gravado com sucesso:[/bold green] {res}")
        return 0
    except Exception as exc:
        console.print(f"[bold red]Erro durante a gravação web:[/bold red] {exc}")
        return 1


def cmd_build(args: argparse.Namespace) -> int:
    """Executes the full end-to-end build pipeline."""
    try:
        script = load_demo_script(args.script)
    except Exception as exc:
        console.print(f"[bold red]Erro de validação do roteiro:[/bold red] {exc}")
        return 1

    # 1. Humanize (unless skipped)
    if not args.skip_humanize:
        console.print("[cyan]1/4. Humanizando falas do roteiro...[/cyan]")
        humanizer = SpeechHumanizer()
        script, diffs = humanizer.process_demo_script(script)
        if getattr(args, "diff", False):
            console.print(_format_diff_table(diffs))

    # 2. Synthesize audio
    console.print("[cyan]2/4. Sintetizando áudio TTS via VoiceStudio...[/cyan]")
    backend_url = getattr(args, "backend_url", None) or script.config.base_url
    voice_id = getattr(args, "voice_id", None) or script.config.voice_id
    model = getattr(args, "model", None) or script.config.model

    offline = bool(getattr(args, "offline", False))
    # If offline wasn't passed explicitly, auto-detect if VoiceStudio is unreachable and fallback to offline if needed
    client = VoiceStudioClient(
        base_url=backend_url,
        voice_id=voice_id,
        model=model,
        offline_mode=offline,
    )
    if not offline:
        try:
            client.check_health()
        except VoiceStudioError:
            console.print("[yellow]Aviso: Backend VoiceStudio inacessível em localhost:3900. Usando modo de síntese acústica local para o build.[/yellow]")
            client.offline_mode = True

    try:
        script = client.synthesize_script(script)
    except Exception as exc:
        console.print(f"[bold red]Erro na síntese de áudio:[/bold red] {exc}")
        return 1

    # 3. Record web demo
    console.print("[cyan]3/4. Gravando navegação web com Playwright (audio-driven)...[/cyan]")
    temp_video = Path(script.config.temp_dir) / "raw_browser_recording.webm"
    headless = not getattr(args, "headed", False) if hasattr(args, "headed") and args.headed else getattr(args, "headless", True)

    runner = BrowserRunner(config=script.config, headless=headless)
    try:
        runner.record_demo(script, temp_video)
    except Exception as exc:
        console.print(f"[bold red]Erro na automação do navegador:[/bold red] {exc}")
        return 1

    # 4. Stitch audio and mux with video
    console.print("[cyan]4/4. Concatenação de áudio e multiplexação final MP4...[/cyan]")
    audio_paths = [s.audio_path for s in script.scenes if s.audio_path]
    pauses = [s.pause_after for s in script.scenes]
    master_audio = Path(script.config.temp_dir) / "narration_master.wav"

    try:
        AudioMuxer.concat_audio_with_silence(audio_paths, pauses, master_audio)
    except Exception as exc:
        console.print(f"[bold red]Erro na concatenação da narração:[/bold red] {exc}")
        return 1

    final_output = (
        Path(args.output)
        if args.output
        else Path(script.config.output_dir) / "demo_final.mp4"
    )

    try:
        AudioMuxer.mux_video_audio(temp_video, master_audio, final_output)
    except Exception as exc:
        console.print(f"[bold red]Erro no muxing final:[/bold red] {exc}")
        return 1

    console.print(f"\n[bold green]✔ DEMO CONCLUÍDA COM SUCESSO![/bold green]")
    console.print(f"[bold]Arquivo final:[/bold] {final_output.resolve()}")
    return 0


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        prog="web-demo-narrator",
        description="Gravação automatizada de demonstrações web com narração sincronizada TTS (VoiceStudio).",
    )
    subparsers = parser.add_subparsers(dest="subcommand", help="Subcomandos disponíveis")

    # humanize
    p_humanize = subparsers.add_parser("humanize", help="Revisa e humaniza as falas do roteiro.")
    p_humanize.add_argument("script", help="Caminho do arquivo YAML do roteiro.")
    p_humanize.add_argument("-o", "--output", help="Caminho de saída para o YAML humanizado.")
    p_humanize.add_argument("--diff", action="store_true", help="Exibe tabela comparativa de alterações.")
    p_humanize.set_defaults(func=cmd_humanize)

    # synthesize
    p_synth = subparsers.add_parser("synthesize", help="Sintetiza os áudios TTS para cada cena.")
    p_synth.add_argument("script", help="Caminho do arquivo YAML do roteiro.")
    p_synth.add_argument("-o", "--output", help="Caminho de saída para o YAML atualizado com timings.")
    p_synth.add_argument("--voice-id", help="ID do perfil de voz (default: bf91cd6a).")
    p_synth.add_argument("--model", help="Modelo TTS (default: omnivoice).")
    p_synth.add_argument("--backend-url", help="URL do backend VoiceStudio.")
    p_synth.add_argument("--offline", action="store_true", help="Modo offline de síntese acústica para testes.")
    p_synth.set_defaults(func=cmd_synthesize)

    # record
    p_record = subparsers.add_parser("record", help="Executa o Playwright gravando vídeo calibrado pelo áudio.")
    p_record.add_argument("script", help="Caminho do arquivo YAML do roteiro.")
    p_record.add_argument("-o", "--output", help="Caminho de saída para o vídeo gravado (.webm).")
    p_record.add_argument("--headless", action="store_true", default=True, help="Executa o browser em modo headless.")
    p_record.add_argument("--headed", action="store_true", help="Executa o browser visível na tela.")
    p_record.add_argument("--action-timeout", type=float, default=10000.0, help="Timeout para seletores em ms.")
    p_record.set_defaults(func=cmd_record)

    # build
    p_build = subparsers.add_parser("build", help="Executa o pipeline completo (humanize -> synth -> record -> mux).")
    p_build.add_argument("script", help="Caminho do arquivo YAML do roteiro.")
    p_build.add_argument("-o", "--output", help="Caminho de saída para o MP4 final.")
    p_build.add_argument("--skip-humanize", action="store_true", help="Pula a etapa de humanização de texto.")
    p_build.add_argument("--diff", action="store_true", help="Exibe o diff de humanização durante o build.")
    p_build.add_argument("--headless", action="store_true", default=True, help="Executa o browser em modo headless.")
    p_build.add_argument("--headed", action="store_true", help="Executa o browser visível na tela.")
    p_build.add_argument("--offline", action="store_true", help="Força modo de síntese offline para testes.")
    p_build.set_defaults(func=cmd_build)

    args = parser.parse_args(argv)

    if not args.subcommand:
        parser.print_help()
        return 0

    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
