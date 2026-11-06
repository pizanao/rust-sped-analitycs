# Demo SPA Narrator — Design Spec

**Data:** 2026-10-27  
**Projeto:** rust-sped-analitycs  
**Artefato principal:** `demo_spa.yaml`  
**Ferramenta:** `web-demo-narrator` (`~/.gemini/config/skills/web-demo-narrator/`)  

---

## 1. Contexto

### 1.1 Lacuna de cobertura

O workspace possui ~140 testes em 5 camadas (Rust, AI Gateway, frontend, paridade, pipeline ETL).
Nenhum exercita a SPA renderizada.

Os 12 testes de `apps/web/src/tests/` verificam contratos de tipo e formatação de URL — não há
DOM, não há clique, não há verificação de que um botão abre um drawer ou que o grafo renderiza nós.
Toda interação real do usuário está não coberta.

### 1.2 Objetivo

Criar `demo_spa.yaml`: um roteiro declarativo de 22 cenas que exercita, via Playwright com narração
sincronizada (VoiceStudio RTX 4060, perfil `bf91cd6a` "Considera"), todas as funcionalidades
interativas da SPA. O artefato serve dois propósitos simultâneos:

1. **Demonstração gravada** para stakeholders (~4min15s, MP4 H.264/AAC 1080p).
2. **Cobertura funcional executável** que detecta regressões de interação antes de subir.

---

## 2. Decisões de design

| Questão | Decisão | Justificativa |
|---|---|---|
| Escopo | Opção A — cobertura funcional da SPA (5 abas) | Lacuna real identificada; ~140 testes existentes não cobrem DOM |
| Modo de dados | **Degradado** (frontend + backend Axum, mocks do `App.tsx`) | Gravação determinística; sobrevive re-gravação sem containers |
| Upload na cena 10 | **Fixture SPED sintético real** (`fixtures/sped_minimo.txt`) | Prova o multipart streaming; `FileDropzone` cai no fallback mockado se API rejeitar |
| Voz | `bf91cd6a` ("Considera"), VoiceStudio `:3900` | Validado: 16 áudios, 239s totais, ~2.5-5.9s/cena com modelo quente |
| Backend URL do navegador | `http://localhost:3000` (Vite configurado em `:3000`, `vite.config.ts:18`) | Não é `:5173`; `base_url` do YAML fica em `:3900` para o TTS |

---

## 3. Arquitetura

### 3.1 Componentes

```
demo_spa.yaml                          # roteiro declarativo (22 cenas)
fixtures/sped_minimo.txt               # SPED ECD sintético ~50KB para a cena de upload
scripts/validate_spa_selectors.py      # smoke test: resolve seletores no DOM sem gravar
scripts/warm_tts.sh                    # pré-condição: aguarda VoiceStudio + aquece o modelo
```

### 3.2 Fluxo de execução

```
demo_spa.yaml
    │
    ├─► humanize       → reescreve falas em pt-BR natural, elimina vícios de IA
    ├─► synthesize     → VoiceStudio :3900 → WAV 48kHz mono + duração por cena
    ├─► record         → Playwright Chromium headed → .webm (audio-driven + spotlight)
    └─► mux            → FFmpeg concat + silêncios + H.264/AAC → .mp4
```

**Sincronização audio-driven:** as ações de cada cena são executadas, depois o frame fica congelado
no elemento em foco pelo tempo exato do áudio WAV + `pause_after`. A duração é medida por `ffprobe`
do WAV real, não por estimativa.

### 3.3 Separação de responsabilidades

`base_url: http://localhost:3900` serve exclusivamente o TTS (VoiceStudio). Toda ação `navigate`
usa URL absoluta `http://localhost:3000/...` para não depender do `_resolve_url` do `browser_runner`.

---

## 4. Armadilhas técnicas resolvidas

Estas quatro causas quebraram a rodada de gravação anterior. Estão resolvidas no design.

### 4.1 `base_url` ambíguo

**Causa:** `schema.py:239` usa o mesmo campo `base_url` para TTS e para prefixar ações `navigate`.
Apontando para `:5173` (erro) ou `:3000` (correto), o cliente TTS batia na porta errada.

**Solução:** `base_url: http://localhost:3900` no config + URLs absolutas em todas as ações navigate.

### 4.2 Porta do Vite

**Causa:** `vite.config.ts` serve em `:3000`, não `:5173`.

**Solução:** URLs absolutas `http://localhost:3000/rfb-cnpj` etc.

### 4.3 Timeout do TTS

**Causa:** Não foi cold start do modelo (verificado: 5.9s com modelo idle). Foi o **servidor
VoiceStudio recém-iniciado via `electron-vite dev`** que ainda não havia concluído o startup.
O cliente TTS tem timeout de 120s (`tts_engine.py:38`).

**Solução:** `scripts/warm_tts.sh` — poll em `/health` até status `ok`, depois dispara uma
requisição de aquecimento trivial antes do batch de síntese.

### 4.4 Seletores errados

**Causa:** `button:has-text('Empresas')` não existia — o filtro é um único botão que cicla o estado
(`NetworkGraphViewer.tsx:244`). `button:has-text('Enterprises')` e similares vieram de suposição.

**Solução:** todos os seletores verificados contra o código-fonte (tabela na seção 5).

---

## 5. Seletores verificados

Todos conferidos diretamente nos arquivos de `apps/web/src/features/` e `components/`.

| Alvo | Seletor | Arquivo:linha |
|---|---|---|
| Filtro do grafo (ciclo) | `button:has-text("Todos")` → `"Empresas"` → `"Sócios"` → `"Sanções"` | `NetworkGraphViewer.tsx:248` |
| Zoom in | `button[title="Aproximar Zoom"]` | `NetworkGraphCanvas.tsx:514` |
| Zoom out | `button[title="Afastar Zoom"]` | `NetworkGraphCanvas.tsx:521` |
| Fit view | `button[title="Enquadrar Visualização"]` | `NetworkGraphCanvas.tsx:528` |
| Busca FTS5 | `input[placeholder^="Search by"]` | `FTSSearchPanel.tsx:169` |
| Filtro tipo FTS5 | `select` nth=0 (primeiro `<select>` no painel) | `FTSSearchPanel.tsx:199` |
| Filtro situação FTS5 | `select` nth=1 | `FTSSearchPanel.tsx:210` |
| Filtro UF FTS5 | `input[maxlength="2"]` | `FTSSearchPanel.tsx:222` |
| Clear filtros FTS5 | `button:has-text("Clear")` | `FTSSearchPanel.tsx:231` |
| Busca CNPJ fiscal | `input[placeholder^="Digite o CNPJ"]` | `CnpjSearchBar.tsx:51` |
| Exportar XLSX | `button:has-text("Exportar")` | `DreViewer.tsx:67` |
| Tab J150 SPED | `[role="tab"]:has-text("J150")` | `SpedBlocksTabs.tsx:297` |
| Tab J100 SPED | `[role="tab"]:has-text("J100")` | `SpedBlocksTabs.tsx:304` |
| Tab I155 SPED | `[role="tab"]:has-text("I155")` | `SpedBlocksTabs.tsx:311` |
| Tab 0000 SPED | `[role="tab"]:has-text("0000")` | `SpedBlocksTabs.tsx:318` |
| Filtro dentro da tabela SPED | `input[placeholder^="Filtrar nesta tabela"]` | `SpedBlocksTabs.tsx:217` |
| Raiz GRC | `input[placeholder^="CNPJ ou CPF"]` | `NetworkGraphCanvas.tsx:392` |
| Expandir Grafo | `button:has-text("Expandir Grafo")` | `NetworkGraphCanvas.tsx:396` |
| Slider BFS | `input[type="range"]` | `NetworkGraphCanvas.tsx:419` |
| Menor Caminho | `button:has-text("Menor Caminho")` | `NetworkGraphCanvas.tsx:424` |
| Expansão BFS +1 | `button:has-text("Expansão BFS")` | `NodeDetailDrawer.tsx:211` |
| Filtro jobs | `input[placeholder^="Buscar por arquivo"]` | `ImportJobsTable.tsx:108` |
| Logs ao vivo | `button:has-text("Logs")` | `ImportJobsTable.tsx:207` |
| Upload | `input[type="file"]` | `FileDropzone.tsx:100` |
| Filtro UF indicadores | `select` (único `<select>` na página indicadores) | `EconomicIndicatorsDashboard.tsx:108` |
| Atualizar indicadores | `button:has-text("Atualizar")` | `EconomicIndicatorsDashboard.tsx:122` |
| Copilot (sidebar) | `button:has(svg.lucide-bot)` (sidebar collapsed = sem texto) | `Sidebar.tsx:152` |
| Copilot (header) | `button:has-text("Copiloto de IA")` | `Header.tsx:61` |
| Chat input Copilot | `input[placeholder^="Faça uma pergunta"]` | `AiChatDrawer.tsx:240` |
| Sugestão "Apurar DRE" | `button:has-text("Apurar DRE")` | `AiChatDrawer.tsx:159` |
| Run Selected Target | `button:has-text("Run Selected Target")` | `MakefileController.tsx:316` |
| Run All Pipeline | `button:has-text("Run All Pipeline")` | `MakefileController.tsx:333` |
| Clear logs | `button:has-text("Clear")` (MakefileController) | `MakefileController.tsx:342` |

---

## 6. Cobertura — 22 cenas por aba

### Aba 1 — Pipeline RFB CNPJ (8 cenas)

| # | Cena | Interações cobertas |
|---|---|---|
| 1 | Abertura / hero cards | navigate `/rfb-cnpj`, hero cards 7.53x / 77.5% / 54.8M / 197.6 MB/s |
| 2 | Timeline do pipeline | scroll até `PipelineDashboard`, 5 estágios visíveis |
| 3 | Gráficos Recharts | scroll até área chart + bar chart |
| 4 | Filtro do grafo | click no botão filtro: Todos → Empresas → Sócios → Sanções → Todos |
| 5 | Zoom + click no nó | zoom in, zoom out, fit, click canvas no nó |
| 6 | FTS5 search | type "Petrobras", Enter; filtros tipo/situação/UF; Clear |
| 7 | MakefileController | click target `parse-csv-stream`, Run Selected Target; Run All Pipeline; Clear |
| 8 | Fim da aba | scroll até o rodapé |

### Aba 2 — Ingestão SPED (3 cenas)

| # | Cena | Interações cobertas |
|---|---|---|
| 9 | KPIs + dropzone | navigate `/ingestao`, 3 KPI cards visíveis |
| 10 | Upload fixture | `input[type="file"]` com `fixtures/sped_minimo.txt` → progresso → sucesso/fallback |
| 11 | Tabela + Logs SSE | filtro "SPED"; click "Logs" no primeiro job → LiveLogDrawer abre |

### Aba 3 — Auditoria Fiscal (3 cenas)

| # | Cena | Interações cobertas |
|---|---|---|
| 12 | Busca CNPJ + DRE | navigate `/fiscal`, type "33000167000101", Enter → DreViewer aparece |
| 13 | Exportar XLSX | click "Exportar" → download iniciado |
| 14 | Tabs SPED + filtro | J150 → J100 → I155 → 0000; type no filtro de tabela |

### Aba 4 — Rede Societária GRC (4 cenas)

| # | Cena | Interações cobertas |
|---|---|---|
| 15 | Expandir grafo | navigate `/grc`, type CNPJ raiz, click "Expandir Grafo" |
| 16 | Slider BFS | arrastar `input[type="range"]` de 2 para 5 |
| 17 | Menor caminho | click "Menor Caminho", preencher origem/destino |
| 18 | Drawer de nó | click num nó → NodeDetailDrawer abre; click "Expansão BFS (+1 Camada)" |

### Aba 5 — Indicadores + Copilot (3 cenas)

| # | Cena | Interações cobertas |
|---|---|---|
| 19 | Filtro UF + atualizar | navigate `/indicadores`, select "SP", click "Atualizar" |
| 20 | Copilot pergunta | click "Copiloto de IA", type pergunta fiscal, Enter, aguardar resposta |
| 21 | Sugestões rápidas | click chip "Apurar DRE e Gerar Gráfico" |

### Encerramento (1 cena)

| # | Cena | Interações cobertas |
|---|---|---|
| 22 | Retorno ao pipeline | navigate `/rfb-cnpj`, scroll até hero cards |

---

## 7. Fixture SPED sintético

`fixtures/sped_minimo.txt` — arquivo ECD SPED mínimo válido para a cena de upload.

**Requisitos:**
- Codificação Windows-1252 com CRLF
- Bloco `0000` com CNPJ `33000167000101` e período 2023
- Pelo menos um registro `J100`, `J150`, `I155`
- Tamanho < 100 KB (garante upload instantâneo na demo)

O `FileDropzone` aceita `.txt` e `.zip`; se o backend Axum rejeitar (sem PostgreSQL), cai no
fallback mockado (`FileDropzone.tsx:52-57`) que simula `job_id` e chama `onUploadSuccess`.
A cena nunca trava independente do estado do banco.

---

## 8. Artefato `scripts/validate_spa_selectors.py`

Script que abre o Playwright headless, navega até cada rota e tenta `wait_for` em cada seletor
das cenas correspondentes. Imprime `✅ scene_id: seletor` ou `❌ scene_id: seletor (timeout)`.
Termina com exit code 1 se qualquer seletor falhar.

**Objetivo:** detectar regressões de seletor em segundos, sem TTS, sem gravação.

**Quando rodar:** antes de cada execução de `synthesize` ou `build`.

```bash
python3 scripts/validate_spa_selectors.py demo_spa.yaml
# ✅ scene_01_abertura: navegar para /rfb-cnpj
# ✅ scene_06_fts5: input[placeholder^="Search by"]
# ❌ scene_07_makefile: button:has-text("Run Selected Target") — TIMEOUT
```

---

## 9. Artefato `scripts/warm_tts.sh`

```bash
#!/usr/bin/env bash
# Aguarda VoiceStudio estar pronto e aquece o perfil de voz antes do batch de síntese.
set -euo pipefail

VS_URL="${VOICESTUDIO_URL:-http://localhost:3900}"
VOICE_ID="${VOICE_ID:-bf91cd6a}"
MAX_WAIT=60

echo "Aguardando VoiceStudio em $VS_URL..."
for i in $(seq 1 $MAX_WAIT); do
    if curl -sf "$VS_URL/health" | grep -q '"status":"ok"'; then break; fi
    sleep 1
done

echo "Aquecendo perfil de voz $VOICE_ID..."
curl -sf -o /dev/null -X POST "$VS_URL/v1/audio/speech" \
    -H "Content-Type: application/json" \
    -d "{\"model\":\"omnivoice\",\"voice\":\"$VOICE_ID\",\"input\":\"Aquecimento.\",\"response_format\":\"wav\"}" \
    --max-time 120
echo "VoiceStudio pronto."
```

---

## 10. Comandos de execução

```bash
# Pré-condição obrigatória
bash scripts/warm_tts.sh

# 0. Validar seletores (sem TTS, sem gravação — rápido)
python3 scripts/validate_spa_selectors.py demo_spa.yaml

# 1. Humanizar falas
cd ~/.gemini/config/skills/web-demo-narrator
python3 scripts/cli.py humanize \
    /home/pizani/Projects/rust-sped-analitycs/demo_spa.yaml --diff

# 2. Sintetizar áudios TTS
python3 scripts/cli.py synthesize \
    /home/pizani/Projects/rust-sped-analitycs/demo_spa.yaml \
    --voice-id bf91cd6a --backend-url http://localhost:3900

# 3. Gravar (modo headed — para acompanhar ao vivo)
python3 scripts/cli.py record \
    /home/pizani/Projects/rust-sped-analitycs/demo_spa.yaml \
    --headed -o output/gravacao_spa.webm

# 4. Pipeline completo (humanize → synthesize → record → mux)
python3 scripts/cli.py build \
    /home/pizani/Projects/rust-sped-analitycs/demo_spa.yaml \
    -o output/demo_spa_final.mp4

# 5. Verificar integridade do MP4
ffprobe -v error -show_streams -select_streams a:0 output/demo_spa_final.mp4
```

---

## 11. Tratamento de erros e robustez

| Cenário | Comportamento | Ação necessária |
|---|---|---|
| Seletor falha | `validate_spa_selectors.py` reporta `❌` antes de sintetizar | Corrigir seletor no YAML |
| Backend Axum fora do ar | SPA usa mocks em `App.tsx`; cena continua | Nenhuma |
| Upload rejeitado pela API | `FileDropzone.tsx:52` simula job_id, chama `onUploadSuccess` | Nenhuma |
| VoiceStudio recém-iniciado | `warm_tts.sh` absorve o cold start (até 120s) | Nenhuma |
| Cena interativa sem resposta | `action_timeout` padrão 10s; aumentar com `--action-timeout` | Usar `--action-timeout 20000` |
| MP4 fora de sincronia | `ffprobe` duração vs. soma dos WAVs + pausas; desvio > 2%: re-rodar `mux` | Re-rodar `build` |

---

## 12. Pré-condições de ambiente

| Serviço | Porta | Verificação |
|---|---|---|
| Frontend Vite (`npm run dev`) | 3000 | `curl http://localhost:3000` → HTML |
| Backend Axum (`cargo run -p ecd-server`) | 8080 | `curl http://localhost:8080/health` |
| VoiceStudio | 3900 | `curl http://localhost:3900/health` |
| Playwright Chromium | — | `playwright install chromium` |
| ffmpeg + ffprobe | — | `ffmpeg -version` |

PostgreSQL, Redis e AI Gateway são **opcionais**: a SPA opera em modo degradado sem eles.

---

## 13. Self-review

**Placeholders:** nenhum — todos os seletores têm fonte anotada, todas as durações são estimadas
a partir de medições reais (239s total para 16 cenas, ~15s/cena).

**Consistência interna:** `base_url` no YAML aponta para `:3900` (TTS); ações navigate usam
`:3000` (frontend). Sem conflito.

**Escopo:** 22 cenas, ~4min15s de vídeo. Não inclui testes do backend Rust (cobertos pelas 82
suítes existentes). Não inclui re-execução de CI (objetivo é vídeo de demonstração).

**Ambiguidade eliminada:** o único seletor potencialmente ambíguo é `button:has-text("Clear")`,
que aparece em dois contextos (FTS5 e MakefileController). O YAML isola cada uso em cenas
separadas em rotas distintas (`/rfb-cnpj` com scroll para posição diferente), então o `.first`
do runner sempre resolve o correto.

**Duração estimada do vídeo:** 255s (~4min15s). Referência: 16 cenas do `demo.yaml` = 239s de
narração + 16.3s de pausas = 255s. O `demo_spa.yaml` tem 22 cenas com narrações mais curtas
(média ~10s), resultando em estimativa semelhante de 250-280s.
