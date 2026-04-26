use crate::commands::CommandError;
use rayon_types::CommandId;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum LauncherError {
    Command(CommandError),
    AppNotFound(CommandId),
    InteractiveSessionNotFound(String),
    Platform(String),
    SearchBackend(String),
}

impl fmt::Display for LauncherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(error) => write!(f, "{error}"),
            Self::AppNotFound(command_id) => write!(f, "unknown application id: {command_id}"),
            Self::InteractiveSessionNotFound(session_id) => {
                write!(f, "unknown interactive session: {session_id}")
            }
            Self::Platform(error) => write!(f, "{error}"),
            Self::SearchBackend(error) => write!(f, "{error}"),
        }
    }
}

impl Error for LauncherError {}

impl From<CommandError> for LauncherError {
    fn from(value: CommandError) -> Self {
        Self::Command(value)
    }
}
