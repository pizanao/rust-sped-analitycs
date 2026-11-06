---
name: web-demo-narrator
description: Gravação automatizada de demonstrações e testes de aplicações web em 1080p com narração simultânea em voz clonada (VoiceStudio) e revisão oral humanizada (pt-BR). Use quando o usuário quiser gravar um vídeo demonstrativo de uma aplicação web com voz natural guiando a tela.
---

# Web Demo Narrator

Skill para automação de vídeos de demonstração e testes de aplicações web no terminal via AGY, combinando **Playwright** (gravação 1080p com destaque visual), **VoiceStudio** (síntese neural local com voz clonada, ex: `Considera` / `bf91cd6a`), **Humanização de Fala** (baseado no `/humanizer`) e **FFmpeg** (sincronia audio-driven sem cortes bruscos).

---

## 🎯 Capacidades Principais

1. **Roteiro Declarativo (`demo.yaml`)**:
   Separação total entre a lógica de interface (ações no navegador) e o script de locução.
2. **Motor de Humanização de Fala (pt-BR)**:
   Elimina fórmulas artificiais de IA (*"não apenas X, mas Y"*, clichês dramáticos, tríades forçadas) e injeta pontuação de respiração oral e conectivos coloquiais naturais (*"olha só"*, *"reparem que"*), preservando intactos seletores CSS, URLs e parâmetros técnicos.
3. **Sincronização Audio-Driven com Spotlight**:
   O Playwright calcula o tempo da fala de cada cena e mantém o foco no elemento pelo tempo exato da narração antes de disparar o próximo passo. Elementos em foco recebem uma animação de contorno iluminado suave (*spotlight*).
4. **Masterização e Multiplexação FFmpeg**:
   Gera pausas de silêncio configuráveis entre cenas, alinha a trilha de voz e estende/congela o frame final do vídeo se necessário (`tpad`), exportando um arquivo MP4 (H.264 / AAC 48kHz mono/estéreo) sem dessincronia.

---

## 🚀 Como Usar

### 1. Criar ou Editar o Roteiro (`demo.yaml`)

```yaml
title: "Demonstração do Painel Web"
description: "Fluxo interativo de navegação e busca de relatórios"

config:
  viewport:
    width: 1920
    height: 1080
  fps: 25
  voice_id: "bf91cd6a"       # Perfil 'Considera' cadastrado no VoiceStudio
  model: "omnivoice"          # Modelo TTS local
  base_url: "http://localhost:5173"
  output_dir: "output"
  temp_dir: ".temp_demo"

scenes:
  - id: "scene_01"
    title: "Abertura"
    narration: |
      Olá pessoal! Sejam muito bem-vindos à demonstração do nosso painel. Aqui gerenciamos fluxos e relatórios em tempo real.
    pause_after: 0.8
    actions:
      - action: "navigate"
        url: "/"
        highlight: false

  - id: "scene_02"
    title: "Pesquisa Dinâmica"
    narration: |
      Vejam como é simples. Basta digitar o termo desejado no campo de busca e a tabela já atualiza na hora.
    pause_after: 0.8
    actions:
      - action: "type"
        selector: "input[name='search']"
        text: "Relatório de Vendas"
        highlight: true
      - action: "click"
        selector: "#btn-filtrar"
        highlight: true
```

---

### 2. Comandos CLI no Terminal

Todos os comandos são executados a partir de `~/.gemini/config/skills/web-demo-narrator`:

#### A. Revisar e Humanizar o Texto da Narração
Exibe um diff comparativo no terminal e grava o roteiro limpo de vícios de IA:
```bash
python3 scripts/cli.py humanize demo.yaml --diff
```

#### B. Pré-sintetizar Áudios e Calcular Durações
Gera os arquivos WAV via VoiceStudio (`http://localhost:3900`), calculando as durações exatas com `ffprobe` e salvando o cache:
```bash
python3 scripts/cli.py synthesize demo.yaml --voice-id bf91cd6a
```

#### C. Gravar a Demonstração Web
Abre o Playwright e grava a navegação, aguardando a duração de cada fala:
```bash
# Modo Headless (padrão em segundo plano)
python3 scripts/cli.py record demo.yaml -o output/gravacao.webm

# Modo Visível na Tela (para acompanhar a execução)
python3 scripts/cli.py record demo.yaml --headed -o output/gravacao.webm
```

#### D. Pipeline Completo (`build`)
Executa o fluxo completo (humanização $\to$ síntese TTS $\to$ gravação Playwright $\to$ mixagem final FFmpeg):
```bash
python3 scripts/cli.py build demo.yaml -o output/demo_final.mp4
```

---

## 🛠️ Ações Suportadas no Roteiro

| Ação | Parâmetros | Descrição |
| :--- | :--- | :--- |
| `navigate` | `url` | Abre a página informada (suporta URLs `http`, `https` ou arquivos locais `file://` / relativos). |
| `click` | `selector`, `highlight: true/false` | Clica no elemento especificado com destaque visual prévio. |
| `type` / `fill` | `selector`, `text`, `highlight: true/false` | Digita o texto no campo com cadência natural. |
| `select` | `selector`, `value` | Seleciona uma opção em um `<select>`. |
| `hover` | `selector` | Passa o cursor sobre o elemento. |
| `press` | `key` | Pressiona uma tecla (ex: `Enter`, `Tab`, `Escape`). |
| `scroll` | `x`, `y` | Rola a janela pelas coordenadas indicadas. |
| `wait` | `seconds` | Pausa explícita adicional no navegador. |
| `screenshot` | - | Registra um quadro estático. |

---

## ⚙️ Pré-requisitos do Sistema

* **Python 3.10+** com `playwright`, `pyyaml`, `rich`, `requests`
* **Chromium** instalado via Playwright: `playwright install chromium`
* **FFmpeg** e **ffprobe** no PATH
* **VoiceStudio** rodando localmente na porta 3900:
  ```bash
  curl http://localhost:3900/health
  # Retorna: {"status":"ok","device":"cuda (NVIDIA GeForce RTX 4060)","version":"0.5.4"}
  ```
