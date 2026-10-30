//! Axum extractors whose rejections are [`AppError`]s, so malformed requests
//! get the same JSON error body as everything else.

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::StatusCode;
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::AppError;

/// A JSON body.
#[derive(Debug)]
pub struct Body<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for Body<T> {
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, AppError> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            Err(e) => Err(json_error(&e)),
        }
    }
}

fn json_error(e: &JsonRejection) -> AppError {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        AppError::TooLarge(e.body_text())
    } else {
        AppError::Invalid(e.body_text())
    }
}

/// A path parameter.
#[derive(Debug)]
pub struct Path<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned + Send> FromRequestParts<S> for Path<T> {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, AppError> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Path(v)| Self(v))
            .map_err(|e: PathRejection| AppError::Invalid(e.body_text()))
    }
}

/// A query string.
#[derive(Debug)]
pub struct Query<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequestParts<S> for Query<T> {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, AppError> {
        axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Query(v)| Self(v))
            .map_err(|e: QueryRejection| AppError::Invalid(e.body_text()))
    }
}
