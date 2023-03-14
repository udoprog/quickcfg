# quickcfg

[<img alt="github" src="https://img.shields.io/badge/github-udoprog/quickcfg-8da0cb?style=for-the-badge&logo=github" height="20">](https://github.com/udoprog/quickcfg)
[<img alt="crates.io" src="https://img.shields.io/crates/v/quickcfg.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/quickcfg)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-quickcfg-66c2a5?style=for-the-badge&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">](https://docs.rs/quickcfg)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/udoprog/quickcfg/ci.yml?branch=main&style=for-the-badge" height="20">](https://github.com/udoprog/quickcfg/actions?query=branch%3Amain)

Apply a base configuration to a system, quickly!

It reads a configuration and template structure from a [dotfiles] directory and tries to normalize
the machine that it is run base on this configuration.

**WARNING**:
This project is currently in development.
I've tried my best to make all operations non-destructive, but beware of bugs!

[dotfiles]: https://github.com/udoprog/dotfiles

![Example](gfx/example1.gif)

<br>

## Getting started

To get started, you can have quickcfg clone the configuration for you in the default location:

```bash
qc --init <git-url>
```

This will put the checked out configuration in the default config location for you platform.
For example:

* Windows - `%APPDATA%\quickcfg`
* Linux - `$HOME/.config/quickcfg`

To find out where the various quickcfg directories are, use:

```bash
qc --paths
```

<br>

## Features

**Zero dependencies**, All you need is the `quickcfg` binary and your configuration repo.

**Blazingly fast**, multi-threaded and uses a simple dependency graph to determine when things can
run in parallel.

**Flexible but opinionated manifests**, There are a couple of powerful primitives available
(e.g. `copy-dir`), which does _a lot_ of work with very little configuration.

**Uses fast checksumming**, to reduce the amount of unnecessary work. Only applies changes when it
has to.

<br>

## Automatically applying updates

If you want quickcfg to periodically check your git repositories for updates, you can add the
following to your `.zshrc` or `.bashrc`:

```bash
if command -v qc > /dev/null 2>&1; then
    qc --updates-only
    alias upd="qc"
fi
```

Every time you open a shell quickcfg will not check if your dotfiles are up-to-date.

You control how frequently by setting the `git_refresh` option in `quickcfg.yml`:

```no_test
git_refresh: 3d
```

<br>

## Configuration

Create a repository with a `quickcfg.yml` in its root:

```no_test
git_refresh: 1d

hierarchy:
  - secrets.yml
  - db/common.yml
  - db/{distro}.yml

systems:
  # System to ensure that a set of packages are installed.
  - type: install
```

You also want to add a `.gitignore` file that looks like this:

```gitignore
/secrets.yml
/.state.yml
/.state
```

Then populate `secrets.yml` with your secret information - this you **DO NOT** check into git.
Any variables you put in here can be used in future templates since they are part of the
hierarchy.

The [`hierarchy`] specifies a set of files that should be looked for.
These can use variables like `{distro}`, which will be expanded based on the facts known of the
system you are running on.

You can use my [dotfiles](https://github.com/udoprog/dotfiles) repository as inspiration.

The following section will detail all the systems which are available.

[`hierarchy`]: #hierarchy

<br>

## Hierarchy

The hierarchy is a collection of files which contain data.

Some systems query the hierarchy for information, like the `key` setting in [`install`].
This then determines which packages should be installed.

Hierarchy variables can also be made available in [`templates`] by adding a `quickcfg:` tag at the
top of the template.

[`install`]: #install
[`templates`]: #templating

<br>

## Systems

<br>

#### `copy-dir`

Copies a directory recursively.

```yaml
type: copy-dir
from: ./some/dir
to: home://some/dir
templates: false
```

Will copy a directory recursively.

<br>

#### `link-dir`

Links a directory recursively.

```yaml
type: link-dir
# Directory to link from.
from: ./some/dir
# Directory to link towards.
to: home://some/dir
```

Will create the corresponding directory structure, but all files will be symbolic links.

<br>

#### `git-sync`

System that syncs a single git repository to some path.

```yaml
type: git-sync
# Where to clone.
path: home://.oh-my-zsh
# Remote to clone.
remote: https://github.com/robbyrussell/oh-my-zsh.git
# Refresh once per day.
refresh: 1d
```

<br>

#### `install`

Compares the set of installed packages, with a set of packages from the hierarchy to install and
installs any that are missing.

Will use `sudo` if needed to install packages.

```yaml
type: install
# The provider of the package manager to use.
provider: pip3
# Hierarchy key to lookup for packages to install.
key: pip3::packages
```

The simplest example of this system is the one that uses the primary provider:

```yaml
systems:
  - type: install
```

This will look up packages under the `packages` key and install it using the primary provider for
the system that you are currently running.

These are the supported providers:

 * `debian`: For Debian-based systems. This is a _primary_ provider.
 * `pip`: The Python 2 package manager.
 * `pip3`: The Python 3 package manager.
 * `gem`: The Ruby package manager.
 * `cargo`: Install packages using `cargo`.
 * `rust components`: Rust components using `rustup`.
   * Key: `rust::components`
 * `rust toolchains`: Rust toolchains using `rustup`.
   * Key: `rust::toolchains`

By default, any _primary_ provider will be the default provider of the system if it can be
detected.

Explicitly configured providers look up packages based on the hierarchy key `<provider>::packages`.
Default providers use the key `packages`.

<br>

#### `download`

Downloads a single file into the given target path.

```yaml
type: download
id: plug-vim
# Url to download the command from.
url: https://raw.githubusercontent.com/junegunn/vim-plug/master/plug.vim
# Path to download to.
path: home://.config/nvim/autoload/plug.vim
```

The `id` is to uniquely identify that this system has only been run once.

<br>

#### `download-and-run`

Downloads a script of the internet and runs it once.

```yaml
type: download-and-run
id: install-oh-my-zsh
# Url to download the command from.
url: https://raw.githubusercontent.com/robbyrussell/oh-my-zsh/master/tools/install.sh
# Set to `true` if the downloaded command requires interaction. (default: false)
interactive: true
# Set to `true` if the command must be run through a shell (`/bin/sh`). (default: false).
shell: true
```

The `id` is to uniquely identify that this system has only been run once.

<br>

#### `link`

Creates a symlink.

```yaml
type: link
path: home://.vimrc
link: .vim/vimrc
```

This creates a symbolic link at `path` which contains whatever is specified in `link`.

<br>

#### `only-for`

Limit a set of systems based on a condition.

```yaml
type: only-for
os: windows
systems:
  # Download and install Rust
  - type: download-and-run
    name: rustup-init.exe
    id: install-rust
    url: https://win.rustup.rs/x86_64
    args: ["-y"]
```

<br>

## Templating

Some systems treats files as templates, like [`copy-dir`] when the `templating` option is enabled.
Any file being copied is then treated as a [`handlebars`] template.

Any template file can make use of hierarchy data, by specifying their dependencies using
a `quickcfg:` tag at the top of the file, like this:

```no_test
# quickcfg: name, hobbies:array

Hi, my name is {{name}}

My hobbies are:
{{#each hobbies}}
- {{this}}
{{/#each}}
```

This will load the `name` and `hobbies` variables out of the [`hierarchy`].
`hobbies` will be loaded as an array, causing all values in the hierarchy for that value to be
loaded.

[`copy-dir`]: #copy-dir
[`handlebars`]: https://handlebarsjs.com/
