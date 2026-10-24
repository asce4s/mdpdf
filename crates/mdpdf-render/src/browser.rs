use crate::{
    AssetContext, ExportOptions, RenderError, RenderedPdf, Result, pdf,
    prepare_document_with_assets,
};
use chromiumoxide::{
    Browser, BrowserConfig, Page,
    cdp::browser_protocol::{
        fetch::{ContinueRequestParams, EventRequestPaused, FailRequestParams},
        network::ErrorReason,
        page::PrintToPdfParams,
    },
};
use futures_util::StreamExt;
use mdpdf_core::{DiagKind, Diagnostic, PrintDocument, Severity, Theme};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, mpsc, oneshot},
    task::JoinHandle,
    time::{Instant, sleep, timeout, timeout_at},
};

#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub size: usize,
    pub chrome_path: Option<PathBuf>,
    pub export_timeout: Duration,
    /// Only enable inside an isolated container that requires it.
    pub container_no_sandbox: bool,
}
impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            size: 4,
            chrome_path: std::env::var_os("CHROME_PATH").map(PathBuf::from),
            export_timeout: Duration::from_secs(30),
            container_no_sandbox: false,
        }
    }
}
impl PoolConfig {
    pub fn from_env() -> Result<Self> {
        let mut config = Self::default();
        if let Ok(size) = std::env::var("MDPDF_POOL_SIZE") {
            config.size = size.parse().map_err(|_| {
                RenderError::Configuration("MDPDF_POOL_SIZE must be an integer".into())
            })?;
        }
        if std::env::var("MDPDF_CONTAINER_NO_SANDBOX").as_deref() == Ok("1") {
            if !std::path::Path::new("/.dockerenv").exists() && std::env::var("container").is_err()
            {
                return Err(RenderError::Configuration(
                    "sandbox override requires a container".into(),
                ));
            }
            config.container_no_sandbox = true;
        }
        config.validate()?;
        Ok(config)
    }
    fn validate(&self) -> Result<()> {
        if !(1..=32).contains(&self.size)
            || self.export_timeout.is_zero()
            || self.export_timeout > Duration::from_secs(30)
        {
            return Err(RenderError::Configuration(
                "pool size must be 1–32 and timeout must be in (0, 30s]".into(),
            ));
        }
        Ok(())
    }
}

struct Session {
    browser: Browser,
    handler: JoinHandle<()>,
    generation: u64,
    _profile: tempfile::TempDir,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.handler.abort();
    }
}

struct Controller {
    config: PoolConfig,
    session: Option<Session>,
    generation: u64,
    closed: bool,
}
impl Controller {
    async fn launch(&mut self) -> Result<()> {
        if self.closed {
            return Err(RenderError::Closed);
        }
        let profile = tempfile::Builder::new()
            .prefix("mdpdf-chrome-")
            .tempdir()
            .map_err(|error| RenderError::Configuration(error.to_string()))?;
        let mut builder = BrowserConfig::builder()
            .new_headless_mode()
            .enable_request_intercept()
            .disable_cache()
            .user_data_dir(profile.path())
            .arg("--disable-gpu")
            .launch_timeout(Duration::from_secs(15))
            .request_timeout(Duration::from_secs(5));
        if let Some(path) = &self.config.chrome_path {
            builder = builder.chrome_executable(path);
        }
        if self.config.container_no_sandbox {
            builder = builder.no_sandbox();
        }
        let config = builder.build().map_err(RenderError::Configuration)?;
        let (browser, mut handler) = Browser::launch(config).await?;
        let handler = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if event.is_err() {
                    break;
                }
            }
        });
        self.generation += 1;
        self.session = Some(Session {
            browser,
            handler,
            generation: self.generation,
            _profile: profile,
        });
        Ok(())
    }
    async fn restart(&mut self, generation: u64) -> Result<()> {
        if self.closed {
            return Err(RenderError::Closed);
        }
        // Another worker may already have replaced the crashed process.
        if self.session.as_ref().is_some_and(|session| {
            session.generation != generation && !session.handler.is_finished()
        }) {
            return Ok(());
        }
        if let Some(mut old) = self.session.take() {
            old.browser.kill().await;
        }
        self.launch().await
    }
    async fn tab(&mut self, current: &mut Option<Tab>) -> Result<()> {
        if self.closed {
            return Err(RenderError::Closed);
        }
        if self.session.as_mut().is_none_or(|session| {
            session.handler.is_finished()
                || session
                    .browser
                    .try_wait()
                    .is_ok_and(|status| status.is_some())
        }) {
            let generation = self.generation;
            self.restart(generation).await?;
        }
        let session = self.session.as_ref().ok_or(RenderError::Closed)?;
        if current
            .as_ref()
            .is_some_and(|tab| tab.generation == session.generation)
        {
            return Ok(());
        }
        // Old-generation tabs belong to the process killed by restart.
        *current = None;
        let page = session.browser.new_page("about:blank").await?;
        let mut events = page.event_listener::<EventRequestPaused>().await?;
        let intercepted = page.clone();
        let intercept = tokio::spawn(async move {
            while let Some(event) = events.next().await {
                let result = if event.request.url == "about:blank"
                    || event.request.url.starts_with("data:")
                {
                    intercepted
                        .execute(ContinueRequestParams::new(event.request_id.clone()))
                        .await
                        .map(|_| ())
                } else {
                    // All document assets are embedded. No HTTP, file, WebSocket,
                    // redirect or untrusted asset-origin request is permitted.
                    intercepted
                        .execute(FailRequestParams::new(
                            event.request_id.clone(),
                            ErrorReason::BlockedByClient,
                        ))
                        .await
                        .map(|_| ())
                };
                if result.is_err() {
                    break;
                }
            }
        });
        *current = Some(Tab {
            page,
            intercept,
            generation: session.generation,
        });
        Ok(())
    }
}
struct Tab {
    page: Page,
    intercept: JoinHandle<()>,
    generation: u64,
}
impl Drop for Tab {
    fn drop(&mut self) {
        self.intercept.abort();
    }
}
impl Tab {
    async fn discard(self) {
        let _ = timeout(Duration::from_secs(1), self.page.clone().close()).await;
    }
}

struct Job {
    document: Arc<PrintDocument>,
    options: ExportOptions,
    deadline: Instant,
    response: oneshot::Sender<Result<RenderedPdf>>,
}
struct Workers {
    handles: Vec<JoinHandle<()>>,
}
impl Drop for Workers {
    fn drop(&mut self) {
        for handle in &self.handles {
            handle.abort();
        }
    }
}

/// One Chromium process, reusable tabs, and a bounded, cancellation-aware queue.
#[derive(Clone)]
pub struct BrowserPool {
    sender: mpsc::Sender<Job>,
    controller: Arc<Mutex<Controller>>,
    workers: Arc<Workers>,
    export_timeout: Duration,
}
impl BrowserPool {
    pub async fn new(config: PoolConfig) -> Result<Self> {
        config.validate()?;
        let mut controller = Controller {
            config: config.clone(),
            session: None,
            generation: 0,
            closed: false,
        };
        controller.launch().await?;
        let mut tabs = Vec::new();
        for _ in 0..config.size {
            let mut tab = None;
            controller.tab(&mut tab).await?;
            tabs.push(tab);
        }
        let controller = Arc::new(Mutex::new(controller));
        let (sender, receiver) = mpsc::channel::<Job>(config.size * 2);
        let receiver = Arc::new(Mutex::new(receiver));
        let mut handles = Vec::new();
        for tab in tabs {
            let controller = Arc::clone(&controller);
            let receiver = Arc::clone(&receiver);
            handles.push(tokio::spawn(worker(controller, receiver, tab)));
        }
        Ok(Self {
            sender,
            controller,
            workers: Arc::new(Workers { handles }),
            export_timeout: config.export_timeout,
        })
    }

    /// Build the same print document used for preview, then export it.
    pub async fn export(
        &self,
        markdown: &str,
        theme: &Theme,
        options: ExportOptions,
    ) -> Result<RenderedPdf> {
        let deadline = Instant::now() + self.export_timeout;
        let markdown = markdown.to_owned();
        let theme = theme.clone();
        let document = timeout_at(
            deadline,
            prepare_document_with_assets(&markdown, &theme, &AssetContext::default()),
        )
        .await
        .map_err(|_| RenderError::Timeout)??;
        self.submit(document, options, deadline).await
    }

    /// Export a prepared document. The caller may use the identical HTML for preview.
    pub async fn render(
        &self,
        document: PrintDocument,
        options: ExportOptions,
    ) -> Result<RenderedPdf> {
        self.submit(document, options, Instant::now() + self.export_timeout)
            .await
    }
    async fn submit(
        &self,
        document: PrintDocument,
        options: ExportOptions,
        deadline: Instant,
    ) -> Result<RenderedPdf> {
        let (response, receiver) = oneshot::channel();
        let job = Job {
            document: Arc::new(document),
            options,
            deadline,
            response,
        };
        timeout_at(deadline, async {
            self.sender
                .send(job)
                .await
                .map_err(|_| RenderError::Closed)?;
            receiver.await.map_err(|_| RenderError::Closed)?
        })
        .await
        .map_err(|_| RenderError::Timeout)?
    }
    pub async fn is_ready(&self) -> bool {
        let controller = self.controller.lock().await;
        !controller.closed
            && controller
                .session
                .as_ref()
                .is_some_and(|session| !session.handler.is_finished())
    }
    pub async fn close(&self) {
        let mut controller = self.controller.lock().await;
        controller.closed = true;
        for worker in &self.workers.handles {
            worker.abort();
        }
        if let Some(mut session) = controller.session.take() {
            if !matches!(
                timeout(Duration::from_secs(2), session.browser.close()).await,
                Ok(Ok(_))
            ) {
                session.browser.kill().await;
            }
            let _ = timeout(Duration::from_secs(2), session.browser.wait()).await;
        }
    }
}

async fn worker(
    controller: Arc<Mutex<Controller>>,
    receiver: Arc<Mutex<mpsc::Receiver<Job>>>,
    mut tab: Option<Tab>,
) {
    loop {
        let Some(job) = receiver.lock().await.recv().await else {
            break;
        };
        if job.response.is_closed() || Instant::now() >= job.deadline {
            continue;
        }
        let result = timeout_at(job.deadline, async {
            for attempt in 0..2 {
                let setup = controller.lock().await.tab(&mut tab).await;
                let result = match setup {
                    Ok(()) => {
                        render_page(
                            &tab.as_ref().expect("tab initialized").page,
                            &job.document,
                            job.options,
                        )
                        .await
                    }
                    Err(error) => Err(error),
                };
                if matches!(result, Err(RenderError::Browser(_))) && attempt == 0 {
                    let generation = tab.as_ref().map_or(0, |tab| tab.generation);
                    tab = None;
                    controller.lock().await.restart(generation).await?;
                    continue;
                }
                return result;
            }
            unreachable!()
        })
        .await
        .unwrap_or(Err(RenderError::Timeout));
        if matches!(result, Err(RenderError::Timeout | RenderError::Browser(_)))
            && let Some(failed) = tab.take()
        {
            failed.discard().await;
        }
        let _ = job.response.send(result);
    }
    if let Some(tab) = tab {
        tab.discard().await;
    }
}

async fn render_page(
    page: &Page,
    document: &PrintDocument,
    options: ExportOptions,
) -> Result<RenderedPdf> {
    page.goto("about:blank").await?;
    page.set_content(&document.html).await?;
    loop {
        if page
            .evaluate("window.__MDPDF_READY__ === true")
            .await?
            .into_value::<bool>()
            .map_err(|error| RenderError::Browser(error.to_string()))?
        {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    let errors: Vec<Diagnostic> = page
        .evaluate("window.__MDPDF_ERRORS__ || []")
        .await?
        .into_value()
        .map_err(|error| RenderError::Browser(error.to_string()))?;
    let mut diagnostics = document.diagnostics.clone();
    diagnostics.extend(errors);
    let diagram_errors: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.severity == Severity::Error && diagnostic.kind == DiagKind::Mermaid
        })
        .cloned()
        .collect();
    if options.strict_diagrams && !diagram_errors.is_empty() {
        return Err(RenderError::DiagramErrors(diagram_errors));
    }
    let mut bytes = page
        .pdf(PrintToPdfParams {
            prefer_css_page_size: Some(true),
            print_background: Some(true),
            generate_tagged_pdf: Some(true),
            generate_document_outline: Some(true),
            display_header_footer: Some(false),
            ..Default::default()
        })
        .await?;
    if document.css.contains("string(section)") {
        let sections = pdf::sections(&bytes)?;
        let mut labelled = Vec::new();
        // CSS named strings are not implemented by Chromium. Keep the same
        // paginated document and print each actual page with its outline label.
        let original: String = page
            .evaluate("document.getElementById('mdpdf-theme').textContent")
            .await?
            .into_value()
            .map_err(|error| RenderError::Browser(error.to_string()))?;
        for (index, section) in sections.iter().enumerate() {
            let section: String = section
                .chars()
                .filter(|character| !character.is_control())
                .collect();
            let css =
                original.replace("string(section)", &serde_json::to_string(&section).unwrap());
            page.evaluate(format!(
                "document.getElementById('mdpdf-theme').textContent = {}",
                serde_json::to_string(&css).unwrap()
            ))
            .await?;
            labelled.push(
                page.pdf(PrintToPdfParams {
                    prefer_css_page_size: Some(true),
                    print_background: Some(true),
                    generate_tagged_pdf: Some(true),
                    display_header_footer: Some(false),
                    page_ranges: Some((index + 1).to_string()),
                    ..Default::default()
                })
                .await?,
            );
        }
        bytes = pdf::replace_page_drawings(&bytes, labelled)?;
    }
    let metadata = document.metadata.clone();
    let (bytes, page_count) = tokio::task::spawn_blocking(move || pdf::metadata(&bytes, &metadata))
        .await
        .map_err(|error| RenderError::Pdf(error.to_string()))??;
    Ok(RenderedPdf {
        bytes,
        diagnostics,
        page_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare_document;
    use std::{io::ErrorKind, net::TcpListener};

    #[test]
    fn invalid_pool_limits_are_rejected() {
        for size in [0, 33] {
            assert!(
                PoolConfig {
                    size,
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            PoolConfig {
                export_timeout: Duration::from_secs(31),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }

    #[tokio::test]
    #[ignore = "requires Chromium; checks crash recovery, network blocking, and timeout recovery"]
    async fn lifecycle_and_network_isolation() {
        let mut pool = BrowserPool::new(PoolConfig {
            size: 1,
            export_timeout: Duration::from_secs(30),
            ..Default::default()
        })
        .await
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut document = prepare_document("# Network isolation", &Theme::default()).unwrap();
        // Deliberately bypass core CSP in this test to exercise the independent
        // Chromium request interceptor rather than the sanitizer/CSP alone.
        document.html = format!(
            r#"<!doctype html><h1>Network isolation</h1><script>
          window.__MDPDF_ERRORS__ = [];
          fetch('http://{}/must-never-arrive').then(() => {{
            window.__MDPDF_ERRORS__.push({{severity:'Error', kind:'Html', message:'Request escaped', source_line:null}});
          }}).catch(() => {{}}).finally(() => {{window.__MDPDF_READY__ = true;}});
        </script>"#,
            listener.local_addr().unwrap()
        );
        let result = pool
            .render(document, ExportOptions::default())
            .await
            .unwrap();
        assert!(result.diagnostics.is_empty());
        assert_eq!(listener.accept().unwrap_err().kind(), ErrorKind::WouldBlock);
        let generation = {
            let mut controller = pool.controller.lock().await;
            let session = controller.session.as_mut().unwrap();
            session.browser.kill().await.unwrap().unwrap();
            session.generation
        };
        let output = pool
            .export(
                "# After browser crash",
                &Theme::default(),
                ExportOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(output.page_count, 1);
        assert!(pool.controller.lock().await.generation > generation);
        pool.export_timeout = Duration::from_secs(3);
        let mut stuck = prepare_document("# Timeout", &Theme::default()).unwrap();
        stuck.html = "<!doctype html><h1>Never ready</h1>".into();
        assert!(matches!(
            pool.render(stuck, ExportOptions::default()).await,
            Err(RenderError::Timeout)
        ));
        pool.export_timeout = Duration::from_secs(30);
        let output = pool
            .export(
                "# After timed out export",
                &Theme::default(),
                ExportOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(output.page_count, 1);
        pool.close().await;
    }
}
