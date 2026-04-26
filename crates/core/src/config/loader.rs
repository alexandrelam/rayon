use super::bookmarks::load_bookmarks;
use super::manifest::PluginManifest;
use crate::declarative_provider::{DeclarativeCommandProvider, ExecutableCommandSpec};
use crate::CommandProvider;
use rayon_types::CommandInputMode;
use rayon_types::{BookmarkDefinition, CommandDefinition, CommandId};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub struct LoadedConfig {
    pub command_providers: Vec<Arc<dyn CommandProvider>>,
    pub bookmarks: Vec<BookmarkDefinition>,
}

pub(super) struct LoadedManifestBundle {
    pub command_provider: DeclarativeCommandProvider,
    pub bookmarks: Vec<BookmarkDefinition>,
}

pub(super) fn load_manifest_bundle(
    base_dir: &Path,
    manifest: PluginManifest,
) -> Result<LoadedManifestBundle, String> {
    let plugin_id = manifest.plugin_id;
    let commands = manifest.commands.unwrap_or_default();
    let bookmarks = manifest.bookmarks.unwrap_or_default();
    let mut command_definitions = Vec::with_capacity(commands.len());
    let mut commands_by_id = HashMap::with_capacity(commands.len());

    for command in commands {
        if command.input_mode == CommandInputMode::RawArgv
            && command
                .arguments
                .as_ref()
                .is_some_and(|arguments| !arguments.is_empty())
        {
            return Err(format!(
                "command '{}' cannot define arguments when input_mode is raw_argv",
                command.id
            ));
        }

        let command_id = CommandId::from(command.id.clone());
        let definition = CommandDefinition {
            id: command_id.clone(),
            title: command.title.clone(),
            subtitle: command.subtitle.clone(),
            owner_plugin_id: plugin_id.clone(),
            keywords: command.keywords.clone().unwrap_or_default(),
            input_mode: command.input_mode,
            arguments: command
                .arguments
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
        };
        let spec = ExecutableCommandSpec::from_manifest_command(base_dir, &definition, command);

        command_definitions.push(definition);
        commands_by_id.insert(command_id.to_string(), spec);
    }

    Ok(LoadedManifestBundle {
        command_provider: DeclarativeCommandProvider::new(command_definitions, commands_by_id),
        bookmarks: load_bookmarks(&plugin_id, bookmarks)?,
    })
}
