//! Handlers HTTP de autenticação (`/api/auth/login`, `/api/auth/me`, `/api/auth/logout`, `/api/auth/refresh`, `/api/auth/usuarios`).

use crate::{
    auth::{
        build_access_cookie, build_refresh_cookie, clear_access_cookie, clear_refresh_cookie,
        extract_cookie_value, gerar_access_token, gerar_refresh_token, validar_token,
        verify_password, AuthError, AuthUser, RequireAdmin, REFRESH_COOKIE_NAME,
    },
    AppState,
};
use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use ecd_storage::auth_repository::{UserRole, Usuario};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    #[serde(alias = "password")]
    pub senha: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsuarioResponse {
    pub id: Uuid,
    pub email: String,
    pub nome: String,
    pub role: String,
    pub ativo: bool,
}

impl From<&Usuario> for UsuarioResponse {
    fn from(u: &Usuario) -> Self {
        Self {
            id: u.id,
            email: u.email.clone(),
            nome: u.nome.clone(),
            role: u.role.clone(),
            ativo: u.ativo,
        }
    }
}

/// `POST /api/auth/login`
/// Valida credenciais com Argon2id e emite `access_token` e `refresh_token` em cookies `HttpOnly; Secure; SameSite=Lax`.
pub async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Response, AuthError> {
    let email_clean = payload.email.trim().to_lowercase();
    if email_clean.is_empty() || payload.senha.is_empty() {
        return Err(AuthError::InvalidCredentials);
    }

    // 1. Tenta localizar no PostgreSQL se o repositório estiver ativo
    let mut usuario_opt: Option<Usuario> = None;
    if let Some(ref repo) = state.auth_repo {
        if let Ok(found) = repo.buscar_por_email(&email_clean).await {
            usuario_opt = found;
        }
    }

    // 2. Fallback para store em memória (testes / bootstrap local)
    if usuario_opt.is_none() {
        let users = state.in_memory_users.read().await;
        usuario_opt = users
            .iter()
            .find(|u| u.email.eq_ignore_ascii_case(&email_clean))
            .cloned();
    }

    let usuario = usuario_opt.ok_or(AuthError::InvalidCredentials)?;

    if !usuario.ativo {
        return Err(AuthError::InactiveUser);
    }

    let senha_valida = verify_password(&payload.senha, &usuario.senha_hash)?;
    if !senha_valida {
        return Err(AuthError::InvalidCredentials);
    }

    if let Some(ref repo) = state.auth_repo {
        let _ = repo.registrar_login(usuario.id).await;
    }

    let role = usuario.role_enum();
    let access_token = gerar_access_token(
        usuario.id,
        &usuario.email,
        &usuario.nome,
        role,
        &state.jwt_secret,
    )?;
    let refresh_token = gerar_refresh_token(
        usuario.id,
        &usuario.email,
        &usuario.nome,
        role,
        &state.jwt_secret,
    )?;

    let access_cookie = build_access_cookie(&access_token);
    let refresh_cookie = build_refresh_cookie(&refresh_token);

    let body = Json(json!({
        "usuario": UsuarioResponse::from(&usuario),
        "id": usuario.id,
        "email": usuario.email,
        "nome": usuario.nome,
        "role": usuario.role,
        "ativo": usuario.ativo,
        "mensagem": "Autenticação realizada com sucesso",
    }));

    let mut response = (StatusCode::OK, body).into_response();
    let headers = response.headers_mut();
    if let Ok(val) = HeaderValue::from_str(&access_cookie) {
        headers.append(header::SET_COOKIE, val);
    }
    if let Ok(val) = HeaderValue::from_str(&refresh_cookie) {
        headers.append(header::SET_COOKIE, val);
    }

    Ok(response)
}

/// `GET /api/auth/me`
/// Retorna os dados do usuário autenticado via extrator `AuthUser`.
pub async fn me_handler(user: AuthUser) -> impl IntoResponse {
    Json(json!({
        "id": user.id,
        "email": user.email,
        "nome": user.nome,
        "role": user.role,
        "ativo": true,
    }))
}

/// `POST /api/auth/logout`
/// Limpa os cookies `access_token` e `refresh_token` (`Max-Age=0`).
pub async fn logout_handler() -> impl IntoResponse {
    let mut response = (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "mensagem": "Sessão encerrada com sucesso",
        })),
    )
        .into_response();

    let headers = response.headers_mut();
    if let Ok(val) = HeaderValue::from_str(&clear_access_cookie()) {
        headers.append(header::SET_COOKIE, val);
    }
    if let Ok(val) = HeaderValue::from_str(&clear_refresh_cookie()) {
        headers.append(header::SET_COOKIE, val);
    }

    response
}

/// `POST /api/auth/refresh`
/// Renova o `access_token` a partir do `refresh_token` armazenado em cookie `HttpOnly`.
pub async fn refresh_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AuthError> {
    let refresh_jwt = extract_cookie_value(&headers, REFRESH_COOKIE_NAME).ok_or_else(|| {
        AuthError::Unauthorized("Cookie 'refresh_token' não encontrado".to_string())
    })?;

    let claims = validar_token(&refresh_jwt, &state.jwt_secret)?;
    if claims.token_type != "refresh" {
        return Err(AuthError::Unauthorized(
            "Token informado não é um refresh_token válido".to_string(),
        ));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AuthError::Unauthorized("UUID inválido no refresh_token".to_string()))?;
    let role = UserRole::parse(&claims.role).unwrap_or(UserRole::Visualizador);

    let new_access = gerar_access_token(
        user_id,
        &claims.email,
        &claims.nome,
        role,
        &state.jwt_secret,
    )?;
    let access_cookie = build_access_cookie(&new_access);

    let mut response = (
        StatusCode::OK,
        Json(json!({
            "id": user_id,
            "email": claims.email,
            "nome": claims.nome,
            "role": role.as_str(),
            "mensagem": "Token de acesso renovado com sucesso",
        })),
    )
        .into_response();

    if let Ok(val) = HeaderValue::from_str(&access_cookie) {
        response.headers_mut().append(header::SET_COOKIE, val);
    }

    Ok(response)
}

/// `GET /api/auth/usuarios`
/// Rota protegida por RBAC (`RequireAdmin`) para listagem de usuários do sistema.
pub async fn listar_usuarios_handler(
    State(state): State<AppState>,
    RequireAdmin(_admin): RequireAdmin,
) -> impl IntoResponse {
    if let Some(ref repo) = state.auth_repo {
        if let Ok(users) = repo.listar_usuarios().await {
            if !users.is_empty() {
                let resp: Vec<UsuarioResponse> = users.iter().map(UsuarioResponse::from).collect();
                return (StatusCode::OK, Json(resp));
            }
        }
    }

    let users = state.in_memory_users.read().await;
    let resp: Vec<UsuarioResponse> = users.iter().map(UsuarioResponse::from).collect();
    (StatusCode::OK, Json(resp))
}
