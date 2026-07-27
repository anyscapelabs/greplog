mod common;

use std::thread;
use std::time::Duration;

#[test]
fn test_axum_middleware_capture() {
    // Test 1: middleware captures via transport
    let (port, rx) = common::start_test_server();

    greplog::init_with_opts(None, None, Some(port));
    thread::sleep(Duration::from_millis(200));

    // Use tower::Service + ServiceBuilder to exercise the middleware directly
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
use std::convert::Infallible;
use tower::Service;
use tower::ServiceBuilder;
use http::{Request, Response};

        let mut svc = ServiceBuilder::new()
            .layer(greplog::axum_layer())
            .service(tower::service_fn(|req: Request<String>| async move {
                let _ = req;
                Ok::<_, Infallible>(Response::new("hello".to_string()))
            }));

        let req = Request::builder()
            .method("GET")
            .uri("/test/42")
            .header("host", "localhost")
            .body(String::new())
            .unwrap();

        let resp = svc.call(req).await.unwrap();
        assert_eq!(resp.status(), 200);
    });

    thread::sleep(Duration::from_millis(500));
    let batches = common::wait_for_frames(&rx, 1);

    assert!(!batches.is_empty(), "expected at least one batch from axum middleware");
    let batch = &batches[0];
    assert!(!batch.spans.is_empty(), "expected at least one span");
    let span = &batch.spans[0];

    assert!(span.name.contains("GET"), "span name should contain method");
    assert_eq!(span.status_code, 200);
    assert!(span.start_time_ns > 0);
    assert!(span.end_time_ns > 0);
}
