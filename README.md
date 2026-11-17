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
A manage remote repository clones

Usage: rrc [OPTIONS] <COMMAND>

Commands:
  get     Clone remote repository
  list    List local repositories
  update  Update local repositories
  look    Look local repository
  remove  Remove local repositories
  each    Execute command for each local repositories
  help    Print this message or the help of the given subcommand(s)

Options:
  -c, --config <FILE>  Set config file [default: $HOME/rrc.toml]
  -h, --help           Print help
  -V, --version        Print version
```

Repositories can be given as `https://host/user/repo`, `ssh://...`,
scp-like `[user@]host:user/repo`, `host/user/repo` or `user/repo` (GitHub).

`update` and `each` run in parallel and exit with an error if any repository
failed. The output of `update` is printed as one block per repository.
`remove` asks for confirmation for each repository (default: no).

The config file can also be set with `RRC_CONFIG`, and the default root with `RRC_ROOT`.

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
