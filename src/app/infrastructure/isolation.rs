use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub(crate) const ARGUMENT: &str = "--isolated";

static ISOLATED: OnceLock<Isolated> = OnceLock::new();

struct Isolated {
    data_dir: PathBuf,
}

pub(crate) fn split(arguments: impl Iterator<Item = OsString>) -> (Option<PathBuf>, bool) {
    let mut project = None;
    let mut isolated = false;
    for argument in arguments {
        if argument == ARGUMENT {
            isolated = true;
        } else if project.is_none() {
            project = Some(PathBuf::from(argument));
        }
    }
    (project, isolated)
}

pub(crate) fn is_isolated() -> bool {
    ISOLATED.get().is_some()
}

pub(crate) fn data_dir() -> Option<&'static Path> {
    ISOLATED.get().map(|isolated| isolated.data_dir.as_path())
}

pub(crate) fn install(source: &Path) -> Result<String, String> {
    let data_dir = std::env::temp_dir().join(format!("farcaster-isolated.{}", std::process::id()));
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("create {}: {error}", data_dir.display()))?;
    if source.is_dir() {
        copy_tree(source, &data_dir)?;
    }
    let summary = format!(
        "isolated data={} pid={}",
        data_dir.display(),
        std::process::id()
    );
    ISOLATED
        .set(Isolated { data_dir })
        .map_err(|_| "isolation was already installed".to_owned())?;
    Ok(summary)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in
        fs::read_dir(source).map_err(|error| format!("read {}: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("read {}: {error}", source.display()))?;
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            fs::create_dir_all(&target)
                .map_err(|error| format!("create {}: {error}", target.display()))?;
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)
                .map_err(|error| format!("copy {}: {error}", entry.path().display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "isolation_tests.rs"]
mod tests;
