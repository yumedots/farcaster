use std::ffi::OsStr;
use std::path::Path;

use crate::agents::{program_available, program_available_in};

pub(crate) struct TextEditor {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) program: &'static str,
    pub(crate) icon: EditorIcon,
    pub(crate) line_argument: bool,
    pub(crate) directory_argument: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EditorIcon {
    Neovim,
    Vim,
    Helix,
    Micro,
    Emacs,
    Nano,
    Generic,
}

pub(crate) const TEXT_EDITORS: [TextEditor; 8] = [
    TextEditor {
        id: "nvim",
        name: "Neovim",
        program: "nvim",
        icon: EditorIcon::Neovim,
        line_argument: true,
        directory_argument: true,
    },
    TextEditor {
        id: "vim",
        name: "Vim",
        program: "vim",
        icon: EditorIcon::Vim,
        line_argument: true,
        directory_argument: true,
    },
    TextEditor {
        id: "helix",
        name: "Helix",
        program: "hx",
        icon: EditorIcon::Helix,
        line_argument: false,
        directory_argument: true,
    },
    TextEditor {
        id: "micro",
        name: "Micro",
        program: "micro",
        icon: EditorIcon::Micro,
        line_argument: true,
        directory_argument: false,
    },
    TextEditor {
        id: "vi",
        name: "vi",
        program: "vi",
        icon: EditorIcon::Vim,
        line_argument: true,
        directory_argument: true,
    },
    TextEditor {
        id: "kakoune",
        name: "Kakoune",
        program: "kak",
        icon: EditorIcon::Generic,
        line_argument: false,
        directory_argument: true,
    },
    TextEditor {
        id: "emacs",
        name: "Emacs",
        program: "emacs",
        icon: EditorIcon::Emacs,
        line_argument: true,
        directory_argument: true,
    },
    TextEditor {
        id: "nano",
        name: "nano",
        program: "nano",
        icon: EditorIcon::Nano,
        line_argument: true,
        directory_argument: false,
    },
];

pub(crate) struct TextEditorStatus {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) program: String,
    pub(crate) icon: EditorIcon,
    pub(crate) available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EditorCommand {
    pub(crate) program: String,
    pub(crate) arguments: Vec<String>,
}

impl EditorCommand {
    pub(crate) fn parse(command: &str) -> Result<Self, String> {
        let mut parts = split_command(command)?;
        if parts.is_empty() {
            return Err("The editor command is empty.".into());
        }
        let program = parts.remove(0);
        Ok(Self {
            program,
            arguments: parts,
        })
    }

    pub(crate) fn of(editor: &TextEditor) -> Self {
        Self {
            program: editor_program(editor),
            arguments: Vec::new(),
        }
    }

    pub(crate) fn command_line(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.arguments.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub(crate) fn name(&self) -> String {
        let program = self.program_name();
        text_editor(&program).map_or(program, |editor| editor.name.to_owned())
    }

    pub(crate) fn icon(&self) -> EditorIcon {
        text_editor(&self.program_name()).map_or(EditorIcon::Generic, |editor| editor.icon)
    }

    pub(crate) fn is_neovim(&self) -> bool {
        self.program_name() == "nvim" || self.program == nvim_program()
    }

    pub(crate) fn supports_line_argument(&self) -> bool {
        text_editor(&self.program_name()).is_some_and(|editor| editor.line_argument)
    }

    pub(crate) fn supports_directory_argument(&self) -> bool {
        text_editor(&self.program_name()).is_none_or(|editor| editor.directory_argument)
    }

    pub(crate) fn available(&self) -> bool {
        program_available(Path::new(&self.program))
    }

    fn program_name(&self) -> String {
        Path::new(&self.program).file_name().map_or_else(
            || self.program.clone(),
            |name| name.to_string_lossy().into_owned(),
        )
    }
}

pub(crate) fn text_editor(id: &str) -> Option<&'static TextEditor> {
    TEXT_EDITORS
        .iter()
        .find(|editor| editor.id == id || editor.program == id)
}

pub(crate) fn text_editor_statuses() -> Vec<TextEditorStatus> {
    text_editor_statuses_in(std::env::var_os("PATH").as_deref())
}

fn text_editor_statuses_in(search_path: Option<&OsStr>) -> Vec<TextEditorStatus> {
    TEXT_EDITORS
        .iter()
        .map(|editor| TextEditorStatus {
            id: editor.id,
            name: editor.name,
            program: editor_program(editor),
            icon: editor.icon,
            available: editor_available(editor, search_path),
        })
        .collect()
}

pub(crate) fn installed_text_editor() -> Option<&'static TextEditor> {
    installed_text_editor_in(std::env::var_os("PATH").as_deref())
}

fn installed_text_editor_in(search_path: Option<&OsStr>) -> Option<&'static TextEditor> {
    TEXT_EDITORS
        .iter()
        .find(|editor| editor_available(editor, search_path))
}

fn editor_available(editor: &TextEditor, search_path: Option<&OsStr>) -> bool {
    program_available_in(Path::new(&editor_program(editor)), search_path)
}

fn editor_program(editor: &TextEditor) -> String {
    if editor.id == "nvim" {
        return nvim_program();
    }
    editor.program.to_owned()
}

fn nvim_program() -> String {
    std::env::var("FARCASTER_NVIM")
        .or_else(|_| std::env::var("GPUI_NVIM"))
        .ok()
        .filter(|program| !program.trim().is_empty())
        .unwrap_or_else(|| "nvim".to_owned())
}

#[cfg(test)]
pub(crate) fn neovim_executable() -> std::path::PathBuf {
    std::path::PathBuf::from(nvim_program())
}

pub(crate) fn resolve_text_editor(saved: Option<&str>) -> Result<EditorCommand, String> {
    match saved.map(str::trim).filter(|saved| !saved.is_empty()) {
        Some(saved) => EditorCommand::parse(saved),
        None => installed_text_editor()
            .map(EditorCommand::of)
            .ok_or_else(|| {
                "No text editor found. Install one or set a command in Settings.".to_owned()
            }),
    }
}

fn split_command(command: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut started = false;
    let mut characters = command.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\'' | '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' if !quoted => {
                match characters.next() {
                    Some(escaped) => {
                        current.push(escaped);
                        started = true;
                    }
                    None => current.push(character),
                };
            }
            character if character.is_whitespace() && !quoted => {
                if started {
                    parts.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            character => {
                current.push(character);
                started = true;
            }
        }
    }
    if quoted {
        return Err("The editor command has an unclosed quote.".into());
    }
    if started {
        parts.push(current);
    }
    Ok(parts)
}

#[cfg(test)]
#[path = "editors_tests.rs"]
mod tests;
