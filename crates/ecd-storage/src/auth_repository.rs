//! Repositório de autenticação de usuários (`usuarios`) e configurações (`configuracoes_sistema`).
//! Escopo exclusivo do Agente responsável pelo Servidor HTTP (`apps/server`).

use crate::StorageError;
use sqlx::postgres::PgRow;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::types::{JsonValue, Uuid};
use sqlx::{PgPool, Row};
use std::fmt;

/// Papéis de controle de acesso baseado em funções (RBAC).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UserRole {
    Admin,
    Analista,
    Visualizador,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "Admin",
            Self::Analista => "Analista",
            Self::Visualizador => "Visualizador",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "admin" | "administrador" => Some(Self::Admin),
            "analista" | "auditor" => Some(Self::Analista),
            "visualizador" | "leitor" | "viewer" => Some(Self::Visualizador),
            _ => None,
        }
    }

    pub fn nivel(&self) -> u8 {
        match self {
            Self::Admin => 3,
            Self::Analista => 2,
            Self::Visualizador => 1,
        }
    }

    /// Verifica se o papel atual possui permissão suficiente para o papel mínimo exigido.
    pub fn permite(&self, minimo: Self) -> bool {
        self.nivel() >= minimo.nivel()
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Entidade persistida na tabela `usuarios`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Usuario {
    pub id: Uuid,
    pub email: String,
    pub nome: String,
    pub senha_hash: String,
    pub role: String,
    pub ativo: bool,
    pub criado_em: DateTime<Utc>,
    pub ultimo_login: Option<DateTime<Utc>>,
}

impl Usuario {
    pub fn role_enum(&self) -> UserRole {
        UserRole::parse(&self.role).unwrap_or(UserRole::Visualizador)
    }
}

/// Dados para criação de um novo usuário (com `Uuid::now_v7()`).
#[derive(Clone, Debug)]
pub struct NovoUsuario {
    pub id: Uuid,
    pub email: String,
    pub nome: String,
    pub senha_hash: String,
    pub role: UserRole,
    pub ativo: bool,
}

impl NovoUsuario {
    pub fn new(
        email: impl Into<String>,
        nome: impl Into<String>,
        senha_hash: impl Into<String>,
        role: UserRole,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            email: email.into(),
            nome: nome.into(),
            senha_hash: senha_hash.into(),
            role,
            ativo: true,
        }
    }
}

/// Entidade persistida na tabela `configuracoes_sistema`.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfiguracaoSistema {
    pub chave: String,
    pub valor: JsonValue,
    pub descricao: Option<String>,
    pub atualizado_em: DateTime<Utc>,
}

fn map_usuario_row(row: PgRow) -> Result<Usuario, sqlx::Error> {
    Ok(Usuario {
        id: row.try_get("id")?,
        email: row.try_get("email")?,
        nome: row.try_get("nome")?,
        senha_hash: row.try_get("senha_hash")?,
        role: row.try_get("role")?,
        ativo: row.try_get("ativo")?,
        criado_em: row.try_get("criado_em")?,
        ultimo_login: row.try_get("ultimo_login")?,
    })
}

fn map_configuracao_row(row: PgRow) -> Result<ConfiguracaoSistema, sqlx::Error> {
    Ok(ConfiguracaoSistema {
        chave: row.try_get("chave")?,
        valor: row.try_get("valor")?,
        descricao: row.try_get("descricao")?,
        atualizado_em: row.try_get("atualizado_em")?,
    })
}

/// Repositório PostgreSQL para gerenciamento de usuários e configurações do sistema.
#[derive(Clone, Debug)]
pub struct AuthRepository {
    pool: PgPool,
}

impl AuthRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Busca um usuário pelo endereço de e-mail (case-insensitive).
    pub async fn buscar_por_email(&self, email: &str) -> Result<Option<Usuario>, StorageError> {
        let email_norm = email.trim().to_lowercase();
        let row = sqlx::query(
            r#"
            SELECT id, email, nome, senha_hash, role, ativo, criado_em, ultimo_login
            FROM usuarios
            WHERE LOWER(email) = $1
            LIMIT 1
            "#,
        )
        .bind(email_norm)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_usuario_row)
            .transpose()
            .map_err(StorageError::from)
    }

    /// Busca um usuário pelo seu identificador UUID.
    pub async fn buscar_por_id(&self, id: Uuid) -> Result<Option<Usuario>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT id, email, nome, senha_hash, role, ativo, criado_em, ultimo_login
            FROM usuarios
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_usuario_row)
            .transpose()
            .map_err(StorageError::from)
    }

    /// Cria um novo usuário no banco utilizando UUIDv7.
    pub async fn criar_usuario(&self, novo: &NovoUsuario) -> Result<Usuario, StorageError> {
        let email_norm = novo.email.trim().to_lowercase();
        let row = sqlx::query(
            r#"
            INSERT INTO usuarios (id, email, nome, senha_hash, role, ativo)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, email, nome, senha_hash, role, ativo, criado_em, ultimo_login
            "#,
        )
        .bind(novo.id)
        .bind(email_norm)
        .bind(novo.nome.trim())
        .bind(&novo.senha_hash)
        .bind(novo.role.as_str())
        .bind(novo.ativo)
        .fetch_one(&self.pool)
        .await?;

        map_usuario_row(row).map_err(StorageError::from)
    }

    /// Atualiza o carimbo `ultimo_login` do usuário autenticado.
    pub async fn registrar_login(&self, id: Uuid) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            UPDATE usuarios
            SET ultimo_login = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Atualiza o papel RBAC e o status de ativação de um usuário.
    pub async fn atualizar_role_e_status(
        &self,
        id: Uuid,
        role: UserRole,
        ativo: bool,
    ) -> Result<Option<Usuario>, StorageError> {
        let row = sqlx::query(
            r#"
            UPDATE usuarios
            SET role = $2, ativo = $3
            WHERE id = $1
            RETURNING id, email, nome, senha_hash, role, ativo, criado_em, ultimo_login
            "#,
        )
        .bind(id)
        .bind(role.as_str())
        .bind(ativo)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_usuario_row)
            .transpose()
            .map_err(StorageError::from)
    }

    /// Lista todos os usuários cadastrados ordenados por criação.
    pub async fn listar_usuarios(&self) -> Result<Vec<Usuario>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT id, email, nome, senha_hash, role, ativo, criado_em, ultimo_login
            FROM usuarios
            ORDER BY criado_em DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(map_usuario_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Obtém uma configuração do sistema pela chave.
    pub async fn obter_configuracao(
        &self,
        chave: &str,
    ) -> Result<Option<ConfiguracaoSistema>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT chave, valor, descricao, atualizado_em
            FROM configuracoes_sistema
            WHERE chave = $1
            LIMIT 1
            "#,
        )
        .bind(chave.trim())
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_configuracao_row)
            .transpose()
            .map_err(StorageError::from)
    }

    /// Insere ou atualiza (`UPSERT`) um parâmetro em `configuracoes_sistema`.
    pub async fn salvar_configuracao(
        &self,
        chave: &str,
        valor: &JsonValue,
        descricao: Option<&str>,
    ) -> Result<ConfiguracaoSistema, StorageError> {
        let row = sqlx::query(
            r#"
            INSERT INTO configuracoes_sistema (chave, valor, descricao, atualizado_em)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (chave) DO UPDATE
            SET valor = EXCLUDED.valor,
                descricao = COALESCE(EXCLUDED.descricao, configuracoes_sistema.descricao),
                atualizado_em = NOW()
            RETURNING chave, valor, descricao, atualizado_em
            "#,
        )
        .bind(chave.trim())
        .bind(valor)
        .bind(descricao)
        .fetch_one(&self.pool)
        .await?;

        map_configuracao_row(row).map_err(StorageError::from)
    }

    /// Lista todas as configurações de sistema armazenadas.
    pub async fn listar_configuracoes(&self) -> Result<Vec<ConfiguracaoSistema>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT chave, valor, descricao, atualizado_em
            FROM configuracoes_sistema
            ORDER BY chave ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(map_configuracao_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_role_hierarchy_and_parsing() {
        assert_eq!(UserRole::parse("Admin"), Some(UserRole::Admin));
        assert_eq!(UserRole::parse("administrador"), Some(UserRole::Admin));
        assert_eq!(UserRole::parse("Analista"), Some(UserRole::Analista));
        assert_eq!(
            UserRole::parse("Visualizador"),
            Some(UserRole::Visualizador)
        );
        assert_eq!(UserRole::parse("invalido"), None);

        assert!(UserRole::Admin.permite(UserRole::Admin));
        assert!(UserRole::Admin.permite(UserRole::Analista));
        assert!(UserRole::Admin.permite(UserRole::Visualizador));

        assert!(!UserRole::Analista.permite(UserRole::Admin));
        assert!(UserRole::Analista.permite(UserRole::Analista));
        assert!(UserRole::Analista.permite(UserRole::Visualizador));

        assert!(!UserRole::Visualizador.permite(UserRole::Admin));
        assert!(!UserRole::Visualizador.permite(UserRole::Analista));
        assert!(UserRole::Visualizador.permite(UserRole::Visualizador));
    }

    #[test]
    fn test_novo_usuario_gera_uuid_v7() {
        let u = NovoUsuario::new(
            "auditor@rfb.gov.br",
            "Auditor Fiscal",
            "$argon2id$v=19$m=19456,t=2,p=1$hash",
            UserRole::Analista,
        );
        assert_eq!(u.id.get_version_num(), 7);
        assert_eq!(u.role, UserRole::Analista);
        assert!(u.ativo);
    }
}
