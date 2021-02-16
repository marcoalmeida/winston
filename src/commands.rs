use std::collections::HashMap;
use std::fmt;
use std::fs;

use rocket::http::uri::Uri;
use serde::Deserialize;
use tera::Context;

static COMMANDS_FILE: &'static str = "commands.toml";

#[derive(Debug, Deserialize)]
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
    pub fn load() -> Result<Commands, String> {
        let contents = fs::read_to_string(COMMANDS_FILE).map_err(|e| e.to_string())?;
        let commands: HashMap<Command, CommandMetadata> =
            toml::from_str(&contents).map_err(|e| e.to_string())?;

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
        args: &Vec<&str>,
    ) -> Result<Execute, String> {
        let mut context = Context::new();

        match target_url {
            Some(url) => {
                let redirect_url = format!(
                    "{}{}",
                    url,
                    Uri::percent_encode(&args.join(" ")).to_string()
                );
                context.insert("redirect_url", &redirect_url);
                Ok(Execute {
                    action: Action::Redirect,
                    context: context,
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
