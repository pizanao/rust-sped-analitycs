# ==============================================================================
# Rust Sped Analytics - Makefile de Automação, Build, Logs e Ciclo de Vida
# ==============================================================================
SHELL          := /bin/bash
.SHELLFLAGS    := -eu -o pipefail -c
.DEFAULT_GOAL  := help

# Variáveis configuráveis
VERSION        ?= $(shell grep -m1 '^version = ' Cargo.toml 2>/dev/null | cut -d '"' -f 2 || echo "0.1.0")
DOCKER_COMPOSE ?= docker compose
STAGE_DIR      := media/ecd_stage
DIST_DIR       := dist

# Paleta de cores para o terminal
BLUE   := \033[1;34m
GREEN  := \033[1;32m
YELLOW := \033[1;33m
RED    := \033[1;31m
CYAN   := \033[1;36m
RESET  := \033[0m

.PHONY: all help
all: help

## Ajuda interativa
help: ## Exibe esta mensagem de ajuda com todos os comandos disponíveis
	@echo -e "${BLUE}====================================================================${RESET}"
	@echo -e "${BLUE}  Rust Sped Analytics - Automação do Projeto (v$(VERSION))${RESET}"
	@echo -e "${BLUE}====================================================================${RESET}"
	@echo -e "Uso: ${GREEN}make${RESET} ${CYAN}<alvo>${RESET}\n"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  ${CYAN}%-20s${RESET} %s\n", $$1, $$2}' $(MAKEFILE_LIST)
	@echo ""

# ==============================================================================
# CICLO DE VIDA (START, STOP, RESTART, STATUS)
# ==============================================================================
.PHONY: start up stop down restart status ps init-env clean-all

init-env: ## Garante arquivos de ambiente e diretórios de execução
	@mkdir -p $(STAGE_DIR)/aguardando $(STAGE_DIR)/processando $(STAGE_DIR)/processados $(STAGE_DIR)/erro
	@if [ ! -f .env ]; then \
		if [ -f .env.example ]; then \
			echo -e "${YELLOW}Criando .env a partir de .env.example...${RESET}"; \
			cp .env.example .env; \
		else \
			echo -e "${RED}Aviso: .env.example não encontrado!${RESET}"; \
		fi \
	fi

start: init-env ## Inicia todos os serviços em background via Docker Compose (alias: up)
	@echo -e "${GREEN}Iniciando serviços da aplicação via Docker Compose...${RESET}"
	@$(DOCKER_COMPOSE) up -d
	@echo -e "${GREEN}Serviços iniciados com sucesso!${RESET}"
	@$(DOCKER_COMPOSE) ps

up: start ## Alias para make start

stop: ## Para todos os serviços em execução via Docker Compose (alias: down)
	@echo -e "${YELLOW}Finalizando serviços da aplicação...${RESET}"
	@$(DOCKER_COMPOSE) down
	@echo -e "${GREEN}Serviços finalizados.${RESET}"

down: stop ## Alias para make stop

restart: stop start ## Reinicia todos os containers da aplicação

status: ## Exibe o status atual e portas de todos os containers
	@$(DOCKER_COMPOSE) ps

ps: status ## Alias para make status

clean-all: ## Para os containers e remove todos os volumes persistentes do banco/cache
	@echo -e "${RED}ATENÇÃO: Removendo containers e volumes (pgdata, redisdata)...${RESET}"
	@$(DOCKER_COMPOSE) down -v

# ==============================================================================
# MONITORAMENTO DE LOGS
# ==============================================================================
.PHONY: logs logs-server logs-worker logs-web logs-ai logs-db

logs: ## Exibe e acompanha os logs unificados de todos os serviços (tail=100)
	@$(DOCKER_COMPOSE) logs -f --tail=100

logs-server: ## Acompanha os logs da API Axum (server)
	@$(DOCKER_COMPOSE) logs -f --tail=100 server

logs-worker: ## Acompanha os logs do consumer worker de arquivos SPED
	@$(DOCKER_COMPOSE) logs -f --tail=100 worker

logs-web: ## Acompanha os logs do frontend SPA web (Nginx)
	@$(DOCKER_COMPOSE) logs -f --tail=100 web

logs-ai: ## Acompanha os logs do AI Gateway (FastAPI)
	@$(DOCKER_COMPOSE) logs -f --tail=100 ai-gateway

logs-db: ## Acompanha os logs do PostgreSQL e Redis
	@$(DOCKER_COMPOSE) logs -f --tail=100 postgres redis

# ==============================================================================
# BUILD (DOCKER & NATIVO)
# ==============================================================================
.PHONY: build build-native build-server build-worker build-web build-server-image build-worker-image build-web-image build-ai-image

build: ## Constrói as imagens Docker de todos os serviços
	@echo -e "${GREEN}Construindo imagens Docker via Docker Compose...${RESET}"
	@$(DOCKER_COMPOSE) build

build-server-image: ## Constrói apenas a imagem Docker do servidor Axum
	@$(DOCKER_COMPOSE) build server

build-worker-image: ## Constrói apenas a imagem Docker do worker
	@$(DOCKER_COMPOSE) build worker

build-web-image: ## Constrói apenas a imagem Docker do frontend web
	@$(DOCKER_COMPOSE) build web

build-ai-image: ## Constrói apenas a imagem Docker do AI Gateway
	@$(DOCKER_COMPOSE) build ai-gateway

build-native: build-server build-worker ## Compila binários Rust nativos (--release) e frontend
	@if command -v npm >/dev/null 2>&1 && [ -d apps/web ]; then \
		echo -e "${CYAN}Compilando frontend SPA estático...${RESET}"; \
		(cd apps/web && npm run build); \
	fi
	@echo -e "${GREEN}Build nativo concluído!${RESET}"

build-server: ## Compila o binário Rust nativo do servidor Axum em modo --release
	@echo -e "${CYAN}Compilando ecd-server (--release)...${RESET}"
	@cargo build --release -p ecd-server

build-worker: ## Compila o binário Rust nativo do worker em modo --release
	@echo -e "${CYAN}Compilando ecd-worker (--release)...${RESET}"
	@cargo build --release -p ecd-worker

build-web: ## Instala dependências e compila o frontend web estático
	@echo -e "${CYAN}Instalando e compilando apps/web...${RESET}"
	@cd apps/web && npm install && npm run build

# ==============================================================================
# EMPACOTAMENTO (PACKAGING & DISTRIBUIÇÃO)
# ==============================================================================
.PHONY: package pack package-force package-docker

package: ## Empacota a aplicação completa gerando o arquivo dist/rust-sped-analytics-$(VERSION).tar.gz
	@bash scripts/package.sh

pack: package ## Alias para make package

package-force: ## Força recompilação completa e gera o pacote de distribuição
	@FORCE_BUILD=1 bash scripts/package.sh

package-docker: build ## Salva as imagens Docker em tarballs compactados para ambientes offline
	@mkdir -p $(DIST_DIR)/images
	@echo -e "${CYAN}Salvando imagens Docker em $(DIST_DIR)/images/...${RESET}"
	@docker save rust-sped-analitycs-server | gzip > $(DIST_DIR)/images/rust-sped-server.tar.gz || true
	@docker save rust-sped-analitycs-worker | gzip > $(DIST_DIR)/images/rust-sped-worker.tar.gz || true
	@docker save rust-sped-analitycs-web | gzip > $(DIST_DIR)/images/rust-sped-web.tar.gz || true
	@docker save rust-sped-analitycs-ai-gateway | gzip > $(DIST_DIR)/images/rust-sped-ai-gateway.tar.gz || true
	@echo -e "${GREEN}Imagens salvas com sucesso em $(DIST_DIR)/images/${RESET}"

# ==============================================================================
# QUALIDADE, TESTES E CHECAGEM (NATIVO)
# ==============================================================================
.PHONY: check test test-unit test-web lint fmt fmt-check clippy clean

check: ## Executa checagem de compilação rápida em todo o workspace Rust
	@cargo check --workspace

test: ## Executa todos os testes unitários e de integração do workspace Rust
	@echo -e "${CYAN}Executando testes do workspace Rust...${RESET}"
	@cargo test --workspace

test-unit: ## Executa apenas testes das bibliotecas (crates)
	@cargo test --workspace --lib

test-web: ## Executa os testes do frontend SPA
	@if [ -d apps/web ]; then \
		cd apps/web && npm test; \
	fi

fmt: ## Formata todo o código Rust com rustfmt
	@cargo fmt --all

fmt-check: ## Verifica a formatação do código Rust sem aplicar alterações
	@cargo fmt --all -- --check

clippy: ## Executa o linter clippy no workspace
	@cargo clippy --workspace -- -D warnings

lint: fmt-check clippy ## Executa todas as validações de formatação e linter

clean: ## Limpa artefatos temporários de compilação (target/ e dist/)
	@echo -e "${YELLOW}Limpando diretórios target/ e dist/...${RESET}"
	@cargo clean
	@rm -rf $(DIST_DIR)
	@echo -e "${GREEN}Limpeza concluída.${RESET}"

# ==============================================================================
# UTILITÁRIOS E BANCO DE DADOS
# ==============================================================================
.PHONY: db-shell redis-cli generate-mock

db-shell: ## Abre sessão psql interativa dentro do container PostgreSQL
	@$(DOCKER_COMPOSE) exec postgres psql -U ecd_user -d ecd_db

redis-cli: ## Abre sessão redis-cli interativa dentro do container Redis
	@$(DOCKER_COMPOSE) exec redis redis-cli

generate-mock: ## Gera arquivos SPED sintéticos de teste via scripts/gerar_massa_sped.py
	@python3 scripts/gerar_massa_sped.py
