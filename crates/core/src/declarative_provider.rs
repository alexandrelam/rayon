use crate::config::manifest::ManifestCommand;
use crate::{CommandError, CommandProvider};
use rayon_types::{
    CommandArgumentDefinition, CommandArgumentType, CommandArgumentValue, CommandDefinition,
    CommandExecutionRequest, CommandExecutionResult, CommandInputMode,
};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub(crate) struct DeclarativeCommandProvider {
    command_definitions: Vec<CommandDefinition>,
    commands_by_id: HashMap<String, ExecutableCommandSpec>,
}

impl DeclarativeCommandProvider {
    pub(crate) fn new(
        command_definitions: Vec<CommandDefinition>,
        commands_by_id: HashMap<String, ExecutableCommandSpec>,
    ) -> Self {
        Self {
            command_definitions,
            commands_by_id,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.command_definitions.is_empty()
    }
}

impl CommandProvider for DeclarativeCommandProvider {
    fn commands(&self) -> Vec<CommandDefinition> {
        self.command_definitions.clone()
    }

    fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        let spec = self
            .commands_by_id
            .get(request.command_id.as_str())
            .ok_or_else(|| CommandError::UnknownCommand(request.command_id.clone()))?;

        spec.execute(request)
    }
}

#[derive(Debug)]
pub(crate) struct ExecutableCommandSpec {
    definition: CommandDefinition,
    program: PathBuf,
    base_args: Vec<String>,
    working_dir: Option<PathBuf>,
    env: BTreeMap<String, String>,
}

impl ExecutableCommandSpec {
    pub(crate) fn from_manifest_command(
        base_dir: &Path,
        definition: &CommandDefinition,
        command: ManifestCommand,
    ) -> Self {
        let program = resolve_path(base_dir, &command.program);
        let working_dir = command
            .working_dir
            .as_deref()
            .map(|path| resolve_path(base_dir, path));

        Self {
            definition: definition.clone(),
            program,
            base_args: command.base_args.unwrap_or_default(),
            working_dir,
            env: command.env.unwrap_or_default(),
        }
    }

    fn execute(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        if self.definition.input_mode == CommandInputMode::RawArgv {
            return self.execute_raw_argv(request);
        }

        self.execute_structured(request)
    }

    fn execute_raw_argv(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        let mut argv = self.base_args.clone();
        argv.extend(request.argv.clone());
        self.run_command(argv)
    }

    fn execute_structured(
        &self,
        request: &CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandError> {
        let mut argv = self.base_args.clone();
        let mut positional_values: BTreeMap<usize, String> = BTreeMap::new();

        for argument in &self.definition.arguments {
            let value = request
                .arguments
                .get(&argument.id)
                .cloned()
                .or_else(|| argument.default_value.clone());

            let Some(value) = value else {
                if argument.required {
                    return Err(CommandError::InvalidArguments(format!(
                        "missing required argument '{}'",
                        argument.label
                    )));
                }
                continue;
            };

            let encoded = encode_argument(argument, &value)?;
            if let Some(positional_index) = argument.positional {
                positional_values.insert(positional_index, encoded);
                continue;
            }

            if let Some(flag) = &argument.flag {
                match value {
                    CommandArgumentValue::Boolean(true) => argv.push(flag.clone()),
                    CommandArgumentValue::Boolean(false) => {}
                    CommandArgumentValue::String(_) => {
                        argv.push(flag.clone());
                        argv.push(encoded);
                    }
                }
                continue;
            }

            argv.push(encoded);
        }

        for (_, positional_value) in positional_values {
            argv.push(positional_value);
        }

        self.run_command(argv)
    }

    fn run_command(&self, argv: Vec<String>) -> Result<CommandExecutionResult, CommandError> {
        let mut command = Command::new(&self.program);
        command.args(&argv);
        if let Some(working_dir) = &self.working_dir {
            command.current_dir(working_dir);
        }
        if !self.env.is_empty() {
            command.envs(&self.env);
        }

        let output = command.output().map_err(|error| {
            CommandError::ExecutionFailed(format!(
                "failed to run {}: {error}",
                self.definition.title
            ))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let output_text = if !stdout.is_empty() {
            stdout
        } else if !stderr.is_empty() {
            stderr
        } else {
            format!("completed {}", self.definition.title)
        };

        if !output.status.success() {
            return Err(CommandError::ExecutionFailed(output_text));
        }

        Ok(CommandExecutionResult {
            output: output_text,
        })
    }
}

fn encode_argument(
    definition: &CommandArgumentDefinition,
    value: &CommandArgumentValue,
) -> Result<String, CommandError> {
    match (&definition.argument_type, value) {
        (CommandArgumentType::String, CommandArgumentValue::String(string_value)) => {
            Ok(string_value.clone())
        }
        (CommandArgumentType::Boolean, CommandArgumentValue::Boolean(bool_value)) => {
            Ok(bool_value.to_string())
        }
        (expected_type, actual_value) => Err(CommandError::InvalidArguments(format!(
            "argument '{}' expected {:?}, got {:?}",
            definition.label, expected_type, actual_value
        ))),
    }
}

fn resolve_path(base_dir: &Path, raw_path: &str) -> PathBuf {
    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rayon_types::{CommandExecutionRequest, CommandId};

    #[test]
    fn raw_argv_commands_append_request_argv_after_base_args() {
        let definition = CommandDefinition {
            id: CommandId::from("user.echo"),
            title: "Echo".into(),
            subtitle: None,
            owner_plugin_id: "user.commands".into(),
            keywords: vec!["echo".into()],
            input_mode: CommandInputMode::RawArgv,
            arguments: vec![],
        };
        let spec = ExecutableCommandSpec::from_manifest_command(
            Path::new("/tmp"),
            &definition,
            ManifestCommand {
                id: "user.echo".into(),
                title: "Echo".into(),
                subtitle: None,
                keywords: Some(vec!["echo".into()]),
                input_mode: CommandInputMode::RawArgv,
                program: "/bin/echo".into(),
                base_args: Some(vec!["hello".into()]),
                working_dir: None,
                env: None,
                arguments: None,
            },
        );

        let result = spec
            .execute(&CommandExecutionRequest {
                command_id: CommandId::from("user.echo"),
                argv: vec!["world".into(), "again".into()],
                arguments: HashMap::new(),
            })
            .unwrap();

        assert_eq!(result.output, "hello world again");
    }
}
