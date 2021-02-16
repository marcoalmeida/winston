#![feature(proc_macro_hygiene, decl_macro)]

use std::io::Cursor;
use std::{io, process};

use lazy_static::lazy_static;
use rocket;
use rocket::http::Status;
use rocket::response::Response;
use rocket::State;
use rocket_contrib::serve::StaticFiles;
use tera::Context;
use tera::Tera;

mod commands;
use commands::{Action, Commands};

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd)]
struct ParsedQS<'a> {
    cmd: &'a str,
    args: Vec<&'a str>,
}

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let tera = match Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/html/*.tera")) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Failed to parse templates: {}", e);
                process::exit(1);
            }
        };

        tera
    };
}

fn main() -> Result<(), String> {
    // load the list of available commands and make it available as state to the handlers
    let cmds = Commands::load()?;

    let e = rocket::ignite()
        .manage(cmds)
        .mount("/", rocket::routes![index])
        .mount(
            "/static",
            StaticFiles::from(concat!(env!("CARGO_MANIFEST_DIR"), "/html")),
        )
        .launch();

    // if we make it this far something bad happened
    Err(format!("Failed to start Rocket: {}", e.to_string()))
}

#[rocket::get("/?<q>")]
fn index<'a>(q: String, commands: State<Commands>) -> rocket::Response<'a> {
    let parsed_qs = parse_query_string(&q);

    match parsed_qs {
        Ok(qs) => match commands.process(qs.cmd, &qs.args) {
            Ok(result) => match result.action {
                Action::Redirect => redirect(&result.context),
                Action::Render => render(&result.context),
            },
            Err(e) => error(Status::BadRequest, Cursor::new(e)),
        },
        Err(e) => error(Status::BadRequest, Cursor::new(e)),
    }
}

fn redirect<'a>(context: &Context) -> rocket::Response<'a> {
    match context.get("redirect_url") {
        Some(url) => match url.as_str() {
            Some(u) => Response::build()
                .status(Status::TemporaryRedirect)
                .raw_header("Location", u.to_string())
                .finalize(),
            None => error(
                Status::InternalServerError,
                Cursor::new("malformed redirect URL"),
            ),
        },
        None => error(
            Status::InternalServerError,
            Cursor::new("missing redirect URL"),
        ),
    }
}

fn render<'a>(context: &Context) -> rocket::Response<'a> {
    let mut response = Response::build();

    match TEMPLATES.render("index.html.tera", context) {
        Ok(html) => response.status(Status::Ok).sized_body(Cursor::new(html)),
        Err(e) => response
            .status(Status::InternalServerError)
            .sized_body(Cursor::new(e.to_string())),
    };

    response.finalize()
}

fn error<'a, B>(status: Status, body: B) -> rocket::Response<'a>
where
    B: io::Read + io::Seek + 'a,
{
    Response::build().status(status).sized_body(body).finalize()
}

fn parse_query_string(qs: &str) -> Result<ParsedQS, String> {
    let mut args: Vec<&str> = qs.split_whitespace().collect();

    if args.len() < 1 {
        return Err("no command provided".to_string());
    }

    // save the command and remove it from the list of args
    let cmd = args[0];
    args.remove(0);

    Ok(ParsedQS { cmd, args })
}

#[cfg(test)]
mod tests {
    use crate::{parse_query_string, ParsedQS};

    #[test]
    fn parse_query_string_empty() {
        assert!(parse_query_string("").is_err());
    }

    #[test]
    fn parse_query_string_no_args() {
        assert_eq!(
            parse_query_string("ls"),
            Ok(ParsedQS {
                cmd: "ls",
                args: vec![]
            })
        );
    }

    #[test]
    fn parse_query_string_and_args() {
        let cmd = ParsedQS {
            cmd: "ls",
            args: vec!["cmd"],
        };

        for str in vec!["ls cmd", " ls   cmd  "] {
            assert_eq!(parse_query_string(str), Ok(cmd.clone()));
        }
    }
}
