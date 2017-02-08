#![feature(plugin)]
#![plugin(rocket_codegen)]

#[macro_use]
extern crate error_chain;

extern crate reqwest;
extern crate rocket;
extern crate toml;
extern crate semver;

use std::collections::HashMap;
use std::fmt::{Display, Formatter, self};
use std::fs::{File, self};
use std::io::{Read, Write, self};
use std::process;

use rocket::request::FromParam;
use semver::Version;

mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types
    error_chain! { }
}

use errors::ResultExt;

#[derive(Eq, PartialEq, PartialOrd, Ord)]
enum Status {
    OutOfDate,
    UpToDate,
    Unknown
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Status::Unknown => write!(f, "unknown"),
            Status::OutOfDate => write!(f, "outofdate"),
            Status::UpToDate => write!(f, "uptodate")
        }
    }
}

enum Provider {
    Github
}

struct Repository {
    owner: &'static str,
    name: &'static str,
    provider: Provider
}

struct MyParam<'r> {
    deps_type: &'r str,
    ext: &'r str
}

impl<'r> FromParam<'r> for MyParam<'r> {
    type Error = &'r str;

    fn from_param(param: &'r str) -> Result<MyParam<'r>, &'r str> {
        let (status_type, ext) = match param.find('.') {
            Some(i) if i > 0 => (&param[..i], &param[(i + 1)..]),
            _ => return Err(param)
        };

        Ok(MyParam {
            deps_type: match status_type {
                "dev-status" => "dev-dependencies",
                "status" => "dependencies",
                _ => return Err(param)
            },
            ext: match ext {
                "png" => "png",
                "svg" => "svg",
                _ => return Err(param)
			      },
		    })
	  }
}

#[get("/<owner>/<name>/<params>")]
fn index(owner: &str, name: &str, params: MyParam) -> io::Result<File> {
    // TODO: HEADER 'Cache-Control': 'no-cache, no-store, must-revalidate',
    // TODO: HEADER 'Expires': new Date().toUTCString()
    let status = get_deps_status(owner, name, params.deps_type);
    File::open(format!("public/img/status/{}.{}", status, params.ext))
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
    dependencies.iter().fold(Status::UpToDate, |oldest, (dep, version_value)| {
        let updated_version = match updated_deps.get::<str>(&dep.to_string()) {
            Some(updated_version) => updated_version,
            None => unreachable!()
        };
        let version = match version_value.as_str() {
            Some(version) => version,
            None => return Status::Unknown
        };
        println!("{:?}", dep);
        println!("{:?}", version);

        if Version::parse(updated_version) >  Version::parse(version) {
            Status::OutOfDate
        } else if Status::OutOfDate == oldest {
            Status::OutOfDate
        } else {
            Status::UpToDate
        }
    })
}

fn recursive_get_status() -> Status {
    let mut status = Status::Unknown;
    // for each sub_dependencies
    //   let sub_status = recursive_get_status();
    //   if sub_status < status
    //     status = sub_status;
    // create file structure
    // let my_status = get_status()
    // if my_status < status
    //   status = my_status;
    status
}


fn get_recursive_status(repo: &Repository, dependencies: &toml::Table) -> Status {
    Status::Unknown
}

fn get_flat_status(repo: &Repository, dependencies: &toml::Table) -> Status {
    if let Err(_) = build_file_structure(repo, None) {
        return Status::Unknown;
    }
    Status::Unknown
}

fn build_file_structure<T: Into<Option<&'static str>>>(repo: &Repository, deps_prefix: T) -> errors::Result<()> {
    let cargo_url = match repo.provider {
        Provider::Github => format!("https://raw.githubusercontent.com/{}/{}/master/{}Cargo.toml", repo.owner, repo.name, deps_prefix.into().unwrap_or(""))
    };

    // 1- Download the Cargo.toml of the project into /tmp/owner/name/Cargo.toml
    let mut resp = reqwest::get(&*cargo_url)
        .chain_err(|| "unable to download Cargo.toml")?;
    let mut cargo = String::new();
    match resp.status() {
        &reqwest::StatusCode::Ok => {
            resp.read_to_string(&mut cargo)
                .chain_err(|| "unable to read Cargo.toml body")?;
        },
        _ => {
            bail!("bad status code retreiving Cargo.toml");
        }
    }

    // 2- Create a dummy /tmp/owner/name/src/lib.rs (avoid `cargo update` complaint)
    let tmp_dir = format!("/tmp/{}/{}", repo.owner, repo.name);
    let tmp_manifest = format!("{}/Cargo.toml", tmp_dir);
    let tmp_lockfile = format!("{}/Cargo.lock", tmp_dir);
    let tmp_src_dir = format!("{}/src", tmp_dir);
    let tmp_src_lib = format!("{}/lib.rs", tmp_src_dir);

    fs::create_dir_all(tmp_src_dir.as_str())
        .and_then(|_| File::create(tmp_manifest.as_str()))
        .and_then(|mut file| file.write_all(cargo.as_bytes()))
        .and_then(|_| File::create(tmp_src_lib.as_str()))
        .chain_err(|| "unable to create tmp file structure")?;

    // 3- `cargo update --manifest-path /tmp/owner/name/Cargo.toml`
    process::Command::new("cargo")
        .arg("update")
        .arg("--manifest-path")
        .arg(tmp_manifest.as_str())
        .output()
        .chain_err(|| "unable to exec cargo update")?;

    // 4- Parse the /tmp/owner/name/Cargo.lock generated
    let mut buffer = String::new();
    File::open(tmp_lockfile)
        .and_then(|mut f| f.read_to_string(&mut buffer))
        .chain_err(|| "unable to read Cargo.lock")?;
    Ok(())
}

fn main() {
    rocket::ignite().mount("/", routes![index]).launch();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::fs;

    #[test]
    fn build_flat_structure() {
        fs::remove_dir_all("/tmp/BurntSushi");
        let repo = Repository { owner: "BurntSushi", name: "ripgrep", provider: Provider::Github };
        build_file_structure(&repo, None);
        assert_eq!(Path::new("/tmp/BurntSushi/ripgrep").exists(), true);
    }

    //#[test]
    //fn build_recursive_structure() {
    //    fs::remove_dir_all("/tmp/foo");
    //    let repo = Repository { owner: "foo", name: "bar", provider: Provider::Github };
    //    build_file_structure(&repo, "dep");
    //    assert_eq!(Path::new("/tmp/foo/bar/dep").exists(), true);
    //}
}
