# rrc - Manage remote repository clones

[![crates.io](https://img.shields.io/crates/v/rrc.svg)](https://crates.io/crates/rrc)

`rrc` is a remote repository management tool like [ghq][1] written in Rust.

`rrc` provides a way to organize remote repository clones, like go get does.

`rrc` makes a directory under a specific root directory (by default ~/repos) using the remote repository URL’s host and path.

## Installation

```shell
$ cargo install rrc
```

## Usage

`rrc` command is almost compatible with [ghq][1].

```
rrc
A manage remote repository clones

USAGE:
    rrc [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --config <FILE>    Set config file

SUBCOMMANDS:
    each      Execute command for each local repositories
    get       Clone remote repository
    help      Prints this message or the help of the given subcommand(s)
    list      List local repositories
    look      Look local repository
    remove    Remove local repositories
    update    Update local repositories
```

For how to use this tool, [ghq-handbook][2] will be helpful.

## Config

`rrc` provides a simple toml-style configuration file.

The configuration file can set profile name in the section. You can then select a profile with command line options. And you can also set host filters. If you set a host filter, it will be enabled across profiles.

```toml
# default profile
[default]
# customize repo root path
root = "~/repos"

# personal profile
[personal]
# customize repo root path
root = "~/personal_repos"
# hosts filter. gitlab repository cloned '~/personal_repos'
hosts = ["gitlab.com"]

```


[1]: https://github.com/motemen/ghq
[2]: https://github.com/Songmu/ghq-handbook
