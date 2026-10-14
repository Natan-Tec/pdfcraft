//! The Android stand-in for the parts of `rfd` PdfCraft uses.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

static MAILBOX: OnceLock<PathBuf> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
/// How long a picker may stay open before the request is given up (the user walked away).
const GIVE_UP: Duration = Duration::from_secs(30 * 60);

/// Tells the shim where the Kotlin activity watches for requests: `<dir>/requests` in,
/// `<dir>/results` out. Call once at startup.
pub fn set_mailbox(dir: PathBuf) {
    let _ = std::fs::create_dir_all(dir.join("requests"));
    let _ = std::fs::create_dir_all(dir.join("results"));
    let _ = MAILBOX.set(dir);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    OpenOne,
    OpenMany,
    Folder,
    Save,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::OpenOne => "open",
            Kind::OpenMany => "open-many",
            Kind::Folder => "folder",
            Kind::Save => "save",
        }
    }
}

/// A chosen file. Only the path is used by the app.
#[derive(Clone, Debug)]
pub struct FileHandle {
    path: PathBuf,
}

impl FileHandle {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

#[derive(Clone, Default, Debug)]
pub struct AsyncFileDialog {
    title: String,
    file_name: String,
    extensions: Vec<String>,
}

impl AsyncFileDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_filter(mut self, _name: impl Into<String>, extensions: &[impl ToString]) -> Self {
        for e in extensions {
            let e = e.to_string();
            if !self.extensions.contains(&e) {
                self.extensions.push(e);
            }
        }
        self
    }

    pub fn set_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn set_file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = name.into();
        self
    }

    pub fn set_directory(self, _dir: impl AsRef<Path>) -> Self {
        self
    }

    pub fn pick_file(self) -> impl Future<Output = Option<FileHandle>> {
        let ask = Ask::start(&self, Kind::OpenOne);
        async move { ask.await.into_iter().next() }
    }

    pub fn pick_files(self) -> impl Future<Output = Option<Vec<FileHandle>>> {
        let ask = Ask::start(&self, Kind::OpenMany);
        async move {
            let all = ask.await;
            if all.is_empty() { None } else { Some(all) }
        }
    }

    pub fn pick_folder(self) -> impl Future<Output = Option<FileHandle>> {
        let ask = Ask::start(&self, Kind::Folder);
        async move { ask.await.into_iter().next() }
    }

    pub fn save_file(self) -> impl Future<Output = Option<FileHandle>> {
        let ask = Ask::start(&self, Kind::Save);
        async move { ask.await.into_iter().next() }
    }
}

struct Shared {
    done: Option<Vec<FileHandle>>,
    waker: Option<Waker>,
}

/// One request in flight: a poller thread watches for the result file and wakes the future.
struct Ask {
    shared: Arc<Mutex<Shared>>,
}

impl Ask {
    fn start(dialog: &AsyncFileDialog, kind: Kind) -> Ask {
        let shared = Arc::new(Mutex::new(Shared { done: None, waker: None }));
        let ask = Ask { shared: shared.clone() };
        let Some(dir) = MAILBOX.get().cloned() else {
            finish(&shared, Vec::new());
            return ask;
        };
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let id = format!("{}-{id}", std::process::id());
        let body = format!(
            "kind={}\ntitle={}\nname={}\nextensions={}\n",
            kind.word(),
            one_line(&dialog.title),
            one_line(&dialog.file_name),
            dialog.extensions.join(",")
        );
        let tmp = dir.join("requests").join(format!("{id}.tmp"));
        let req = dir.join("requests").join(format!("{id}.req"));
        // Written aside and renamed, so the watcher never sees half a request.
        if std::fs::write(&tmp, body).and_then(|()| std::fs::rename(&tmp, &req)).is_err() {
            finish(&shared, Vec::new());
            return ask;
        }
        let result = dir.join("results").join(format!("{id}.res"));
        std::thread::spawn(move || {
            let started = Instant::now();
            loop {
                if let Ok(text) = std::fs::read_to_string(&result) {
                    let _ = std::fs::remove_file(&result);
                    let handles = text.lines().map(str::trim).filter(|l| !l.is_empty()).map(|l| FileHandle { path: PathBuf::from(l) }).collect();
                    finish(&shared, handles);
                    return;
                }
                if started.elapsed() > GIVE_UP {
                    let _ = std::fs::remove_file(&req);
                    finish(&shared, Vec::new());
                    return;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        });
        ask
    }
}

fn one_line(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

fn finish(shared: &Arc<Mutex<Shared>>, handles: Vec<FileHandle>) {
    if let Ok(mut s) = shared.lock() {
        s.done = Some(handles);
        if let Some(w) = s.waker.take() {
            w.wake();
        }
    }
}

impl Future for Ask {
    type Output = Vec<FileHandle>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Ok(mut s) = self.shared.lock() else { return Poll::Ready(Vec::new()) };
        match s.done.take() {
            Some(h) => Poll::Ready(h),
            None => {
                s.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}
