//! The public side of invites: `https://<site>/join/<code>` links that chat apps show as links.
//! With the app installed, iOS Universal Links and Android App Links open it directly (they check
//! the two `.well-known` files below); without it, a small page shows the code and how to get
//! the app. Nothing about the group is shown: the code alone is the invitation.

use axum::Json;
use axum::extract::State;
use axum::http::header::ACCEPT_LANGUAGE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use serde_json::{Value, json};

use crate::extract::Path;
use crate::{AppError, AppState};

/// Where invite links point and which apps may open them.
#[derive(Debug, Clone, Default)]
pub struct Site {
    /// e.g. `https://example.com`; without it invites carry no link.
    pub url: Option<String>,
    /// iOS apps as `TEAMID.bundle.id`.
    pub ios_apps: Vec<String>,
    /// Android apps as (package, SHA-256 of the signing certificate, `AA:BB:…`).
    pub android_apps: Vec<(String, String)>,
}

impl Site {
    /// From `DIBILL_SITE_URL`, `DIBILL_IOS_APP_IDS` (comma-separated `TEAMID.bundle.id`) and
    /// `DIBILL_ANDROID_APPS` (comma-separated `package=SHA256`).
    #[must_use]
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let list = |name: &str| -> Vec<String> {
            var(name)
                .map(|v| {
                    v.split(',')
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            url: var("DIBILL_SITE_URL").map(|u| u.trim().trim_end_matches('/').to_owned()),
            ios_apps: list("DIBILL_IOS_APP_IDS"),
            android_apps: list("DIBILL_ANDROID_APPS")
                .into_iter()
                .filter_map(|a| {
                    a.split_once('=')
                        .map(|(p, s)| (p.trim().to_owned(), s.trim().to_uppercase()))
                })
                .collect(),
        }
    }

    /// The shareable link for an invite code, if the site is configured.
    #[must_use]
    pub fn invite_link(&self, code: &str) -> Option<String> {
        self.url.as_ref().map(|u| format!("{u}/join/{code}"))
    }
}

/// `GET /.well-known/apple-app-site-association`
pub async fn apple_app_site_association(State(s): State<AppState>) -> Json<Value> {
    Json(json!({
        "applinks": {
            "details": [{
                "appIDs": s.site.ios_apps,
                "components": [{ "/": "/join/*", "comment": "DiBillSion invites" }]
            }]
        }
    }))
}

/// `GET /.well-known/assetlinks.json`
pub async fn asset_links(State(s): State<AppState>) -> Json<Value> {
    Json(Value::Array(
        s.site
            .android_apps
            .iter()
            .map(|(package, sha256)| {
                json!({
                    "relation": ["delegate_permission/common.handle_all_urls"],
                    "target": {
                        "namespace": "android_app",
                        "package_name": package,
                        "sha256_cert_fingerprints": [sha256]
                    }
                })
            })
            .collect(),
    ))
}

/// `GET /join/{code}`: shown only when the app isn't installed (or the link opened in a browser).
pub async fn join_page(headers: HeaderMap, Path(code): Path<String>) -> Result<Response, AppError> {
    if code.is_empty()
        || code.len() > 64
        || !code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::NotFound("no such invite"));
    }
    let spanish = headers
        .get(ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim_start().to_ascii_lowercase().starts_with("es"));
    let t = if spanish { &ES } else { &EN };
    // The code is letters, digits, '-' and '_' only (checked above), so it needs no escaping.
    let page = format!(
        r#"<!doctype html>
<html lang="{lang}"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>{title}</title>
<style>
:root {{ color-scheme: light dark; --teal: #1F6F5C; }}
body {{ font: 17px/1.5 system-ui, -apple-system, sans-serif; max-width: 30rem; margin: 0 auto; padding: 2.5rem 1.25rem; }}
h1 {{ font-size: 1.5rem; line-height: 1.25; }}
a.button {{ display: block; text-align: center; background: var(--teal); color: #fff; text-decoration: none;
  padding: .9rem; border-radius: .9rem; font-weight: 600; margin: 1.5rem 0; }}
.code {{ font: 600 1.1rem ui-monospace, monospace; padding: .8rem; border-radius: .6rem; background: rgba(127,127,127,.15);
  word-break: break-all; user-select: all; }}
p.small {{ font-size: .9rem; opacity: .75; }}
</style></head>
<body>
<h1>{title}</h1>
<p>{intro}</p>
<a class="button" href="dibillsion://join/{code}">{open}</a>
<p>{no_app}</p>
<p>{code_hint}</p>
<div class="code">{code}</div>
<p class="small">{privacy}</p>
</body></html>"#,
        lang = t.lang,
        title = t.title,
        intro = t.intro,
        open = t.open,
        no_app = t.no_app,
        code_hint = t.code_hint,
        privacy = t.privacy,
    );
    Ok((StatusCode::OK, [("cache-control", "no-store")], Html(page)).into_response())
}

struct Texts {
    lang: &'static str,
    title: &'static str,
    intro: &'static str,
    open: &'static str,
    no_app: &'static str,
    code_hint: &'static str,
    privacy: &'static str,
}

const EN: Texts = Texts {
    lang: "en",
    title: "You're invited to a DiBillSion group",
    intro: "DiBillSion splits shared expenses exactly to the cent.",
    open: "Open in DiBillSion",
    no_app: "Don't have the app yet? Ask the person who invited you how to install it (TestFlight on iPhone, an APK on Android), then open this link again.",
    code_hint: "Or open the app, tap Join and paste this code:",
    privacy: "This page doesn't show anything about the group. Only people with the code can join.",
};

const ES: Texts = Texts {
    lang: "es",
    title: "Te han invitado a un grupo de DiBillSion",
    intro: "DiBillSion reparte gastos compartidos exactos al céntimo.",
    open: "Abrir en DiBillSion",
    no_app: "¿Aún no tienes la app? Pide a quien te invitó cómo instalarla (TestFlight en iPhone, un APK en Android) y vuelve a abrir este enlace.",
    code_hint: "O abre la app, toca Unirse y pega este código:",
    privacy: "Esta página no muestra nada del grupo. Solo quien tenga el código puede unirse.",
};
