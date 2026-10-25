# Original User Request

## Initial Request — 2026-10-24T19:36:38Z

Reconstrução em Rust nativo de alta performance das rotinas de ETL e geração das bases SQLite dos Dados Abertos do CNPJ da Receita Federal (`cnpj.db` e `rede.db`), integradas a um pipeline completo e automatizadas via `make`.

Working directory: /home/pizani/Documents/dados_abertos/rede-cnpj-master-v26/rede_cria_tabelas
Integrity mode: development

## Requirements

### R1. Ferramenta CLI de Ingestão em Rust (`cnpj.db`)
Implementar um utilitário CLI em Rust de alto desempenho (`memmap2`, `rusqlite`, multithreading com `rayon`) capaz de ler os 37 arquivos de Dados Abertos do CNPJ da RFB (Empresas, Estabelecimentos, Sócios, Simples, CNAEs e tabelas de domínio) e inseri-los no banco SQLite `dados-publicos/cnpj.db`. O layout das tabelas (`empresa`, `estabelecimento`, `socios`, `simples`, `cnae`, etc.) e os índices (`cnpj_basico`, `cnpj`, `cpf_cnpj_socio`, `nome_socio`, `razao_social`) devem manter 100% de compatibilidade com o formato esperado pelo `rede-cnpj` e pelo `rust-sped-analytics`.

### R2. Geração da Base de Ligações Societárias (`rede.db`)
Implementar a rotina em Rust para criação da base `dados-publicos/rede.db` a partir do `cnpj.db`. Deve gerar a tabela `ligacao` contendo as arestas bidirecionais (empresa -> sócio e sócio -> empresa), com papéis, percentuais de participação societária e índices apropriados para consultas em grafos de alta velocidade, além de suporte a busca textual FTS.

### R3. Pipeline de Download e Descompactação
Implementar módulo de download assíncrono e validação dos 37 arquivos `.zip` diretamente do repositório oficial da Receita Federal (WebDAV ou HTTP), com suporte a resumo de download, barra de progresso, descompactação em streaming e limpeza de arquivos intermediários se solicitado.

### R4. Orquestração e Operação via Makefile
Criar um `Makefile` completo na raiz do projeto com alvos bem documentados e idempotentes:
- `make download`: Baixa os 37 arquivos compactados da RFB para `dados-publicos-zip/`.
- `make build-cnpj`: Executa a carga das tabelas e criação dos índices em `dados-publicos/cnpj.db`.
- `make build-rede`: Gera o banco de dados `dados-publicos/rede.db` e as tabelas de ligações.
- `make all`: Executa o pipeline completo (download -> build-cnpj -> build-rede).
- `make test`: Executa testes automatizados com dados sintéticos (fixtures) para validar schema e integridade.
- `make status`: Exibe o tamanho, contagem de registros e status das bases geradas.
- `make clean`: Limpa arquivos temporários descompactados.

## Acceptance Criteria

### Schema e Compatibilidade
- [ ] O banco `dados-publicos/cnpj.db` gerado contém todas as tabelas oficiais (`empresa`, `estabelecimento`, `socios`, `simples`, `cnae`, `motivo`, `municipio`, `natureza_juridica`, `pais`, `qualificacao_socio`).
- [ ] Todos os índices essenciais de busca por CNPJ, sócio e razão social estão presentes e operacionais.
- [ ] O banco `dados-publicos/rede.db` possui a tabela `ligacao` populada e consistente com os vínculos de sócios de `cnpj.db`.

### Performance e Uso de Memória
- [ ] A ingestão opera em lotes transacionais com PRAGMAs SQLite otimizados (`journal_mode = WAL`, `synchronous = OFF`, `cache_size`, `temp_store = MEMORY`).
- [ ] O processamento não esgota a memória RAM (limite seguro para máquinas com 8GB a 16GB de RAM através de processamento em chunks/streaming).

### Operação via Makefile
- [ ] `make all` executa a cadeia de forma autônoma sem exigir intervenção manual ou inputs interativos bloqueantes.
- [ ] O comando `make test` executa testes unitários/integração com arquivos de fixture reduzidos e valida a integridade referencial.
- [ ] `make help` exibe descrições claras de todos os alvos disponíveis.
