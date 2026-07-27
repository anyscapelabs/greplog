use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use http::{Request, Response};
use tower::{Layer, Service};

use greplog_core::gen::{Span, SpanKind};

use crate::{generate_ulid, is_initialized, send_span, service_name};
use crate::redact::redact_attributes;

#[derive(Clone)]
pub struct GreplogAxumLayer;

impl<S> Layer<S> for GreplogAxumLayer
where
    S: Send + 'static,
{
    type Service = GreplogMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GreplogMiddleware { inner }
    }
}

#[derive(Clone)]
pub struct GreplogMiddleware<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for GreplogMiddleware<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send,
    ReqBody: Send + 'static,
    ResBody: Default + Send + 'static,
{
    type Response = Response<ResBody>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let start = Instant::now();
        let start_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i64;

        let method = req.method().to_string();
        let uri_path = req.uri().path().to_string();

        let mut headers = std::collections::HashMap::new();
        for (name, val) in req.headers() {
            if let Ok(v) = val.to_str() {
                headers.insert(name.to_string(), v.to_string());
            }
        }
        if let Some(host) = req.uri().host() {
            headers.insert("host".to_string(), host.to_string());
        }

        let mut svc = self.inner.clone();

        Box::pin(async move {
            let result = svc.call(req).await;
            let end_ns = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as i64;

            if is_initialized() {
                let status = match &result {
                    Ok(r) => r.status().as_u16(),
                    Err(_) => 500,
                };

                let span = Span {
                    service_name: service_name(),
                    name: format!("{} {}", method, uri_path),
                    start_time_ns: start_ns,
                    end_time_ns: end_ns,
                    route: uri_path.clone(),
                    method,
                    status_code: status as i32,
                    kind: SpanKind::Server as i32,
                    attributes: redact_attributes(&headers),
                    event_id: generate_ulid(),
                    ..Default::default()
                };

                send_span(span);
            }

            _ = start; // suppress unused
            result
        })
    }
}
