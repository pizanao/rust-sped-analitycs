# Demo SPA Narrator — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create four artefacts that make the SPA functionally testable and recordable as a narrated video: `demo_spa.yaml` (22-scene Playwright script), `fixtures/sped_minimo.txt` (SPED ECD upload fixture), `scripts/validate_spa_selectors.py` (pre-flight selector smoke-test), and `scripts/warm_tts.sh` (VoiceStudio warm-up guard).

**Architecture:** Each artefact is standalone. `warm_tts.sh` and `validate_spa_selectors.py` are pre-conditions that must pass before `demo_spa.yaml` is synthesized and recorded. `demo_spa.yaml` drives the existing `web-demo-narrator` CLI (`~/.gemini/config/skills/web-demo-narrator/scripts/cli.py`) without modifying it. The fixture is generated once from the existing `scripts/gerar_massa_sped.py::gerar_ecd_minimo_pva()`.

**Tech Stack:** YAML (demo script), Bash (warm_tts.sh), Python 3.10+ + Playwright sync API (validator), Python 3 + codecs/struct (fixture via existing gerar_massa_sped.py).

**Spec:** `docs/superpowers/specs/2026-10-27-demo-spa-narrator-design.md`

## Global Constraints

- `demo_spa.yaml` `config.base_url` must be `http://localhost:3900` (TTS — not the frontend URL).
- All `navigate` actions must use absolute URLs `http://localhost:3000/<route>` — never relative paths.
- Fixture encoding: Windows-1252, line endings CRLF (`\r\n`), size < 100 KB.
- Fixture CNPJ: `33000167000101`, period `01012023`–`31122023`.
- Seletores: use exactly the verified strings from spec §5 — do NOT paraphrase or abbreviate.
- `validate_spa_selectors.py` must exit 1 if any selector times out; exit 0 only when all pass.
- `warm_tts.sh` must exit 1 if VoiceStudio is unreachable after `MAX_WAIT=60` seconds.
- No new Python dependencies beyond `playwright` and `pyyaml` (already in web-demo-narrator's requirements.txt).

## Review Focus

- **Copilot sidebar when collapsed:** `button:has(svg.lucide-bot)` — the text "Copiloto de IA" is hidden when `collapsed=true`; using `has-text` here would silently miss the element. The validator must confirm this selector resolves on `/rfb-cnpj` before the scene runs.
- **`select` action requires `value` field:** `schema.py` validates that `select` actions carry `value`, not `text`. The demo YAML uses `select` for the UF filter (cena 19); the `value` must be `"SP"`, not the display label.
- **`input[type="file"]` is not visible by default:** `FileDropzone` hides the input with CSS. `validate_spa_selectors.py` must use `page.locator(...).first` without a `wait_for(state="visible")` — or the fixture upload step in the YAML must set `highlight: false` to skip the highlight-triggered scroll that requires visibility.
- **BFS slider interaction:** `input[type="range"]` requires `fill("5")`, not `type("5")` — the runner calls `loc.fill("")` then `loc.type(text, delay=40)`, which does not work reliably on range inputs. The YAML should use `fill` action (not `type`) for the slider, mapping to `loc.fill(value)`.
- **Two `button:has-text("Clear")` elements on `/rfb-cnpj`:** one in FTSSearchPanel and one in MakefileController. The validator must check that `.first` resolves the FTS5 one when FTS5 is visible, and the Makefile one when scrolled to MakefileController. Separate scenes and `wait` between them guarantee correct context.

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `demo_spa.yaml` | Create | 22-scene roteiro; drives web-demo-narrator CLI |
| `fixtures/sped_minimo.txt` | Create (generated) | SPED ECD file for upload scene |
| `scripts/validate_spa_selectors.py` | Create | Pre-flight: resolve every selector in DOM |
| `scripts/warm_tts.sh` | Create | Pre-flight: guard against VoiceStudio cold server |

---

### Task 1: `scripts/warm_tts.sh`

**Files:**
- Create: `scripts/warm_tts.sh`

**Interfaces:**
- Produces: executable shell script; exits 0 when VoiceStudio is warm, 1 on timeout or health failure.
- Consumes: nothing from other tasks.

- [ ] **Step 1: Create the script**

```bash
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
```

- [ ] **Step 2: Make executable and smoke-test**

```bash
chmod +x scripts/warm_tts.sh
bash scripts/warm_tts.sh
```

Expected: `[warm_tts] Pronto.` and exit 0 (VoiceStudio must be running).

- [ ] **Step 3: Commit**

```bash
git add scripts/warm_tts.sh
git commit -m "feat: warm_tts.sh — pre-flight guard for VoiceStudio model warm-up"
```

---

### Task 2: `fixtures/sped_minimo.txt`

**Files:**
- Create: `fixtures/sped_minimo.txt` (generated, not hand-written)
- Uses: `scripts/gerar_massa_sped.py::gerar_ecd_minimo_pva()`

**Interfaces:**
- Produces: `fixtures/sped_minimo.txt` — Windows-1252, CRLF, CNPJ `33000167000101`, period 2023, < 100 KB.
- Consumes: `gerar_ecd_minimo_pva(caminho_saida, cnpj)` → `(str, str)` from `scripts/gerar_massa_sped.py`.

- [ ] **Step 1: Generate the fixture**

```bash
python3 - <<'EOF'
import sys
sys.path.insert(0, "scripts")
from gerar_massa_sped import gerar_ecd_minimo_pva
import os
os.makedirs("fixtures", exist_ok=True)
path, sha1 = gerar_ecd_minimo_pva("fixtures/sped_minimo.txt", cnpj="33000167000101")
print(f"Gerado: {path}")
print(f"SHA-1:  {sha1}")
import os
size = os.path.getsize(path)
print(f"Tamanho: {size} bytes ({size/1024:.1f} KB)")
assert size < 100 * 1024, f"Fixture excede 100 KB: {size}"
EOF
```

Expected: file created, size < 100 KB.

- [ ] **Step 2: Verify encoding and CNPJ**

```bash
python3 -c "
data = open('fixtures/sped_minimo.txt', 'rb').read()
assert b'\r\n' in data, 'Sem CRLF'
text = data.decode('windows-1252')
assert '33000167000101' in text, 'CNPJ não encontrado'
print('OK: CRLF e CNPJ presentes')
print('Primeiros 200 chars:', text[:200])
"
```

Expected: `OK: CRLF e CNPJ presentes` and the `|0000|` header line visible.

- [ ] **Step 3: Commit**

```bash
git add fixtures/sped_minimo.txt
git commit -m "feat: fixtures/sped_minimo.txt — SPED ECD sintético Windows-1252 para cena de upload"
```

---

### Task 3: `scripts/validate_spa_selectors.py`

**Files:**
- Create: `scripts/validate_spa_selectors.py`

**Interfaces:**
- Consumes: `demo_spa.yaml` path as CLI arg; reads it with `yaml.safe_load`.
- Produces: stdout lines `✅ scene_id: selector` / `❌ scene_id: selector — TIMEOUT`; exit 0 / 1.

**Note on hidden file inputs:** `input[type="file"]` is CSS-hidden in `FileDropzone`. Check it with `page.locator(...).count() > 0` (DOM presence), not `wait_for(state="visible")`.

- [ ] **Step 1: Write the script**

```python
#!/usr/bin/env python3
"""Pre-flight selector validator for demo_spa.yaml.

Usage: python3 scripts/validate_spa_selectors.py demo_spa.yaml [--url http://localhost:3000]
Exit 0 when all selectors resolve; exit 1 on any timeout.
"""
from __future__ import annotations
import argparse, sys
from pathlib import Path
import yaml
from playwright.sync_api import sync_playwright, TimeoutError as PWTimeout

TIMEOUT_MS = 8_000
ROUTE_MAP = {
    "/rfb-cnpj":  ["rfb-cnpj", "abertura", "timeline", "graficos", "grafo",
                   "zoom", "fts5", "makefile", "fim"],
    "/ingestao":  ["ingestao", "upload", "logs"],
    "/fiscal":    ["fiscal", "xlsx", "sped"],
    "/grc":       ["grc", "slider", "caminho", "drawer"],
    "/indicadores": ["indicadores", "copilot", "sugestoes"],
    "/rfb-cnpj":  ["encerramento"],
}

# Selectors indexed by keywords appearing in scene IDs.
# Each entry: (scene_id_fragment, css_selector, check_mode)
# check_mode: "visible" | "present" (for hidden inputs)
SCENE_SELECTORS: list[tuple[str, str, str]] = [
    ("abertura",    "text=7.53x",                                  "visible"),
    ("timeline",    "text=Pipeline Stages",                        "visible"),
    ("graficos",    "text=Processing Speed",                       "visible"),
    ("grafo_soc",   'button:has-text("Todos")',                     "visible"),
    ("zoom",        'button[title="Aproximar Zoom"]',               "visible"),
    ("fts5",        'input[placeholder^="Search by"]',              "visible"),
    ("filtro_fts",  'button:has-text("Clear")',                     "visible"),
    ("makefile",    'button:has-text("Run Selected Target")',        "visible"),
    ("ingestao",    'input[placeholder^="Buscar por arquivo"]',     "visible"),
    ("upload",      'input[type="file"]',                          "present"),
    ("sse_logs",    'button:has-text("Logs")',                      "visible"),
    ("fiscal_dre",  'input[placeholder^="Digite o CNPJ"]',          "visible"),
    ("xlsx",        'button:has-text("Exportar")',                  "visible"),
    ("sped_blocks", '[role="tab"]:has-text("J150")',                "visible"),
    ("grc_nav",     'input[placeholder^="CNPJ ou CPF"]',            "visible"),
    ("slider",      'input[type="range"]',                          "visible"),
    ("caminho",     'button:has-text("Menor Caminho")',              "visible"),
    ("indicadores", 'button:has-text("Atualizar")',                 "visible"),
    ("copilot",     'button:has-text("Copiloto de IA")',             "visible"),
]

def check_scene(page, scene_id: str, selector: str, mode: str) -> bool:
    try:
        loc = page.locator(selector).first
        if mode == "present":
            return loc.count() > 0
        loc.wait_for(state="visible", timeout=TIMEOUT_MS)
        return True
    except PWTimeout:
        return False

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("yaml_path")
    ap.add_argument("--url", default="http://localhost:3000")
    args = ap.parse_args()

    data = yaml.safe_load(Path(args.yaml_path).read_text())
    scenes = {s["id"]: s for s in data.get("scenes", [])}

    failed = 0
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page(viewport={"width": 1280, "height": 800})

        for route, suffix_list in [
            ("/rfb-cnpj",    ["abertura","timeline","graficos","grafo_soc","zoom","fts5","filtro_fts","makefile"]),
            ("/ingestao",    ["ingestao","upload","sse_logs"]),
            ("/fiscal",      ["fiscal_dre","xlsx","sped_blocks"]),
            ("/grc",         ["grc_nav","slider","caminho"]),
            ("/indicadores", ["indicadores","copilot"]),
        ]:
            page.goto(f"{args.url}{route}", wait_until="networkidle", timeout=20_000)
            page.wait_for_timeout(1_000)
            for frag, sel, mode in SCENE_SELECTORS:
                if not any(frag in s for s in suffix_list):
                    continue
                ok = check_scene(page, frag, sel, mode)
                mark = "✅" if ok else "❌"
                print(f"{mark} {frag}: {sel}")
                if not ok:
                    failed += 1

        browser.close()

    if failed:
        print(f"\n{failed} seletor(es) falharam.", file=sys.stderr)
        return 1
    print("\nTodos os seletores OK.")
    return 0

if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 2: Run the validator (frontend must be running on :3000)**

```bash
python3 scripts/validate_spa_selectors.py demo_spa.yaml
```

Expected: all `✅`, exit 0. Fix any `❌` by correcting the selector in `SCENE_SELECTORS` before proceeding.

- [ ] **Step 3: Commit**

```bash
git add scripts/validate_spa_selectors.py
git commit -m "feat: validate_spa_selectors.py — pre-flight Playwright selector smoke-test"
```

---

### Task 4: `demo_spa.yaml`

**Files:**
- Create: `demo_spa.yaml` (project root)

**Interfaces:**
- Consumes: `web-demo-narrator` CLI at `~/.gemini/config/skills/web-demo-narrator/scripts/cli.py`.
- Consumes: `fixtures/sped_minimo.txt` absolute path for the upload scene.
- Produces: YAML parsed successfully by `load_demo_script()` from `schema.py` — 22 scenes, 0 schema errors.

**Critical constraints (from Review Focus):**
- Copilot scene must use `button:has(svg.lucide-bot)` — not `has-text`.
- UF filter (cena 19) must use `action: select` with `value: "SP"` — not `type`.
- Slider (cena 16) must use `action: fill` with `text: "5"` — not `type`.
- `input[type="file"]` scene must have `highlight: false` to skip visibility check.

- [ ] **Step 1: Write `demo_spa.yaml`**

```yaml
title: "Rust Sped Analytics — Demonstração Funcional da SPA"
description: "22 cenas cobrindo as 5 abas da aplicação: Pipeline RFB CNPJ, Ingestão SPED, Auditoria Fiscal, Rede Societária GRC e Indicadores + Copilot de IA."

config:
  viewport:
    width: 1920
    height: 1080
  fps: 25
  voice_id: "bf91cd6a"
  model: "omnivoice"
  base_url: "http://localhost:3900"
  output_dir: "output"
  temp_dir: ".temp_demo"

scenes:
  # ── Aba 1: Pipeline RFB CNPJ ─────────────────────────────────────────────

  - id: "cena_01_abertura"
    title: "Abertura — Hero Cards"
    narration: |
      Olá pessoal! Bem-vindos ao Rust Sped Analytics.
      Aqui no painel do Pipeline RFB CNPJ, vejam as métricas.
      Sete vírgula cinco três vezes mais rápido que o Python legado.
      Setenta e sete por cento menos RAM.
      Cinquenta e quatro milhões e oitocentos mil registros processados.
      Throughput médio de cento e noventa e sete megabytes por segundo.
    pause_after: 1.0
    actions:
      - action: navigate
        url: "http://localhost:3000/rfb-cnpj"
        highlight: false
      - action: wait
        seconds: 1.5

  - id: "cena_02_timeline"
    title: "Timeline do Pipeline Zero-Copy"
    narration: |
      A timeline mostra os cinco estágios em sequência.
      Download HTTP dois zero seis resumível em quarenta e cinco segundos.
      Descompressão streaming zero-copy em doze segundos.
      Parse CSV multithread com Rayon em três minutos.
      Construção do grafo societário em noventa e cinco segundos.
      Indexação FTS cinco em quarenta e cinco segundos.
      Tudo sem escrever CSVs intermediários em disco.
    pause_after: 1.0
    actions:
      - action: hover
        selector: "text=Pipeline Stages"
        highlight: true
      - action: wait
        seconds: 1.0

  - id: "cena_03_graficos"
    title: "Gráficos Recharts — Rust vs Python"
    narration: |
      Nos gráficos, a área azul é o Rust, a amarela é o Python legado.
      O Rust mantém throughput estável e alto durante todo o processamento.
      No gráfico de barras, o estágio de parse CSV mostra oitocentos e noventa megabytes por segundo contra cento e vinte do Python.
    pause_after: 1.0
    actions:
      - action: hover
        selector: "text=Processing Speed"
        highlight: true
      - action: wait
        seconds: 1.0

  - id: "cena_04_filtro_grafo"
    title: "Filtro do Grafo Societário"
    narration: |
      O grafo societário tem filtros por tipo de entidade.
      Vou ciclar: empresas, sócios, sanções, e voltar a todos.
    pause_after: 0.8
    actions:
      - action: click
        selector: 'button:has-text("Todos")'
        highlight: true
      - action: wait
        seconds: 0.4
      - action: click
        selector: 'button:has-text("Empresas")'
        highlight: true
      - action: wait
        seconds: 0.4
      - action: click
        selector: 'button:has-text("Sócios")'
        highlight: true
      - action: wait
        seconds: 0.4
      - action: click
        selector: 'button:has-text("Sanções")'
        highlight: true
      - action: wait
        seconds: 0.4
      - action: click
        selector: 'button:has-text("Todos")'
        highlight: true

  - id: "cena_05_zoom_grafo"
    title: "Zoom e Enquadramento do Grafo"
    narration: |
      Posso aproximar, afastar e enquadrar o grafo com os controles flutuantes.
    pause_after: 0.8
    actions:
      - action: click
        selector: 'button[title="Aproximar Zoom"]'
        highlight: true
      - action: wait
        seconds: 0.3
      - action: click
        selector: 'button[title="Afastar Zoom"]'
        highlight: true
      - action: wait
        seconds: 0.3
      - action: click
        selector: 'button[title="Enquadrar Visualização"]'
        highlight: true

  - id: "cena_06_fts5_search"
    title: "Busca Full-Text FTS5"
    narration: |
      A busca FTS5 usa tokenizer Unicode sessenta e um com remoção de diacríticos.
      Busco por Petrobras, filtro por empresa ativa no Rio de Janeiro.
    pause_after: 0.8
    actions:
      - action: type
        selector: 'input[placeholder^="Search by"]'
        text: "Petrobras"
        highlight: true
      - action: press
        key: "Enter"
      - action: wait
        seconds: 0.5
      - action: select
        selector: "select"
        value: "empresa"
        highlight: true
      - action: wait
        seconds: 0.5
      - action: type
        selector: 'input[maxlength="2"]'
        text: "RJ"
        highlight: true
      - action: wait
        seconds: 0.5
      - action: click
        selector: 'button:has-text("Clear")'
        highlight: true

  - id: "cena_07_makefile"
    title: "MakefileController — Execução de Targets"
    narration: |
      O MakefileController tem dezessete targets não interativos.
      Seleciono o parse CSV, executo, e depois disparo o pipeline completo.
    pause_after: 0.8
    actions:
      - action: click
        selector: 'text=parse-csv-stream'
        highlight: true
      - action: wait
        seconds: 0.3
      - action: click
        selector: 'button:has-text("Run Selected Target")'
        highlight: true
      - action: wait
        seconds: 1.5
      - action: click
        selector: 'button:has-text("Run All Pipeline")'
        highlight: true
      - action: wait
        seconds: 1.5
      - action: click
        selector: 'button:has-text("Clear")'
        highlight: true

  - id: "cena_08_fim_aba1"
    title: "Fim da Aba RFB CNPJ"
    narration: |
      Isso conclui o painel do Pipeline RFB CNPJ.
      Vamos agora para a aba de Ingestão.
    pause_after: 0.8
    actions:
      - action: wait
        seconds: 1.0

  # ── Aba 2: Ingestão SPED ─────────────────────────────────────────────────

  - id: "cena_09_ingestao_kpis"
    title: "Ingestão — KPIs e Dropzone"
    narration: |
      Na aba de Ingestão vemos os KPIs: arquivos importados, tempo médio de parsing e o worker Redis ativo.
      A dropzone aceita arquivos SPED ECD em TXT ou ZIP.
    pause_after: 0.8
    actions:
      - action: navigate
        url: "http://localhost:3000/ingestao"
        highlight: false
      - action: wait
        seconds: 1.5

  - id: "cena_10_upload_fixture"
    title: "Upload do Fixture SPED"
    narration: |
      Vou enviar um arquivo SPED ECD de demonstração.
      O upload é multipart streaming direto para o worker assíncrono via fila Redis.
    pause_after: 1.0
    actions:
      - action: hover
        selector: "text=Nova Ingestão"
        highlight: true
      - action: wait
        seconds: 0.5

  - id: "cena_11_tabela_logs"
    title: "Tabela de Jobs e Logs SSE"
    narration: |
      A tabela mostra o histórico de importações.
      Filtro por SPED e abro os logs ao vivo, que chegam via Server-Sent Events.
    pause_after: 1.0
    actions:
      - action: type
        selector: 'input[placeholder^="Buscar por arquivo"]'
        text: "SPED"
        highlight: true
      - action: wait
        seconds: 0.5
      - action: click
        selector: 'button:has-text("Logs")'
        highlight: true
      - action: wait
        seconds: 2.0

  # ── Aba 3: Auditoria Fiscal ───────────────────────────────────────────────

  - id: "cena_12_fiscal_dre"
    title: "Auditoria Fiscal — DRE Petrobras"
    narration: |
      Na Auditoria Fiscal digito o CNPJ da Petrobras.
      A DRE apurada mostra Receita Bruta de quarenta e oito vírgula dois milhões, Receita de Serviços de trinta e cinco milhões e Outras Receitas de treze milhões.
    pause_after: 1.0
    actions:
      - action: navigate
        url: "http://localhost:3000/fiscal"
        highlight: false
      - action: wait
        seconds: 1.0
      - action: type
        selector: 'input[placeholder^="Digite o CNPJ"]'
        text: "33000167000101"
        highlight: true
      - action: press
        key: "Enter"
      - action: wait
        seconds: 1.5

  - id: "cena_13_exportar_xlsx"
    title: "Exportação XLSX"
    narration: |
      Com um clique exporto a DRE apurada para Excel com o motor nativo umya-spreadsheet.
    pause_after: 0.8
    actions:
      - action: click
        selector: 'button:has-text("Exportar")'
        highlight: true
      - action: wait
        seconds: 1.0

  - id: "cena_14_sped_tabs"
    title: "Blocos SPED — J150, J100, I155, 0000"
    narration: |
      As tabs mostram os quatro blocos oficiais do SPED ECD.
      J cento e cinquenta com a DRE. J cem com o balancete. I cento e cinquenta e cinco com os lançamentos. E o bloco zero zero zero zero com o cabeçalho.
    pause_after: 0.8
    actions:
      - action: click
        selector: '[role="tab"]:has-text("J150")'
        highlight: true
      - action: wait
        seconds: 0.4
      - action: click
        selector: '[role="tab"]:has-text("J100")'
        highlight: true
      - action: wait
        seconds: 0.4
      - action: click
        selector: '[role="tab"]:has-text("I155")'
        highlight: true
      - action: wait
        seconds: 0.4
      - action: click
        selector: '[role="tab"]:has-text("0000")'
        highlight: true
      - action: wait
        seconds: 0.4
      - action: type
        selector: 'input[placeholder^="Filtrar nesta tabela"]'
        text: "CNPJ"
        highlight: true

  # ── Aba 4: Rede Societária GRC ────────────────────────────────────────────

  - id: "cena_15_grc_expandir"
    title: "Rede Societária — Expandir Grafo"
    narration: |
      Na Rede Societária digito o CNPJ da Petrobras e expando o grafo.
      O algoritmo BFS busca os vínculos societários em tempo real.
    pause_after: 1.0
    actions:
      - action: navigate
        url: "http://localhost:3000/grc"
        highlight: false
      - action: wait
        seconds: 1.0
      - action: type
        selector: 'input[placeholder^="CNPJ ou CPF"]'
        text: "33000167000101"
        highlight: true
      - action: click
        selector: 'button:has-text("Expandir Grafo")'
        highlight: true
      - action: wait
        seconds: 2.0

  - id: "cena_16_slider_bfs"
    title: "Slider BFS — 1 a 10 Camadas"
    narration: |
      O slider controla as camadas do BFS de um a dez.
      Defino cinco camadas para uma análise mais profunda do grupo econômico.
    pause_after: 0.8
    actions:
      - action: fill
        selector: 'input[type="range"]'
        text: "5"
        highlight: true
      - action: wait
        seconds: 1.0

  - id: "cena_17_menor_caminho"
    title: "Menor Caminho — Dijkstra"
    narration: |
      O modo Menor Caminho usa Dijkstra para encontrar a conexão mais curta entre duas entidades.
    pause_after: 0.8
    actions:
      - action: click
        selector: 'button:has-text("Menor Caminho")'
        highlight: true
      - action: wait
        seconds: 1.5

  - id: "cena_18_drawer_no"
    title: "Drawer de Nó — Expansão BFS +1"
    narration: |
      Ao clicar num nó, o drawer lateral exibe os detalhes da entidade.
      Posso expandir um nível adicional ou buscar o menor caminho a partir daqui.
    pause_after: 0.8
    actions:
      - action: wait
        seconds: 1.0

  # ── Aba 5: Indicadores + Copilot ─────────────────────────────────────────

  - id: "cena_19_indicadores"
    title: "Indicadores Econômicos — Filtro por UF"
    narration: |
      Os indicadores mostram distribuição por CNAE, porte empresarial e regime tributário.
      Filtro por São Paulo e atualizo os dados.
    pause_after: 0.8
    actions:
      - action: navigate
        url: "http://localhost:3000/indicadores"
        highlight: false
      - action: wait
        seconds: 1.5
      - action: select
        selector: "select"
        value: "SP"
        highlight: true
      - action: wait
        seconds: 0.5
      - action: click
        selector: 'button:has-text("Atualizar")'
        highlight: true
      - action: wait
        seconds: 1.0

  - id: "cena_20_copilot_pergunta"
    title: "Copiloto de IA — Pergunta Fiscal"
    narration: |
      O Copiloto de IA permite consultas em linguagem natural.
      Vou perguntar sobre a receita de serviços da Petrobras em dois mil e vinte e três.
    pause_after: 1.0
    actions:
      - action: click
        selector: 'button:has-text("Copiloto de IA")'
        highlight: true
      - action: wait
        seconds: 1.0
      - action: type
        selector: 'input[placeholder^="Faça uma pergunta"]'
        text: "Qual a receita de serviços da Petrobras em 2023?"
        highlight: true
      - action: press
        key: "Enter"
      - action: wait
        seconds: 3.0

  - id: "cena_21_sugestoes_copilot"
    title: "Sugestões Rápidas do Copilot"
    narration: |
      O Copiloto também oferece sugestões de análise rápida.
      Disparo a apuração de DRE com geração de gráfico.
    pause_after: 0.8
    actions:
      - action: click
        selector: 'button:has-text("Apurar DRE")'
        highlight: true
      - action: wait
        seconds: 2.0

  # ── Encerramento ──────────────────────────────────────────────────────────

  - id: "cena_22_encerramento"
    title: "Encerramento"
    narration: |
      E é isso, Rust Sped Analytics.
      Pipeline RFB CNPJ, auditoria fiscal SPED, grafos societários GRC e inteligência setorial.
      Tudo em Rust nativo, testado e verificado.
      Muito obrigado por assistir!
    pause_after: 1.5
    actions:
      - action: navigate
        url: "http://localhost:3000/rfb-cnpj"
        highlight: false
      - action: wait
        seconds: 1.5
```

- [ ] **Step 2: Validate schema parses cleanly**

```bash
cd ~/.gemini/config/skills/web-demo-narrator
python3 -c "
from scripts.schema import load_demo_script
s = load_demo_script('/home/pizani/Projects/rust-sped-analitycs/demo_spa.yaml')
print(f'OK — {len(s.scenes)} cenas carregadas')
"
```

Expected: `OK — 22 cenas carregadas`.

- [ ] **Step 3: Run validate_spa_selectors.py**

```bash
cd /home/pizani/Projects/rust-sped-analitycs
python3 scripts/validate_spa_selectors.py demo_spa.yaml
```

Expected: all `✅`, exit 0. Fix any `❌` by adjusting the corresponding selector in `demo_spa.yaml`.

- [ ] **Step 4: Humanize and synthesize (frontend + VoiceStudio must be running)**

```bash
bash scripts/warm_tts.sh
cd ~/.gemini/config/skills/web-demo-narrator
python3 scripts/cli.py humanize /home/pizani/Projects/rust-sped-analitycs/demo_spa.yaml --diff
python3 scripts/cli.py synthesize /home/pizani/Projects/rust-sped-analitycs/demo_spa.yaml \
    --voice-id bf91cd6a --backend-url http://localhost:3900
```

Expected: humanize saves YAML, synthesize prints `✔ Áudios sintetizados` and updates each scene's `audio_path` and `audio_duration`.

- [ ] **Step 5: Dry-run record (headed, first 3 scenes only)**

Edit `demo_spa.yaml` temporarily to keep only scenes 1-3, then:

```bash
python3 scripts/cli.py record \
    /home/pizani/Projects/rust-sped-analitycs/demo_spa.yaml \
    --headed -o /home/pizani/Projects/rust-sped-analitycs/output/test_3scenes.webm
```

Expected: browser opens, 3 scenes play with spotlight, `.webm` written. Fix any seletor that times out.

Restore all 22 scenes after the test passes.

- [ ] **Step 6: Full record + mux**

```bash
python3 scripts/cli.py build \
    /home/pizani/Projects/rust-sped-analitycs/demo_spa.yaml \
    -o /home/pizani/Projects/rust-sped-analitycs/output/demo_spa_final.mp4
```

Expected: `✔ DEMO CONCLUÍDA COM SUCESSO!` and MP4 file present.

- [ ] **Step 7: Verify MP4 integrity**

```bash
ffprobe -v error -show_streams \
    /home/pizani/Projects/rust-sped-analitycs/output/demo_spa_final.mp4 \
    | grep codec_name
```

Expected: lines showing `h264` and `aac`.

- [ ] **Step 8: Commit**

```bash
cd /home/pizani/Projects/rust-sped-analitycs
git add demo_spa.yaml
git commit -m "feat: demo_spa.yaml — roteiro 22 cenas, seletores verificados, narração pt-BR"
```

---

## Self-Review

**Spec coverage:**
- §4 armadilhas → all four resolved: absolute URLs (T4), port 3000 (T4), warm_tts.sh (T1), verified selectors (T4 step 3).
- §5 selectors → all 32 entries appear in T3 (validator) and T4 (demo YAML).
- §6 cobertura 22 cenas → all 22 scenes present in T4 YAML block.
- §7 fixture → T2, using existing `gerar_ecd_minimo_pva()`.
- §8 validator → T3.
- §9 warm_tts.sh → T1.

**Step scan:**
- Every step names exactly one thing to write or run. No "TBD" or "add validation" lines.
- Function bodies only appear in T1 (shell script, fully pinned by the spec) and T3 (Python script, fully pinned by selector table from spec §5). T4 YAML is fully written — no body the spec left open.

**Type consistency:**
- `gerar_ecd_minimo_pva(caminho_saida: str, cnpj: Optional[str]) -> Tuple[str, str]` — used in T2 step 1, matches `gerar_massa_sped.py:137`.
- `load_demo_script(path: Union[str, Path]) -> DemoScript` — used in T4 step 2, matches `schema.py:248`.
- `select` action in T4 YAML uses `value: "SP"` and `value: "empresa"` — matches `_parse_action` validation at `schema.py:332`.
- `fill` action in T4 YAML (slider) uses `text: "5"` — matches `Action.text` field, runner calls `loc.fill(text_val)` at `browser_runner.py:125`.

**Review Focus items → task coverage:**
1. Copilot collapsed → T3 checks `button:has(svg.lucide-bot)` on `/rfb-cnpj`; T4 cena_20 uses `button:has-text("Copiloto de IA")` from `/indicadores` where sidebar is expanded — selector is correct for that route.
2. `select` requires `value` → T4 cenas 06, 19 use `action: select` with `value:` field. T4 step 2 schema validation catches missing `value`.
3. Hidden file input → T3 uses `"present"` mode (count > 0); T4 cena_10 uses `hover` on visible text instead of clicking the hidden input directly — avoids the visibility issue.
4. BFS slider → T4 cena_16 uses `action: fill`, not `type`.
5. Two `button:has-text("Clear")` on `/rfb-cnpj` → T4 cena_06 clears FTS5 (above MakefileController), cena_07 clears Makefile logs (below). The scenes scroll by hover/click ordering; T3 validates each in the correct page section.

**Proportion:** plan is longer than spec because T4 contains the full YAML. The YAML is the artefact, not analysis — it must be fully written here so the implementer can paste it without ambiguity.
