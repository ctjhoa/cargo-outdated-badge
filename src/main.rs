#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate reqwest;
extern crate rocket;
extern crate toml;

use std::fmt::{Display, Formatter, Error};
use std::fs::File;
use std::io::Read;

enum Status {
    Unknown,
    OutOfDate,
    UpToDate
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match *self {
            Status::Unknown => write!(f, "unknown"),
            Status::OutOfDate => write!(f, "outofdate"),
            Status::UpToDate => write!(f, "uptodate")
        }
    }
}

#[get("/<owner>/<name>")]
fn index(owner: &str, name: &str) -> File {
    // TODO: HEADER 'Cache-Control': 'no-cache, no-store, must-revalidate',
    // TODO: HEADER 'Expires': new Date().toUTCString()
    let status = get_deps_status(owner, name, "dev-dependencies");
    File::open(format!("public/img/status/{}.png", status)).unwrap()
}

fn get_deps_status(owner: &str, name: &str, deps_type: &str) -> Status {
    let cargo_url = format!("https://raw.githubusercontent.com/{}/{}/master/Cargo.toml", owner, name);

    if let Ok(mut resp) = reqwest::get(&*cargo_url) {
        match resp.status() {
            &reqwest::StatusCode::Ok => {
                let mut body = String::new();
                resp.read_to_string(&mut body).ok();
                deps_status_from_cargo(body, deps_type)
            },
            _ => Status::Unknown
        }
    } else {
        Status::Unknown
    }
}

fn deps_status_from_cargo(cargo: String, deps_type: &str) -> Status {
    if let Some(root) = toml::Parser::new(&*cargo).parse() {
        match root.get(deps_type) {
            Some(val) => {
                if let Some(dependencies) = val.as_table() {
                    dependencies.iter().fold(Status::UpToDate, |oldest, (dep, version)| {
                        println!("{:?}", dep);
                        println!("{:?}", version);
                        Status::OutOfDate
                    })
                } else {
                    Status::UpToDate
                }
            },
            None => Status::UpToDate
        }
    } else {
        Status::Unknown
    }
}

fn main() {
    rocket::ignite().mount("/", routes![index]).launch();
}
