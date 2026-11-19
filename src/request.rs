//! HTTP request reading abstractions.
//!
//! [`Request`] provides access to the HTTP method, path, headers, route
//! parameters, query string, and request body.

use crate::session::{Cookie, CookieError, Session, SessionManager};
use crate::upload::UploadedFile;
use bytes::Bytes;
use hyper::{
    header::{COOKIE, HeaderValue},
    http::Method,
};
use std::sync::{Arc, Mutex};

/// Represents an HTTP request received by the server.
///
/// This structure encapsulates the original Hyper request and adds
/// useful information for the web application, namely route parameters
/// extracted by the router. Its purpose is to provide a simple and idiomatic
/// API for registered route handlers.
pub struct Request {
    inner: hyper::Request<Bytes>,
    params: Vec<(String, String)>,
    session: Option<Arc<Mutex<Session>>>,
    session_manager: Option<Arc<SessionManager>>,
    multipart_fields: Vec<(String, String)>,
    is_multipart: bool,
    uploaded_files: Vec<UploadedFile>,
}

impl Request {
    /// Creates a new `Request` instance with the original HTTP request and
    /// the already resolved route parameters.
    pub(crate) fn new(inner: hyper::Request<Bytes>, params: Vec<(String, String)>) -> Self {
        Self {
            inner,
            params,
            session: None,
            session_manager: None,
            multipart_fields: Vec::new(),
            is_multipart: false,
            uploaded_files: Vec::new(),
        }
    }

    /// Returns the session associated with the request.
    ///
    /// Returns `None` when session management is not enabled.
    pub fn session(&self) -> Option<Arc<Mutex<Session>>> {
        self.session.clone()
    }

    /// Associates a session with the HTTP request.
    pub(crate) fn set_session(&mut self, session: Arc<Mutex<Session>>) {
        self.session = Some(session);
    }

    /// Associates the session manager with the HTTP request.
    pub(crate) fn set_session_manager(&mut self, manager: Arc<SessionManager>) {
        self.session_manager = Some(manager);
    }

    /// Regenerates the current session identifier.
    ///
    /// Preserves existing data but replaces the identifier
    /// with a new UUID.
    ///
    /// Returns `false` if sessions are not enabled
    /// or if the session cannot be locked.
    pub fn regenerate_session(&self) -> Result<bool, crate::session::SessionError> {
        let (Some(session), Some(manager)) = (&self.session, &self.session_manager) else {
            return Ok(false);
        };

        let mut session = session
            .lock()
            .map_err(|_| crate::session::SessionError::LockPoisoned)?;

        manager.regenerate(&mut session)?;

        Ok(true)
    }

    /// Returns the HTTP method used in the request, such as `GET`, `POST`,
    /// `PUT`, `PATCH`, or `DELETE`.
    pub fn method(&self) -> &Method {
        self.inner.method()
    }

    /// Returns the request URL path without the query string.
    ///
    /// For example, for `/utilizadores/42?activo=true`, this method returns
    /// `/utilizadores/42`.
    pub fn path(&self) -> &str {
        self.inner.uri().path()
    }

    /// Looks up a specific HTTP header and returns its value, if present.
    pub fn header(&self, name: &str) -> Option<&HeaderValue> {
        self.inner.headers().get(name)
    }

    /// Returns all cookies received in the request.
    ///
    /// Supports multiple Cookie headers.
    /// Returns an error if any header cannot be interpreted as text.
    pub fn cookies(&self) -> Result<Vec<Cookie>, CookieError> {
        let mut cookies = Vec::new();

        for header in self.inner.headers().get_all(COOKIE).iter() {
            let value = header.to_str().map_err(|_| CookieError::InvalidHeader)?;

            cookies.extend(Cookie::parse(value));
        }

        Ok(cookies)
    }

    /// Looks up a cookie by name.
    ///
    /// Returns Ok(None) when the cookie does not exist.
    /// Returns an error when the cookie is invalid, duplicated,
    /// or when a header cannot be interpreted.
    pub fn cookie(&self, name: &str) -> Result<Option<String>, CookieError> {
        let headers = self
            .inner
            .headers()
            .get_all(COOKIE)
            .iter()
            .map(|header| header.to_str().map_err(|_| CookieError::InvalidHeader))
            .collect::<Result<Vec<_>, _>>()?;

        Cookie::get_unique_from_headers(headers, name)
    }

    /// Returns the value of a route parameter.
    ///
    /// Supports routes such as `/utilizadores/:id`, allowing the value of a
    /// dynamic segment to be retrieved using its defined name, for example
    /// `param("id")`.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Returns the value of a parameter in the URL query string.
    ///
    /// Example: `/produtos?pagina=2&limite=10` can be accessed using
    /// `query("pagina")`.
    pub fn query(&self, name: &str) -> Option<&str> {
        let query = self.inner.uri().query()?;

        query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;

            if key == name { Some(value) } else { None }
        })
    }

    /// Creates a copy of the request metadata for error handling.
    pub fn context(&self) -> RequestContext {
        RequestContext {
            method: self.inner.method().clone(),
            path: self.inner.uri().path().to_owned(),
            params: self.params.clone(),
            query: self.inner.uri().query().map(str::to_owned),
            headers: self
                .inner
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (name.as_str().to_owned(), value.to_owned()))
                })
                .collect(),
        }
    }

    /// Returns the HTTP request body as bytes.
    ///
    /// The content is returned by reference without copying the data.
    pub fn body(&self) -> &Bytes {
        self.inner.body()
    }

    /// Returns the HTTP request body as UTF-8 text.
    ///
    /// Returns an error when the body contains a byte sequence
    /// that does not represent valid UTF-8 text.
    pub fn text(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.body())
    }

    /// Deserializes the HTTP request body from JSON into a Rust type.
    ///
    /// The target type must implement `DeserializeOwned`.
    /// Returns an error if the content is not valid JSON
    /// or does not match the expected structure.
    pub fn json<T>(&self) -> Result<T, serde_json::Error>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_slice(self.body())
    }

    /// Converts the HTTP request body into a list of form fields.
    ///
    /// Automatically decodes percent-encoded characters and spaces.
    /// Preserves the original order and fields with duplicate names.
    ///
    /// Example: `nome=Fabricio&interesses=Rust&interesses=PHP`.
    pub fn form(&self) -> Vec<(String, String)> {
        if self.is_multipart {
            return self.multipart_fields.clone();
        }

        form_urlencoded::parse(self.body()).into_owned().collect()
    }

    /// Returns all files received in the request.
    pub fn files(&self) -> &[UploadedFile] {
        &self.uploaded_files
    }

    /// Looks up the first file associated with a form field name.
    pub fn file(&self, field_name: &str) -> Option<&UploadedFile> {
        self.uploaded_files
            .iter()
            .find(|file| file.field_name() == field_name)
    }

    /// Associates the processed multipart data with the request.
    pub(crate) fn set_multipart_data(
        &mut self,
        fields: Vec<(String, String)>,
        files: Vec<UploadedFile>,
    ) {
        self.multipart_fields = fields;
        self.uploaded_files = files;
        self.is_multipart = true;
    }

    pub(crate) fn set_body(&mut self, body: Bytes) {
        *self.inner.body_mut() = body;
    }

    /// Removes stored files when the request is rejected.
    pub(crate) async fn cleanup_uploaded_files(&mut self) {
        for file in self.uploaded_files.drain(..) {
            let _ = tokio::fs::remove_file(file.path()).await;
        }
    }
}

/// Lightweight request context available during error handling.
#[derive(Debug, Clone)]
pub struct RequestContext {
    method: hyper::http::Method,
    path: String,
    params: Vec<(String, String)>,
    query: Option<String>,
    headers: Vec<(String, String)>,
}

impl RequestContext {
    /// Returns the HTTP request method.
    pub fn method(&self) -> &hyper::http::Method {
        &self.method
    }

    /// Returns the request path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the value of a route parameter.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Returns a query string parameter.
    pub fn query(&self, name: &str) -> Option<&str> {
        self.query.as_deref()?.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == name).then_some(value)
        })
    }

    /// Returns the value of an HTTP header.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    #[cfg(test)]
    pub(crate) fn test() -> Self {
        Self {
            method: hyper::http::Method::GET,
            path: "/teste".to_string(),
            params: Vec::new(),
            query: None,
            headers: Vec::new(),
        }
    }
}
