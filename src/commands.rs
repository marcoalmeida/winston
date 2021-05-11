use std::collections::HashMap;
use std::fmt;
use std::fs;

use rocket::http::uri::Uri;
use serde::Deserialize;
use tera::Context;

#[derive(Debug, Deserialize, Eq, PartialEq)]
enum Type {
    #[serde(rename = "internal")]
    Internal,
    #[serde(rename = "redirect")]
    Redirect,
    #[serde(rename = "alias")]
    Alias,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            Type::Internal => write!(f, "{}", "Internal"),
            Type::Redirect => write!(f, "{}", "Redirect"),
            Type::Alias => write!(f, "{}", "Alias"),
        }
    }
}

type Command = String;

#[derive(Debug, Deserialize)]
struct CommandMetadata {
    #[serde(rename = "type")]
    command_type: Type,
    description: String,
    url: Option<String>,
    target: Option<String>,
}

pub struct Commands {
    commands: HashMap<Command, CommandMetadata>,
}

pub enum Action {
    Redirect,
    Render,
}

pub struct Execute {
    pub action: Action,
    pub context: Context,
}

impl Commands {
    pub fn load(files: &[String]) -> Result<Commands, String> {
        let mut commands: HashMap<Command, CommandMetadata> = HashMap::new();

        for file in files.iter() {
            let contents = fs::read_to_string(file)
                .map_err(|e| format!("failed to read '{}': {}", file, e.to_string()))?;
            let c: HashMap<Command, CommandMetadata> = toml::from_str(&contents)
                .map_err(|e| format!("failed to parse '{}': {}", file, e.to_string()))?;
            commands.extend(c);
        }

        Ok(Commands { commands })
    }

    pub fn process(&self, command: &str, args: &Vec<&str>) -> Result<Execute, String> {
        match self.commands.get(command) {
            Some(cmd_meta) => match &cmd_meta.command_type {
                Type::Alias => match &cmd_meta.target {
                    Some(tgt) => self.process(tgt, args),
                    None => Err("missing alias target".to_string()),
                },
                Type::Redirect => self.process_redirect(&cmd_meta.url, args),
                Type::Internal => self.process_internal_command(command, args),
            },
            // if the command is not found, if there's a default provided, use that
            None => match self.commands.get("default") {
                Some(_tgt) => {
                    let mut new_args = vec![command];
                    new_args.extend_from_slice(&args);
                    self.process("default", &new_args)
                }
                None => Err(format!("unknown command: '{}'", command)),
            },
        }
    }

    fn process_redirect(
        &self,
        target_url: &Option<String>,
        args: &[&str],
    ) -> Result<Execute, String> {
        let mut context = Context::new();

        match target_url {
            Some(url) => {
                let query = Uri::percent_encode(&args.join(" ")).to_string();
                let redirect_url = match url.contains("{query}") {
                    // replace the placeholder
                    true => url.replace("{query}", &query),
                    // append the query
                    false => format!("{}{}", &url, &query),
                };
                context.insert("redirect_url", &redirect_url);
                Ok(Execute {
                    action: Action::Redirect,
                    context,
                })
            }
            None => Err("redirect command missing target URL".to_string()),
        }
    }

    fn process_internal_command(&self, command: &str, args: &Vec<&str>) -> Result<Execute, String> {
        let mut context = Context::new();

        match command {
            "echo" => {
                context.insert("echo", "true");
                context.insert("args", &args.join(" "));
                Ok(Execute {
                    action: Action::Render,
                    context,
                })
            }
            "list" => {
                // if the command is `list` and there's an argument we want to show the
                // info on that command, otherwise `list` by itself should return the
                // list of all available commands
                match args.len() > 0 {
                    true => {
                        let data = self.list_cmd_data(args[0]);
                        context.insert("command", args[0]);
                        context.insert("data", &data);
                        Ok(Execute {
                            action: Action::Render,
                            context,
                        })
                    }
                    false => {
                        context.insert("commands_list", &self.list_data());
                        Ok(Execute {
                            action: Action::Render,
                            context,
                        })
                    }
                }
            }
            cmd => Err(format!("internal command '{}' not yet implemented.", cmd)),
        }
    }

    fn list_data(&self) -> HashMap<String, HashMap<&str, HashMap<&str, String>>> {
        // group commands by type: type -> (command, -> (property, value))
        let mut data: HashMap<String, HashMap<&str, HashMap<&str, String>>> = HashMap::new();

        // collect data for all commands
        for (cmd, meta) in &self.commands {
            // make sure the current type exists in the map
            let type_key = meta.command_type.to_string();
            if let None = data.get(&type_key) {
                data.insert(type_key.clone(), HashMap::new());
            }
            // insert each command, grouped by type
            match data.get_mut(&type_key) {
                Some(by_type) => {
                    by_type.insert(cmd, self.list_cmd_data(cmd));
                }
                None => {
                    eprintln!("impossible error: hashmap key missing right after being inserted");
                }
            }
        }

        data
    }

    fn list_cmd_data(&self, command: &str) -> HashMap<&str, String> {
        // let mut context = Context::new();
        let mut data = HashMap::new();

        // something like `ls non_existent` would bring us here with `command == non_existent`
        // so we really need to confirm the command does exist
        match self.commands.get(command) {
            Some(meta) => {
                data.insert("type", meta.command_type.to_string());
                data.insert("description", meta.description.clone());
                match &meta.command_type {
                    Type::Redirect => {
                        data.insert("url", meta.url.clone().unwrap_or("unknown url".to_string()))
                    }
                    Type::Alias => data.insert(
                        "target",
                        meta.target.clone().unwrap_or("unknown target".to_string()),
                    ),
                    _ => None,
                };
            }
            None => {
                data.insert("type", "unknown".to_string());
                data.insert("description", "unknown".to_string());
            }
        }

        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_commands_empty() -> Result<(), String> {
        let cmds = Commands::load(&vec![])?;

        assert!(cmds.commands.is_empty());

        Ok(())
    }

    #[test]
    fn test_load_commands_default() -> Result<(), String> {
        let cmds = Commands::load(&vec!["commands.toml".to_string()])?;

        assert!(cmds.commands.len() > 0);

        let cmd = cmds.commands.get("h").unwrap();
        assert_eq!(cmd.command_type, Type::Alias);
        assert_eq!(cmd.target, Some("help".to_string()));

        Ok(())
    }

    #[test]
    fn test_load_commands_multiple() -> Result<(), String> {
        let cmds = Commands::load(&vec![
            "commands.toml".to_string(),
            "test_commands.toml".to_string(),
        ])?;

        assert!(cmds.commands.len() > 0);

        // from the default commands
        let cmd = cmds.commands.get("echo").unwrap();
        assert_eq!(cmd.command_type, Type::Internal);
        // from the test commands
        let cmd1 = cmds.commands.get("test_internal").unwrap();
        assert_eq!(cmd1.command_type, Type::Internal);
        let cmd2 = cmds.commands.get("test_redirect").unwrap();
        assert_eq!(cmd2.command_type, Type::Redirect);

        Ok(())
    }

    #[test]
    fn test_redirect_simple() -> Result<(), String> {
        let cmds = Commands::load(&vec!["test_commands.toml".to_string()])?;

        let query = "foo bar";
        let target_simple = "http://some.url.tld/search?q=";
        let url_simple = "http://some.url.tld/search?q=foo%20bar";

        let exec = cmds.process_redirect(&Some(target_simple.to_string()), &vec![query])?;
        assert_eq!(
            exec.context.get("redirect_url").unwrap().as_str().unwrap(),
            url_simple
        );

        Ok(())
    }

    #[test]
    fn test_redirect_fmt() -> Result<(), String> {
        let cmds = Commands::load(&vec!["test_commands.toml".to_string()])?;

        let query = "bar baz";
        let target_fmt = "http://some.url.tld/search?q=\"{query}\"";
        let url_fmt = "http://some.url.tld/search?q=\"bar%20baz\"";

        let exec = cmds.process_redirect(&Some(target_fmt.to_string()), &vec![query])?;
        assert_eq!(
            exec.context.get("redirect_url").unwrap().as_str().unwrap(),
            url_fmt
        );

        Ok(())
    }
}
