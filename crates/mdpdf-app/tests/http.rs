#![cfg(feature = "ssr")]
use leptos::prelude::LeptosOptions;
use mdpdf_app::server::{AppState, assets::AssetStore, router};
use mdpdf_core::Theme;
use mdpdf_render::{BrowserPool, PoolConfig};
use serde_json::{Value, json};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::{Mutex, Semaphore};

#[tokio::test]
#[ignore = "requires Chromium; exercises real HTTP preview/export and session-scoped uploads"]
async fn preview_export_assets_and_sessions() {
    let storage = tempfile::tempdir().unwrap();
    let pool = BrowserPool::new(PoolConfig {
        size: 1,
        ..Default::default()
    })
    .await
    .unwrap();
    let assets = Arc::new(AssetStore::new(storage.path().into()).unwrap());
    let state = AppState {
        pool: pool.clone(),
        assets,
        max_markdown: 2 * 1024 * 1024,
        rates: Arc::new(Mutex::new(HashMap::new())),
        preview_slots: Arc::new(Semaphore::new(12)),
    };
    let options = LeptosOptions::builder().output_name("mdpdf-app").build();
    let app = router(state, options, "target/site".into());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let root = format!("http://{address}");
    let client = reqwest::Client::new();
    let home = client.get(&root).send().await.unwrap();
    assert!(home.status().is_success());
    let cookie = home
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    assert!(home.text().await.unwrap().contains("Document settings"));
    let body = json!({"markdown":"# Web export\n\n```mermaid\nflowchart LR\n A-->B\n```","theme":Theme::default()});
    let preview: Value = client
        .post(format!("{root}/api/rpc/render_preview"))
        .header("Cookie", &cookie)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        preview["html"].as_str().unwrap().contains("Web export"),
        "{preview}"
    );
    assert!(preview["diagnostics"].as_array().unwrap().is_empty());
    let update: Value = client
        .post(format!("{root}/api/rpc/render_preview_update"))
        .header("Cookie", &cookie)
        .json(&json!({"markdown":body["markdown"], "theme":Theme::default(), "include_math":false}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        update["body"].as_str().unwrap(),
        preview["html"]
            .as_str()
            .unwrap()
            .split_once("<main>")
            .unwrap()
            .1
            .split_once("</main>")
            .unwrap()
            .0
    );
    assert!(
        update.get("html").is_none(),
        "Warm previews must not resend vendor scripts"
    );
    assert!(
        update["css"]
            .as_str()
            .unwrap()
            .contains("data:font/woff2;base64,")
    );
    assert!(update["mathScript"].is_null());
    let exported = client
        .post(format!("{root}/api/export"))
        .header("Cookie", &cookie)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(exported.headers()["content-type"], "application/pdf");
    let bytes = exported.bytes().await.unwrap();
    assert_eq!(
        lopdf::Document::load_mem(&bytes).unwrap().get_pages().len(),
        1
    );
    let strict = json!({"markdown":"```mermaid\nflowchart TD\n A[Unclosed\n```","theme":Theme::default(),"strict_diagrams":true});
    let response = client
        .post(format!("{root}/api/export"))
        .header("Cookie", &cookie)
        .json(&strict)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422);
    assert_eq!(
        response.json::<Value>().await.unwrap()["diagnostics"][0]["kind"],
        "Mermaid"
    );
    let png = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../e2e/fixtures/pixel.png"),
    )
    .unwrap();
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(png.clone()).file_name("pixel.png"),
    );
    let response = client
        .post(format!("{root}/api/assets"))
        .header("Cookie", &cookie)
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let image = response.json::<Value>().await.unwrap();
    let id = image["id"].as_str().unwrap();
    assert_eq!(
        client
            .get(format!("{root}/assets/{id}"))
            .header("Cookie", &cookie)
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap()
            .as_ref(),
        png
    );
    assert_eq!(
        client
            .get(format!("{root}/assets/{id}"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let document = json!({"markdown":format!("# Image\n\n![Uploaded](asset://{id}){{width=50%}}"),"theme":Theme::default()});
    let preview: Value = client
        .post(format!("{root}/api/preview"))
        .header("Cookie", &cookie)
        .json(&document)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        preview["html"]
            .as_str()
            .unwrap()
            .contains("src=\"data:image/png;base64,")
    );
    let response = client
        .post(format!("{root}/api/export"))
        .header("Cookie", &cookie)
        .header("Origin", "https://unrelated.example")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    let font = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/fonts/merriweather/merriweather-latin-wght-normal.woff2"),
    )
    .unwrap();
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(font).file_name("Custom.woff2"),
    );
    let asset: Value = client
        .post(format!("{root}/api/assets"))
        .header("Cookie", &cookie)
        .multipart(form)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let family = format!("assetfont-{}", asset["id"].as_str().unwrap());
    let mut theme = Theme::default();
    theme.typography.body_font = family.clone();
    theme.typography.heading_font = family.clone();
    theme.diagrams.font_family = Some(family);
    let doc = json!({"markdown":"# Custom font\n\n```mermaid\nflowchart LR\n A[Custom]-->B[Font]\n```","theme":theme});
    let preview: Value = client
        .post(format!("{root}/api/preview"))
        .header("Cookie", &cookie)
        .json(&doc)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        preview["html"]
            .as_str()
            .unwrap()
            .contains("mdpdf-custom-fonts")
    );
    let response = client
        .post(format!("{root}/api/export"))
        .header("Cookie", &cookie)
        .json(&doc)
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let pdf = lopdf::Document::load_mem(&response.bytes().await.unwrap()).unwrap();
    assert!(
        pdf.objects.values().any(|object| object
            .as_dict()
            .ok()
            .and_then(|dictionary| dictionary
                .get(b"FontName")
                .or_else(|_| dictionary.get(b"BaseFont"))
                .ok())
            .and_then(|name| name.as_name().ok())
            .is_some_and(|name| String::from_utf8_lossy(name).contains("Merriweather"))),
        "Uploaded font was not embedded"
    );
    let bad =
        json!({"markdown":"![private](http://127.0.0.1:1/internal)","theme":Theme::default()});
    let response: Value = client
        .post(format!("{root}/api/preview"))
        .header("Cookie", &cookie)
        .json(&bad)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        response["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"].as_str().unwrap().contains("private"))
    );
    let busy = json!({"markdown": (0..20).map(|_|"```mermaid\nflowchart LR\n A-->B\n```\n\n").collect::<String>(), "theme":Theme::default()});
    let mut pending = Vec::new();
    for _ in 0..2 {
        let client = client.clone();
        let url = format!("{root}/api/export");
        let cookie = cookie.clone();
        let busy = busy.clone();
        pending.push(tokio::spawn(async move {
            client
                .post(url)
                .header("Cookie", cookie)
                .json(&busy)
                .send()
                .await
                .unwrap()
        }));
    }
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let limited = client
        .post(format!("{root}/api/export"))
        .header("Cookie", &cookie)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(limited.status(), 429);
    for task in pending {
        assert!(task.await.unwrap().status().is_success());
    }
    pool.close().await;
    server.abort();
}
