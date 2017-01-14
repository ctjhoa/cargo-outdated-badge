#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate reqwest;
extern crate rocket;
extern crate toml;

use std::fmt::{Display, Formatter, self};
use std::fs::{File, self};
use std::io::Read;
use std::io::Write;
use std::process;

enum Status {
    Unknown,
    OutOfDate,
    UpToDate
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
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
                deps_status_from_cargo(owner, name, body, deps_type)
            },
            _ => Status::Unknown
        }
    } else {
        Status::Unknown
    }
}

fn deps_status_from_cargo(owner: &str, name: &str, cargo: String, deps_type: &str) -> Status {

    if let Some(root) = toml::Parser::new(&*cargo).parse() {
        match root.get(deps_type) {
            Some(val) => {
                if let Some(dependencies) = val.as_table() {
                    // TODO:
                    // 1- Download the Cargo.toml of the project into /tmp/owner/name/Cargo.toml
                    // 2- Create a dummy /tmp/owner/name/src/lib.rs (avoid `cargo update` complaint)
                    let tmp_dir = format!("/tmp/{}/{}", owner, name);
                    let tmp_manifest = format!("{}/Cargo.toml", tmp_dir);
                    let tmp_src_dir = format!("{}/src", tmp_dir);
                    let tmp_src_lib = format!("{}/lib.rs", tmp_src_dir);

                    if let Err(_) = fs::create_dir_all(tmp_src_dir.as_str())
                        .and_then(|_| File::create(tmp_manifest.as_str()))
                        .and_then(|mut file| file.write_all(cargo.as_bytes()))
                        .and_then(|_| File::create(tmp_src_lib.as_str())) {
                            return Status::Unknown;
                        }
                    // 3- `cargo update --manifest-path /tmp/owner/name/Cargo.toml`
                    if let Err(_) = process::Command::new("cargo")
                        .arg("update")
                        .arg("--manifest-path")
                        .arg(tmp_manifest.as_str())
                        .output() {
                            return Status::Unknown;
                        }
                    // 4- Parse the /tmp/owner/name/Cargo.lock generated
                    // 5- Compare each deps with semver
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
