//! File storage: data directory, session journal, user content, export (A§5.3).
//!
//! Reads never create anything (D-62); the first write creates directories.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use oraculum_core::catalog::{ItemKind, LoadWarning, Source, WarningKind};
use oraculum_core::journal::{self, Entry, JournalError, JournalLoad};
use thiserror::Error;

/// Largest user content file that is read (D-35).
const MAX_CONTENT_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StorageError {
    #[error("no data directory is available on this system; pass --data-dir <DIR>")]
    NoDataDir,
    #[error("invalid session name {0:?} (use 1 to 64 letters, digits, '_' or '-')")]
    InvalidSessionName(String),
    #[error("cannot access {}: {reason}", path.display())]
    Io { path: PathBuf, reason: String },
    #[error(transparent)]
    Journal(#[from] JournalError),
}

fn io_err(path: &Path, e: &std::io::Error) -> StorageError {
    StorageError::Io {
        path: path.to_path_buf(),
        reason: e.to_string(),
    }
}

/// A session name that is safe to use as a file name (D-31).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionName(String);

impl SessionName {
    pub fn parse(s: &str) -> Result<SessionName, StorageError> {
        let ok = (1..=64).contains(&s.len())
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if ok {
            Ok(SessionName(s.to_string()))
        } else {
            Err(StorageError::InvalidSessionName(s.to_string()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// `--data-dir` if given, else the platform data directory for `oraculum`.
    pub fn resolve(flag: Option<PathBuf>) -> Result<DataDir, StorageError> {
        let root = match flag {
            Some(p) => p,
            None => ProjectDirs::from("", "", "oraculum")
                .ok_or(StorageError::NoDataDir)?
                .data_dir()
                .to_path_buf(),
        };
        Ok(DataDir { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn tables_dir(&self) -> PathBuf {
        self.root.join("tables")
    }

    pub fn generators_dir(&self) -> PathBuf {
        self.root.join("generators")
    }

    pub fn session_path(&self, session: &SessionName) -> PathBuf {
        self.root
            .join("sessions")
            .join(format!("{}.jsonl", session.as_str()))
    }

    pub fn export_path(&self) -> PathBuf {
        self.root.join("export.md")
    }
}

/// One session's JSON Lines file.
#[derive(Debug, Clone)]
pub struct JournalStore {
    path: PathBuf,
}

impl JournalStore {
    pub fn new(path: PathBuf) -> JournalStore {
        JournalStore { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A missing file is an empty journal. Bytes that are not UTF-8 only spoil
    /// the lines they are on.
    pub fn load(&self) -> Result<JournalLoad, StorageError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(journal::decode(&String::from_utf8_lossy(&bytes))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(JournalLoad::default()),
            Err(e) => Err(io_err(&self.path, &e)),
        }
    }

    /// Append one entry with a single write. If a crash left a partial last
    /// line, start on a new line so the two are not merged (D-32).
    pub fn append(&self, entry: &Entry) -> Result<(), StorageError> {
        let line = journal::encode_line(entry)?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| io_err(dir, &e))?;
        }
        let mut buf = String::new();
        if self.ends_mid_line()? {
            buf.push('\n');
        }
        buf.push_str(&line);
        buf.push('\n');
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| io_err(&self.path, &e))?;
        file.write_all(buf.as_bytes())
            .map_err(|e| io_err(&self.path, &e))
    }

    fn ends_mid_line(&self) -> Result<bool, StorageError> {
        let mut file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(io_err(&self.path, &e)),
        };
        let len = file.metadata().map_err(|e| io_err(&self.path, &e))?.len();
        if len == 0 {
            return Ok(false);
        }
        let mut last = [0u8; 1];
        file.seek(SeekFrom::End(-1))
            .and_then(|_| file.read_exact(&mut last))
            .map_err(|e| io_err(&self.path, &e))?;
        Ok(last[0] != b'\n')
    }
}

fn warn(origin: String, kind: WarningKind, message: String) -> LoadWarning {
    LoadWarning {
        origin,
        item: None,
        kind,
        message,
        kept_builtin: false,
    }
}

/// Read user tables then generators, each sorted by file name. Unreadable,
/// non-UTF-8 and oversized files become warnings (D-35); a missing folder is
/// not a problem (D-62).
pub fn load_user_sources(dir: &DataDir) -> (Vec<Source>, Vec<LoadWarning>) {
    let mut sources = Vec::new();
    let mut warnings = Vec::new();
    for (kind, folder, label) in [
        (ItemKind::Table, dir.tables_dir(), "tables"),
        (ItemKind::Generator, dir.generators_dir(), "generators"),
    ] {
        let mut names: Vec<String> = match std::fs::read_dir(&folder) {
            Ok(rd) => rd
                .filter_map(Result::ok)
                .filter(|e| e.path().is_file())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| is_content_file(n))
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                warnings.push(warn(
                    label.to_string(),
                    WarningKind::FileUnreadable,
                    format!("cannot read the folder: {e}"),
                ));
                continue;
            }
        };
        names.sort();
        for name in names {
            let origin = format!("{label}/{name}");
            let path = folder.join(&name);
            match read_content(&path) {
                Ok(text) => sources.push(Source {
                    kind,
                    origin,
                    file_stem: Path::new(&name)
                        .file_stem()
                        .map_or_else(String::new, |s| s.to_string_lossy().into_owned()),
                    text,
                }),
                Err((kind, message)) => warnings.push(warn(origin, kind, message)),
            }
        }
    }
    (sources, warnings)
}

fn is_content_file(name: &str) -> bool {
    !name.starts_with('.')
        && !name.ends_with('~')
        && Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("toml"))
}

fn read_content(path: &Path) -> Result<String, (WarningKind, String)> {
    let unreadable = |e: std::io::Error| (WarningKind::FileUnreadable, e.to_string());
    let len = std::fs::metadata(path).map_err(unreadable)?.len();
    if len > MAX_CONTENT_BYTES {
        return Err((
            WarningKind::FileTooLarge,
            format!("file is {len} bytes; the limit is {MAX_CONTENT_BYTES}"),
        ));
    }
    let bytes = std::fs::read(path).map_err(unreadable)?;
    String::from_utf8(bytes).map_err(|_| {
        (
            WarningKind::FileUnreadable,
            "file is not valid UTF-8".to_string(),
        )
    })
}

/// Write the export through a temporary file in the same folder, then rename.
pub fn write_export(path: &Path, markdown: &str) -> Result<(), StorageError> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| io_err(dir, &e))?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, markdown).map_err(|e| io_err(&tmp, &e))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        io_err(path, &e)
    })
}
