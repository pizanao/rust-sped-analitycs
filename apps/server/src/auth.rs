//! Autenticação segura com Argon2id, tokens JWT em cookies `HttpOnly; Secure; SameSite=Lax`,
//! extrator `AuthUser` e controle de acesso baseado em papéis (RBAC).

use crate::AppState;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use axum::{
    async_trait,
    extract::{FromRequestParts, Request, State},
    http::{header, request::Parts, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use ecd_storage::auth_repository::UserRole;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

/// Tempo de vida do `access_token` (15 minutos = 900 segundos).
pub const ACCESS_TOKEN_TTL_SECS: i64 = 15 * 60;

/// Tempo de vida do `refresh_token` (7 dias = 604.800 segundos).
pub const REFRESH_TOKEN_TTL_SECS: i64 = 7 * 24 * 60 * 60;

/// Nome do cookie para o token de acesso de curta duração.
pub const ACCESS_COOKIE_NAME: &str = "access_token";

/// Nome do cookie para o token de renovação de longa duração.
pub const REFRESH_COOKIE_NAME: &str = "refresh_token";

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Credenciais inválidas: e-mail ou senha incorretos")]
    InvalidCredentials,

    #[error("Não autenticado: token JWT ausente ou inválido")]
    Unauthorized(String),

    #[error("Acesso negado: papel '{atual}' insuficiente (requer '{requerido}')")]
    Forbidden { atual: String, requerido: String },

    #[error("Conta de usuário inativa ou bloqueada")]
    InactiveUser,

    #[error("Erro criptográfico Argon2id: {0}")]
    PasswordHash(String),

    #[error("Erro ao processar token JWT: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, codigo) = match &self {
            AuthError::InvalidCredentials | AuthError::Unauthorized(_) => {
                (StatusCode::UNAUTHORIZED, "NAO_AUTENTICADO")
            }
            AuthError::Forbidden { .. } | AuthError::InactiveUser => {
                (StatusCode::FORBIDDEN, "ACESSO_NEGADO")
            }
            AuthError::PasswordHash(_) | AuthError::Jwt(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "ERRO_AUTENTICACAO")
            }
        };

        (
            status,
            Json(json!({
                "erro": self.to_string(),
                "codigo": codigo,
            })),
        )
            .into_response()
    }
}

/// Claims armazenadas no payload assinado do JWT.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub nome: String,
    pub role: String,
    pub token_type: String,
    pub iat: usize,
    pub exp: usize,
}

/// Usuário autenticado extraído da requisição HTTP (`Cookie` ou `Authorization: Bearer`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
    pub nome: String,
    pub role: String,
}

impl AuthUser {
    pub fn role_enum(&self) -> UserRole {
        UserRole::parse(&self.role).unwrap_or(UserRole::Visualizador)
    }

    /// Verifica se o usuário autenticado possui pelo menos o papel `minimo`.
    pub fn require_role(&self, minimo: UserRole) -> Result<(), AuthError> {
        let atual = self.role_enum();
        if atual.permite(minimo) {
            Ok(())
        } else {
            Err(AuthError::Forbidden {
                atual: atual.as_str().to_string(),
                requerido: minimo.as_str().to_string(),
            })
        }
    }
}

/// Gera o hash de uma senha utilizando exclusivamente o algoritmo **Argon2id** com sal de 16 bytes.
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt_bytes = *Uuid::new_v4().as_bytes();
    let salt =
        SaltString::encode_b64(&salt_bytes).map_err(|e| AuthError::PasswordHash(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default());
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AuthError::PasswordHash(e.to_string()))?
        .to_string();

    Ok(password_hash)
}

/// Verifica uma senha em texto plano contra um hash **Argon2id** armazenado.
pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, AuthError> {
    let parsed_hash =
        PasswordHash::new(password_hash).map_err(|e| AuthError::PasswordHash(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default());
    Ok(argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Gera um `access_token` JWT assinado com validade de 15 minutos.
pub fn gerar_access_token(
    user_id: Uuid,
    email: &str,
    nome: &str,
    role: UserRole,
    secret: &str,
) -> Result<String, AuthError> {
    gerar_token_interno(
        user_id,
        email,
        nome,
        role,
        "access",
        ACCESS_TOKEN_TTL_SECS,
        secret,
    )
}

/// Gera um `refresh_token` JWT assinado com validade de 7 dias.
pub fn gerar_refresh_token(
    user_id: Uuid,
    email: &str,
    nome: &str,
    role: UserRole,
    secret: &str,
) -> Result<String, AuthError> {
    gerar_token_interno(
        user_id,
        email,
        nome,
        role,
        "refresh",
        REFRESH_TOKEN_TTL_SECS,
        secret,
    )
}

fn gerar_token_interno(
    user_id: Uuid,
    email: &str,
    nome: &str,
    role: UserRole,
    token_type: &str,
    ttl_secs: i64,
    secret: &str,
) -> Result<String, AuthError> {
    let now = Utc::now().timestamp();
    let exp = (now + ttl_secs).max(0) as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        nome: nome.to_string(),
        role: role.as_str().to_string(),
        token_type: token_type.to_string(),
        iat: now.max(0) as usize,
        exp,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

/// Decodifica e valida a assinatura e expiração de um token JWT.
pub fn validar_token(token: &str, secret: &str) -> Result<Claims, AuthError> {
    let mut validation = Validation::default();
    validation.validate_exp = true;

    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| AuthError::Unauthorized(format!("Token JWT inválido ou expirado: {e}")))?;

    Ok(data.claims)
}

/// Formata o cabeçalho `Set-Cookie` para o `access_token` com `HttpOnly; Secure; SameSite=Lax`.
pub fn build_access_cookie(token: &str) -> String {
    format!(
        "{ACCESS_COOKIE_NAME}={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={ACCESS_TOKEN_TTL_SECS}"
    )
}

/// Formata o cabeçalho `Set-Cookie` para o `refresh_token` com `HttpOnly; Secure; SameSite=Lax`.
pub fn build_refresh_cookie(token: &str) -> String {
    format!(
        "{REFRESH_COOKIE_NAME}={token}; HttpOnly; Secure; SameSite=Lax; Path=/api/auth; Max-Age={REFRESH_TOKEN_TTL_SECS}"
    )
}

/// Formata o cabeçalho `Set-Cookie` para invalidar o `access_token` no logout.
pub fn clear_access_cookie() -> String {
    format!("{ACCESS_COOKIE_NAME}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0")
}

/// Formata o cabeçalho `Set-Cookie` para invalidar o `refresh_token` no logout.
pub fn clear_refresh_cookie() -> String {
    format!("{REFRESH_COOKIE_NAME}=; HttpOnly; Secure; SameSite=Lax; Path=/api/auth; Max-Age=0")
}

/// Extrai o valor de um cookie específico a partir do `HeaderMap`.
pub fn extract_cookie_value(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    for cookie_header in headers.get_all(header::COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for part in cookie_str.split(';') {
                let trimmed = part.trim();
                if let Some((k, v)) = trimmed.split_once('=') {
                    if k.trim() == cookie_name {
                        let val = v.trim();
                        if !val.is_empty() {
                            return Some(val.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Extrai um token Bearer do cabeçalho `Authorization`.
pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth_val = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = auth_val
        .strip_prefix("Bearer ")
        .or_else(|| auth_val.strip_prefix("bearer "))?
        .trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

/// Extrai e valida o [`AuthUser`] a partir dos cabeçalhos da requisição e do [`AppState`].
pub fn authenticate_from_headers(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<AuthUser, AuthError> {
    // 1. Prioriza cookie HttpOnly `access_token`
    if let Some(token) = extract_cookie_value(headers, ACCESS_COOKIE_NAME) {
        let claims = validar_token(&token, &state.jwt_secret)?;
        if claims.token_type != "access" {
            return Err(AuthError::Unauthorized(
                "Tipo de token inválido para acesso à API".to_string(),
            ));
        }
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
            AuthError::Unauthorized("UUID de usuário inválido no token".to_string())
        })?;
        return Ok(AuthUser {
            id: user_id,
            email: claims.email,
            nome: claims.nome,
            role: claims.role,
        });
    }

    // 2. Fallback para Authorization: Bearer (suporta JWT ou chave interna do AI Gateway)
    if let Some(bearer) = extract_bearer_token(headers) {
        if let Some(ref api_key) = state.internal_api_key {
            if !api_key.is_empty() && bearer == *api_key {
                return Ok(AuthUser {
                    id: Uuid::nil(),
                    email: "ai-gateway@ecd.internal".to_string(),
                    nome: "AI Gateway Service".to_string(),
                    role: UserRole::Analista.as_str().to_string(),
                });
            }
        }

        let claims = validar_token(&bearer, &state.jwt_secret)?;
        if claims.token_type != "access" {
            return Err(AuthError::Unauthorized(
                "Tipo de token inválido para acesso à API".to_string(),
            ));
        }
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
            AuthError::Unauthorized("UUID de usuário inválido no token".to_string())
        })?;
        return Ok(AuthUser {
            id: user_id,
            email: claims.email,
            nome: claims.nome,
            role: claims.role,
        });
    }

    Err(AuthError::Unauthorized(
        "Cookie HttpOnly 'access_token' ausente".to_string(),
    ))
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(existing) = parts.extensions.get::<AuthUser>() {
            return Ok(existing.clone());
        }
        let user = authenticate_from_headers(&parts.headers, state)?;
        parts.extensions.insert(user.clone());
        Ok(user)
    }
}

/// Extrator RBAC para rotas restritas a `Admin`.
#[derive(Clone, Debug)]
pub struct RequireAdmin(pub AuthUser);

#[async_trait]
impl FromRequestParts<AppState> for RequireAdmin {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        user.require_role(UserRole::Admin)?;
        Ok(Self(user))
    }
}

/// Extrator RBAC para rotas restritas a `Analista` ou `Admin`.
#[derive(Clone, Debug)]
pub struct RequireAnalista(pub AuthUser);

#[async_trait]
impl FromRequestParts<AppState> for RequireAnalista {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        user.require_role(UserRole::Analista)?;
        Ok(Self(user))
    }
}

/// Middleware Axum para impor controle de acesso baseado em papel mínimo (RBAC).
pub async fn rbac_middleware(
    State(state): State<AppState>,
    minimo: UserRole,
    mut req: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let user = authenticate_from_headers(req.headers(), &state)?;
    user.require_role(minimo)?;
    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argon2id_hash_and_verify() {
        let senha = "SenhaForte@Auditor2026!";
        let hash = hash_password(senha).expect("deve gerar hash Argon2id");
        assert!(hash.starts_with("$argon2id$"));

        assert!(verify_password(senha, &hash).expect("verificação deve funcionar"));
        assert!(!verify_password("SenhaErrada", &hash).expect("verificação deve rejeitar"));
    }

    #[test]
    fn test_jwt_access_and_refresh_tokens() {
        let secret = "segredo_super_seguro_teste_jwt_2026";
        let uid = Uuid::now_v7();
        let access = gerar_access_token(
            uid,
            "auditor@rfb.gov.br",
            "Auditor Fiscal",
            UserRole::Analista,
            secret,
        )
        .expect("deve gerar access token");

        let claims = validar_token(&access, secret).expect("deve validar access token");
        assert_eq!(claims.sub, uid.to_string());
        assert_eq!(claims.email, "auditor@rfb.gov.br");
        assert_eq!(claims.role, "Analista");
        assert_eq!(claims.token_type, "access");

        let refresh = gerar_refresh_token(
            uid,
            "auditor@rfb.gov.br",
            "Auditor Fiscal",
            UserRole::Analista,
            secret,
        )
        .expect("deve gerar refresh token");
        let refresh_claims = validar_token(&refresh, secret).expect("deve validar refresh token");
        assert_eq!(refresh_claims.token_type, "refresh");

        assert!(validar_token(&access, "outro_segredo").is_err());
    }

    #[test]
    fn test_cookie_headers_format() {
        let cookie = build_access_cookie("meu_jwt");
        assert!(cookie.contains("access_token=meu_jwt"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("SameSite=Lax"));

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "theme=dark; access_token=jwt_extraido_123; refresh_token=ref_456"
                .parse()
                .unwrap(),
        );
        assert_eq!(
            extract_cookie_value(&headers, ACCESS_COOKIE_NAME),
            Some("jwt_extraido_123".to_string())
        );
        assert_eq!(
            extract_cookie_value(&headers, REFRESH_COOKIE_NAME),
            Some("ref_456".to_string())
        );
    }
}
