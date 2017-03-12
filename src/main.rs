#![feature(plugin)]
#![plugin(rocket_codegen)]
#[macro_use]
extern crate error_chain;
extern crate reqwest;
extern crate rocket;
extern crate semver;
extern crate tempdir;
extern crate toml;

use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::process;

use rocket::request::FromParam;
use semver::Version;
use tempdir::TempDir;

mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types
    error_chain! { }
}

use errors::ResultExt;

#[derive(Eq, PartialEq, PartialOrd, Ord)]
enum Status {
    OutOfDate,
    UpToDate,
    Unknown,
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Status::Unknown => write!(f, "unknown"),
            Status::OutOfDate => write!(f, "outofdate"),
            Status::UpToDate => write!(f, "uptodate"),
        }
    }
}

struct MyParam<'r> {
    deps_type: &'r str,
    ext: &'r str,
}

impl<'r> FromParam<'r> for MyParam<'r> {
    type Error = &'r str;

    fn from_param(param: &'r str) -> Result<MyParam<'r>, &'r str> {
        let (status_type, ext) = match param.find('.') {
            Some(i) if i > 0 => (&param[..i], &param[(i + 1)..]),
            _ => return Err(param),
        };

        Ok(MyParam {
            deps_type: match status_type {
                "dev-status" => "dev-dependencies",
                "status" => "dependencies",
                _ => return Err(param),
            },
            ext: match ext {
                "png" => "png",
                "svg" => "svg",
                _ => return Err(param),
            },
        })
    }
}


#[get("/<owner>/<name>/<params>")]
fn index(owner: &str, name: &str, params: MyParam) -> io::Result<File> {
    // TODO: HEADER 'Cache-Control': 'no-cache, no-store, must-revalidate',
    // TODO: HEADER 'Expires': new Date().toUTCString()
    let status = match get_deps_status(owner, name, params.deps_type) {
        Ok(status) => status,
        Err(ref e) => {
            println!("error: {}", e);

            for e in e.iter().skip(1) {
                println!("caused by: {}", e);
            }

            // The backtrace is not always generated. Try to run this example
            // with `RUST_BACKTRACE=1`.
            if let Some(backtrace) = e.backtrace() {
                println!("backtrace: {:?}", backtrace);
            }
            Status::Unknown
        }
    };
    File::open(format!("public/img/status/{}.{}", status, params.ext))
}

fn get_deps_status(owner: &str, name: &str, deps_type: &str) -> errors::Result<Status> {
    let manifest_url = format!("https://raw.githubusercontent.com/{}/{}/master/Cargo.toml",
                            owner,
                            name);

    let mut resp = reqwest::get(manifest_url.as_str())
        .chain_err(|| "Unable to download Cargo.toml")?;

    let mut buffer = String::new();
    match resp.status() {
        &reqwest::StatusCode::Ok => {
            resp.read_to_string(&mut buffer)
                .chain_err(|| "Unable to read Cargo.toml body")?;
            deps_status_from_cargo(owner, name, buffer, deps_type)
                .chain_err(|| "Unable verify status")
        },
        _ => {
            bail!("Bad status code retreiving Cargo.toml");
        }
    }
}

fn deps_status_from_cargo(owner: &str, name: &str, manifest: String, deps_type: &str) -> errors::Result<Status> {

    let root = manifest.as_str()
        .parse::<toml::Value>()
        .chain_err(|| "Unable to parse manifest")?;

    let dependencies = match root.get(deps_type)
        .and_then(|val| val.as_table()) {
        Some(dependencies) => dependencies,
        None => return Ok(Status::UpToDate),
    };

    // 1- Download the Cargo.toml of the project into /tmp/owner/name/Cargo.toml
    // 2- Create a dummy /tmp/owner/name/src/lib.rs (avoid `cargo update` complaint)
    let tmp_dir = TempDir::new(format!("{}_{}", owner, name).as_str())
        .chain_err(|| "Unable to create tmp dir")?;
    let tmp_manifest = tmp_dir.path().join("Cargo.toml");
    let tmp_lockfile = tmp_dir.path().join("Cargo.lock");
    let tmp_src_dir = tmp_dir.path().join("src");
    let tmp_src_lib = tmp_src_dir.join("lib.rs");

    File::create(tmp_manifest.clone())
        .and_then(|mut file| file.write_all(manifest.as_bytes()))
        .and_then(|_| fs::create_dir(tmp_src_dir))
        .and_then(|_| File::create(tmp_src_lib))
        .chain_err(|| "Unable to create tmp file structure")?;

    // 3- `cargo update --manifest-path /tmp/owner/name/Cargo.toml`
    let output = process::Command::new("cargo")
        .arg("update")
        .arg("--manifest-path")
        .arg(tmp_manifest)
        .output()
        .chain_err(|| "Unable to exec cargo update")?;

    // 4- Parse the /tmp/owner/name/Cargo.lock generated
    let mut buffer = String::new();

    File::open(tmp_lockfile)
        .and_then(|mut f| f.read_to_string(&mut buffer))
        .chain_err(|| "Unable to read Cargo.lock")?;

    let tmp_root_lockfile = buffer.as_str()
        .parse::<toml::Value>()
        .chain_err(|| "Unable to parse Cargo.lock")?;

    let tmp_root_table = match tmp_root_lockfile.get("root") {
        Some(root) => root,
        None => bail!("Unable to find root in lockfile"),
    };

    let updated_raw_deps = match tmp_root_table.get("dependencies") {
        Some(&toml::Value::Array(ref raw_deps)) => raw_deps,
        Some(_) => unreachable!(),
        None => bail!("Unable to find dependencies in lockfile"),
    };

    let mut updated_deps = HashMap::new();
    for updated_raw_dep in updated_raw_deps {
        let raw_dep_vec: Vec<_> = updated_raw_dep.as_str().unwrap_or("").split(' ').collect();
        if raw_dep_vec.len() < 2 {
            bail!("Invalid dependency found");
        }
        updated_deps.insert(raw_dep_vec[0], raw_dep_vec[1]);
    }

    // 5- Compare each deps with semver
    let status = dependencies.iter().fold(Status::UpToDate, |oldest, (dep, version_value)| {
        let updated_version = match updated_deps.get::<str>(&dep.to_string()) {
            Some(updated_version) => updated_version,
            None => unreachable!(),
        };
        let version = match version_value.as_str() {
            Some(version) => version,
            None => return oldest,
        };

        if Version::parse(updated_version) > Version::parse(version) {
            println!("{} is outdated", dep);
            println!("Specified: {}", version);
            println!("Latest: {}", updated_version);
            println!("");
            Status::OutOfDate
        } else if Status::OutOfDate == oldest {
            Status::OutOfDate
        } else {
            Status::UpToDate
        }
    });

    Ok(status)
}

fn main() {
    rocket::ignite().mount("/", routes![index]).launch();
}
