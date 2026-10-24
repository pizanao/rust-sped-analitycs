#!/usr/bin/env bash
# ==============================================================================
# Rust Sped Analytics - Script de Empacotamento para Distribuição
# ==============================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

# 1. Determinar versão
VERSION="${VERSION:-$(grep -m1 '^version = ' Cargo.toml | cut -d '"' -f 2 || echo "0.1.0")}"
DIST_DIR="${ROOT_DIR}/dist"
PKG_NAME="rust-sped-analytics-${VERSION}"
STAGE_DIR="${DIST_DIR}/${PKG_NAME}"
ARCHIVE_FILE="${DIST_DIR}/${PKG_NAME}.tar.gz"

echo "===================================================================="
echo "📦 Iniciando empacotamento: ${PKG_NAME}"
echo "===================================================================="

# 2. Preparar diretórios limpos
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/bin"
mkdir -p "${STAGE_DIR}/web"
mkdir -p "${STAGE_DIR}/migrations"
mkdir -p "${STAGE_DIR}/scripts"
mkdir -p "${STAGE_DIR}/media/ecd_stage/aguardando"
mkdir -p "${STAGE_DIR}/media/ecd_stage/processando"
mkdir -p "${STAGE_DIR}/media/ecd_stage/processados"
mkdir -p "${STAGE_DIR}/media/ecd_stage/erro"

# 3. Compilar binários Rust caso não existam
SERVER_BIN="${ROOT_DIR}/target/release/ecd-server"
WORKER_BIN="${ROOT_DIR}/target/release/ecd-worker"

if [[ ! -f "${SERVER_BIN}" || ! -f "${WORKER_BIN}" || "${FORCE_BUILD:-0}" == "1" ]]; then
    echo "🔨 Compilando binários Rust em release (ecd-server, ecd-worker)..."
    cargo build --release -p ecd-server -p ecd-worker
fi

echo "📋 Copiando binários..."
cp -f "${SERVER_BIN}" "${STAGE_DIR}/bin/"
cp -f "${WORKER_BIN}" "${STAGE_DIR}/bin/"
chmod +x "${STAGE_DIR}/bin/"*

# 4. Empacotar Frontend Web se disponível
if [[ -d "${ROOT_DIR}/apps/web" ]]; then
    if [[ ! -d "${ROOT_DIR}/apps/web/dist" || "${FORCE_BUILD:-0}" == "1" ]]; then
        if command -v npm >/dev/null 2>&1; then
            echo "🌐 Compilando frontend SPA em apps/web..."
            if [[ ! -d "${ROOT_DIR}/apps/web/node_modules" ]]; then
                echo "📦 Instalando dependências de apps/web..."
                (cd "${ROOT_DIR}/apps/web" && npm install --prefer-offline --no-audit)
            fi
            (cd "${ROOT_DIR}/apps/web" && npm run build) || echo "⚠️  Falha ao compilar frontend SPA. Prosseguindo..."
        fi
    fi

    if [[ -d "${ROOT_DIR}/apps/web/dist" ]]; then
        echo "📋 Copiando artefatos estáticos do frontend..."
        cp -r "${ROOT_DIR}/apps/web/dist" "${STAGE_DIR}/web/"
    else
        echo "⚠️  Aviso: apps/web/dist não encontrado. Pulando frontend estático."
    fi
fi

# 5. Copiar AI Gateway
if [[ -d "${ROOT_DIR}/services/ai-gateway" ]]; then
    echo "📋 Copiando serviço AI Gateway..."
    mkdir -p "${STAGE_DIR}/services"
    cp -r "${ROOT_DIR}/services/ai-gateway" "${STAGE_DIR}/services/"
    rm -rf "${STAGE_DIR}/services/ai-gateway/__pycache__" "${STAGE_DIR}/services/ai-gateway/.pytest_cache"
fi

# 6. Copiar migrações, scripts e configurações
echo "📋 Copiando migrações SQL e scripts auxiliares..."
cp -r "${ROOT_DIR}/migrations/"* "${STAGE_DIR}/migrations/"
cp -f "${ROOT_DIR}/scripts/gerar_massa_sped.py" "${STAGE_DIR}/scripts/"
chmod +x "${STAGE_DIR}/scripts/"*.py

echo "📋 Copiando manifestos e configurações..."
cp -f "${ROOT_DIR}/docker-compose.yml" "${STAGE_DIR}/"
[[ -f "${ROOT_DIR}/Dockerfile.server" ]] && cp -f "${ROOT_DIR}/Dockerfile.server" "${STAGE_DIR}/"
[[ -f "${ROOT_DIR}/Dockerfile.worker" ]] && cp -f "${ROOT_DIR}/Dockerfile.worker" "${STAGE_DIR}/"
[[ -f "${ROOT_DIR}/.env.example" ]] && cp -f "${ROOT_DIR}/.env.example" "${STAGE_DIR}/"
[[ -f "${ROOT_DIR}/README.md" ]] && cp -f "${ROOT_DIR}/README.md" "${STAGE_DIR}/"
[[ -f "${ROOT_DIR}/LICENSE" ]] && cp -f "${ROOT_DIR}/LICENSE" "${STAGE_DIR}/"

# 7. Criar script de inicialização standalone no pacote
cat << 'EOF' > "${STAGE_DIR}/run.sh"
#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${DIR}"

if [[ ! -f .env ]]; then
    if [[ -f .env.example ]]; then
        echo "ℹ️  Criando .env a partir de .env.example..."
        cp .env.example .env
    fi
fi

echo "🚀 Iniciando Rust Sped Analytics (Docker Compose)..."
docker compose up -d
echo "✅ Serviços em execução!"
docker compose ps
EOF
chmod +x "${STAGE_DIR}/run.sh"

# 8. Compactar pacote tar.gz
echo "🗜️  Gerando arquivo compactado: ${ARCHIVE_FILE}..."
(cd "${DIST_DIR}" && tar -czf "${PKG_NAME}.tar.gz" "${PKG_NAME}")

# 9. Gerar checksum SHA256
if command -v sha256sum >/dev/null 2>&1; then
    (cd "${DIST_DIR}" && sha256sum "${PKG_NAME}.tar.gz" > "${PKG_NAME}.tar.gz.sha256")
    echo "🔒 Checksum gerado em ${DIST_DIR}/${PKG_NAME}.tar.gz.sha256"
fi

echo "===================================================================="
echo "🎉 Pacote gerado com sucesso!"
echo "📁 Diretório: ${STAGE_DIR}"
echo "📦 Arquivo:   ${ARCHIVE_FILE}"
echo "===================================================================="
