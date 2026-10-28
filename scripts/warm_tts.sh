#!/usr/bin/env bash
set -euo pipefail

VS_URL="${VOICESTUDIO_URL:-http://localhost:3900}"
VOICE_ID="${VOICE_ID:-bf91cd6a}"
MAX_WAIT="${MAX_WAIT:-60}"

echo "[warm_tts] Aguardando VoiceStudio em $VS_URL (máx ${MAX_WAIT}s)..."
for i in $(seq 1 "$MAX_WAIT"); do
    if curl -sf "$VS_URL/health" | grep -q '"status":"ok"'; then
        echo "[warm_tts] Health OK."
        break
    fi
    if [ "$i" -eq "$MAX_WAIT" ]; then
        echo "[warm_tts] ERRO: VoiceStudio não respondeu em ${MAX_WAIT}s." >&2
        exit 1
    fi
    sleep 1
done

echo "[warm_tts] Aquecendo perfil $VOICE_ID..."
curl -sf -o /dev/null \
    -X POST "$VS_URL/v1/audio/speech" \
    -H "Content-Type: application/json" \
    -d "{\"model\":\"omnivoice\",\"voice\":\"$VOICE_ID\",\"input\":\"Aquecimento.\",\"response_format\":\"wav\"}" \
    --max-time 120
echo "[warm_tts] Pronto."
