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
use std::collections::HashMap;

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

    let root = match toml::Parser::new(&*cargo).parse() {
        Some(root) => root,
        None => return Status::Unknown
    };

    let dependencies = match root.get(deps_type)
        .and_then(|val| val.as_table()) {
            Some(dependencies) => dependencies,
            None => return Status::UpToDate
        };

    // TODO:
    // 1- Download the Cargo.toml of the project into /tmp/owner/name/Cargo.toml
    // 2- Create a dummy /tmp/owner/name/src/lib.rs (avoid `cargo update` complaint)
    let tmp_dir = format!("/tmp/{}/{}", owner, name);
    let tmp_manifest = format!("{}/Cargo.toml", tmp_dir);
    let tmp_lockfile = format!("{}/Cargo.lock", tmp_dir);
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
    let mut buffer = String::new();
    if let Err(_) = File::open(tmp_lockfile)
        .and_then(|mut f| f.read_to_string(&mut buffer)) {
            return Status::Unknown;
        }

    //let updated_raw_deps = match toml::Parser::new(buffer.as_str()).parse()
    //    .and_then(|cargo_lockfile | cargo_lockfile.get("root"))
    //    .and_then(|root| root.lookup("dependencies")) {
    //        Some(&toml::Value::Array(ref raw_deps)) => raw_deps,
    //        Some(_) => unreachable!(),
    //        None => return Status::Unknown
    //    };

    let tmp_root_lockfile = match toml::Parser::new(buffer.as_str()).parse() {
        Some(root) => root,
        None => return Status::Unknown
    };

    let tmp_root_table = match tmp_root_lockfile.get("root") {
        Some(root) => root,
        None => return Status::Unknown
    };

    let updated_raw_deps = match tmp_root_table.lookup("dependencies") {
        Some(&toml::Value::Array(ref raw_deps)) => raw_deps,
        Some(_) => unreachable!(),
        None => return Status::Unknown
    };

    let mut updated_deps = HashMap::new();
    for updated_raw_dep in updated_raw_deps {
         let raw_dep_vec : Vec<_>= updated_raw_dep.as_str().unwrap_or("").split(' ').collect();
         if raw_dep_vec.len() < 2 {
             return Status::Unknown
         }
         updated_deps.insert(raw_dep_vec[0], raw_dep_vec[1]);
    }

    println!("{:?}", updated_deps);

    // 5- Compare each deps with semver
    dependencies.iter().fold(Status::UpToDate, |oldest, (dep, version)| {
        println!("{:?}", dep);
        println!("{:?}", version);
        Status::OutOfDate
    })
}

fn main() {
    rocket::ignite().mount("/", routes![index]).launch();
}
