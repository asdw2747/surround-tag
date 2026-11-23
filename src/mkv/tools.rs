//! External tool discovery for MKVToolNix and ffprobe.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::config::AppConfig;

use super::runner;

#[derive(Clone, Debug)]
pub struct ToolCommand {
    pub program: PathBuf,
    pub prefix: Vec<OsString>,
    pub label: String,
    pub version: Option<String>,
}

impl ToolCommand {
    pub fn direct(program: PathBuf) -> Self {
        Self {
            program,
            prefix: Vec::new(),
            label: String::new(),
            version: None,
        }
    }

    pub fn flatpak(tool: &str) -> Self {
        Self {
            program: PathBuf::from("flatpak"),
            prefix: vec![
                OsString::from("run"),
                OsString::from(format!("--command={tool}")),
                OsString::from("org.bunkus.mkvtoolnix-gui"),
            ],
            label: format!("Flatpak · {tool}"),
            version: None,
        }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.prefix);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        command
    }

    pub fn display(&self) -> String {
        if self.prefix.is_empty() {
            self.program.display().to_string()
        } else {
            let prefix = self
                .prefix
                .iter()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(" ");
            format!("{} {}", self.program.display(), prefix)
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ToolSet {
    pub mkvpropedit: Option<ToolCommand>,
    pub mkvmerge: Option<ToolCommand>,
    pub mkvextract: Option<ToolCommand>,
    pub ffprobe: Option<ToolCommand>,
    pub notes: Vec<String>,
}

impl ToolSet {
    pub fn detect(config: &AppConfig) -> Self {
        let mut notes = Vec::new();
        let mkvpropedit = detect_tool(
            "mkvpropedit",
            config.mkvpropedit_path.as_deref(),
            true,
            &mut notes,
        );
        let mkvmerge = detect_tool(
            "mkvmerge",
            config.mkvmerge_path.as_deref(),
            true,
            &mut notes,
        );
        let mkvextract = detect_tool(
            "mkvextract",
            config.mkvextract_path.as_deref(),
            true,
            &mut notes,
        );
        let ffprobe = detect_tool("ffprobe", config.ffprobe_path.as_deref(), false, &mut notes);

        if mkvpropedit.is_some() && mkvmerge.is_none() {
            notes.push(
                "mkvpropedit was found, but mkvmerge is missing. Target introspection and verification will be limited."
                    .to_owned(),
            );
        }

        Self {
            mkvpropedit,
            mkvmerge,
            mkvextract,
            ffprobe,
            notes,
        }
    }

    pub fn can_apply(&self) -> bool {
        self.mkvpropedit.is_some()
    }

    pub fn summary(&self) -> String {
        match &self.mkvpropedit {
            Some(tool) => {
                let version = tool.version.as_deref().unwrap_or("unknown version");
                if tool.prefix.is_empty() {
                    format!("{version} · {}", tool.program.display())
                } else {
                    format!("{version} · Flatpak")
                }
            }
            None => "MKVToolNix not found".to_owned(),
        }
    }
}

fn detect_tool(
    name: &str,
    configured: Option<&Path>,
    mkv_tool: bool,
    notes: &mut Vec<String>,
) -> Option<ToolCommand> {
    let env_name = format!("SURROUNDTAG_{}", name.to_ascii_uppercase());
    if let Some(path) = std::env::var_os(env_name) {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return finish(ToolCommand::direct(candidate), name, notes);
        }
    }

    if let Some(path) = configured {
        if path.is_file() {
            return finish(ToolCommand::direct(path.to_path_buf()), name, notes);
        }
        notes.push(format!(
            "Configured {name} path does not exist: {}",
            path.display()
        ));
    }

    if let Ok(path) = which::which(name) {
        if path.is_file() {
            return finish(ToolCommand::direct(path), name, notes);
        }
    }

    if mkv_tool && cfg!(target_os = "linux") && flatpak_available() {
        let tool = ToolCommand::flatpak(name);
        if version_of(&tool).is_some() {
            return finish(tool, name, notes);
        }
    }

    for path in standard_paths(name) {
        if path.is_file() {
            return finish(ToolCommand::direct(path), name, notes);
        }
    }

    None
}

fn finish(mut tool: ToolCommand, name: &str, notes: &mut Vec<String>) -> Option<ToolCommand> {
    tool.version = version_of(&tool);
    if tool.version.is_none() {
        notes.push(format!(
            "Found {name} at {}, but it did not respond to a version check.",
            tool.display()
        ));
        return None;
    }
    if tool.prefix.is_empty() {
        tool.label = tool.program.display().to_string();
    }
    Some(tool)
}

fn version_of(tool: &ToolCommand) -> Option<String> {
    let argument = if tool.program.file_name().and_then(|name| name.to_str()) == Some("ffprobe")
        || tool
            .prefix
            .iter()
            .any(|value| value.to_string_lossy().contains("ffprobe"))
    {
        "-version"
    } else {
        "--version"
    };
    let output = runner::run(tool, &[OsString::from(argument)], Duration::from_secs(10)).ok()?;
    if !output.success() {
        return None;
    }
    let text = if output.stdout.is_empty() {
        output.stderr
    } else {
        output.stdout
    };
    text.lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_owned())
}

fn flatpak_available() -> bool {
    Command::new("flatpak")
        .args(["info", "org.bunkus.mkvtoolnix-gui"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn standard_paths(name: &str) -> Vec<PathBuf> {
    let executable = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    let mut paths = Vec::new();

    if cfg!(windows) {
        for variable in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
            if let Some(root) = std::env::var_os(variable) {
                paths.push(PathBuf::from(root).join("MKVToolNix").join(&executable));
            }
        }
        if let Some(root) = std::env::var_os("LOCALAPPDATA") {
            paths.push(
                PathBuf::from(root)
                    .join("Programs")
                    .join("MKVToolNix")
                    .join(&executable),
            );
        }
    } else {
        for root in ["/usr/bin", "/usr/local/bin", "/snap/bin", "/opt/mkvtoolnix"] {
            paths.push(PathBuf::from(root).join(&executable));
        }
    }

    paths
}
