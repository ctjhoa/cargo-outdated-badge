<p align="center">
<a href="#"><img src="http://34.249.40.139/ctjhoa/cargo-outdated-badge/status.svg" alt="Dependency Status"></a>
<a href="#"><img src="http://34.249.40.139/ctjhoa/cargo-outdated-badge/dev-status.svg" alt="devDependency Status"></a>
</p>

# cargo-outdated-badge
It's an experimental tool that tells you when your cargo dependencies are out of date. (Similar to https://david-dm.org/ for npm)
You can get dependencies and development dependencies statuses in `svg` or `png` format.

## Usage

`/owner/repository/status.format`

- `owner`: github's username
- `repository`: github's repository name
- `status`: it could be `status` or `dev-status` to check main dependencies or development dependencies
- `format`: it could be `svg` or `png`

For example, to check development dependencies on this repository in svg, use this url:

http://34.249.40.139/ctjhoa/cargo-outdated-badge/dev-status.svg

### Demo
I've created an demo server on AWS at this address: http://34.249.40.139/

You can use it as you want but I cannot guarantee the availability of the service.

### Run locally
This tool relies on [rocket webserver](https://github.com/SergioBenitez/Rocket) so you can start the server with:
```
$ cargo +nightly run
```
The server is started at http://localhost:8000/

## Limitations
This project does not currently support:
- [build dependencies](http://doc.crates.io/specifying-dependencies.html#build-dependencies)
- [git dependencies](http://doc.crates.io/specifying-dependencies.html#specifying-dependencies-from-git-repositories)
- [path dependencies](http://doc.crates.io/specifying-dependencies.html#specifying-path-dependencies)
- Other branch than `master`

## Thanks
- [SergioBenitez/Rocket](https://github.com/SergioBenitez/Rocket)
- [alanshaw/david](https://github.com/alanshaw/david)
- [alexcrichton/toml-rs](https://github.com/alexcrichton/toml-rs)
- [brson/error-chain](https://github.com/brson/error-chain)
- [kbknapp/cargo-outdated](https://github.com/kbknapp/cargo-outdated)
- [steveklabnik/semver](https://github.com/steveklabnik/semver)
