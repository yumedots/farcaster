//! Set up graphics before loading drivers.
use std::{ffi::OsString, process::Command};

const VULKAN_DRIVER_CONFIGURATION: [&str; 5] = [
    "VK_DRIVER_FILES",
    "VK_ICD_FILENAMES",
    "VK_ADD_DRIVER_FILES",
    "VK_LOADER_DRIVERS_SELECT",
    "VK_LOADER_DRIVERS_DISABLE",
];

type Environment = Vec<(&'static str, OsString)>;

pub(crate) fn relaunch() -> Result<(), String> {
    use std::os::unix::process::CommandExt as _;

    let is_wsl = std::env::var_os("WSL_INTEROP").is_some()
        || std::env::var_os("WSL_DISTRO_NAME").is_some()
        || ["/proc/sys/kernel/osrelease", "/proc/version"]
            .into_iter()
            .any(|path| {
                std::fs::read_to_string(path)
                    .is_ok_and(|version| kernel_version_reports_wsl(&version))
            });
    let mut environment: Environment = Vec::new();
    if should_disable_dzn(is_wsl, |name| std::env::var_os(name).is_some()) {
        environment.push(("VK_LOADER_DRIVERS_DISABLE", "*dzn*".into()));
    }
    if environment.is_empty() {
        return Ok(());
    }

    let executable = std::env::current_exe()
        .map_err(|error| format!("resolve farcaster executable for graphics setup: {error}"))?;
    let error = Command::new(executable)
        .args(std::env::args_os().skip(1))
        .envs(environment)
        .exec();
    Err(format!(
        "relaunch farcaster with host graphics setup: {error}"
    ))
}

fn should_disable_dzn(is_wsl: bool, mut environment_is_set: impl FnMut(&str) -> bool) -> bool {
    !is_wsl
        && !VULKAN_DRIVER_CONFIGURATION
            .into_iter()
            .any(&mut environment_is_set)
}

fn kernel_version_reports_wsl(version: &str) -> bool {
    version.to_ascii_lowercase().contains("microsoft")
}

#[cfg(test)]
#[path = "linux_graphics_tests.rs"]
mod tests;
