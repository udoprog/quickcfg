# quickcfg
[![Build Status](https://travis-ci.org/udoprog/quickcfg.svg?branch=master)](https://travis-ci.org/udoprog/quickcfg)

Apply a base configuration to a system, quickly!

It reads a configuration and template structure from a [dotfiles] directory and tries to normalize
the machine that it is run base on this configuration.

Until Rust Edition 2018 is released, this crate is _Nightly Only_.

**WARNING**:
This will modify your system and potentially overwrite files!
Make sure you have backed everything up before using it!

[dotfiles]: https://github.com/udoprog/dotfiles

## Features

* Zero dependencies! All you need is the `quickcfg` binary and your configuration repo.
* Blazingly fast! We will normalize your machine and keep the configuration in sync with the remote
  repository, no questions asked.
* Dependency graph! Builds a dependency graph internally, making sure everything happens _in the
  right order_ and as quickly as possible.
* Flexible configuration, but opinionated!
  There are a couple of powerful primitives available (e.g. `copy-dir`), which does _a lot_ of work
  with very little configuration.
* Hashes dependencies to reduce the amount of work as much as possible.

## Configuration

Create a repository with a `quickcfg.yml` in its root:

```
hierarchy:
  - secrets.yaml
  - db/common.yaml
  - db/{distro}.yaml

systems:
  # System to copy an entire directory to another.
  - type: copy-dir
    from: home
    to: home:.
    templates: true
  # System to ensure that a set of packages are installed.
  - type: install-packages
  # Will download and run the downloaded script once, recording it as done under the provided ID.
  - type: download-and-run
    id: install-rust
    url: https://sh.rustup.rs
  - type: download-and-run
    id: install-oh-my-zsh
    url: https://raw.githubusercontent.com/robbyrussell/oh-my-zsh/master/tools/install.sh
    shell: true
  # Create a symlink.
  - type: link
    path: home:.vimrc
    link: .vim/vimrc
  - type: link-dir
    from: bin
    to: home:usr/bin
```

## Systems

#### `copy-dir`

Copies a directory recursively.

```yaml
from: ./some/dir
to: home:some/dir
templates: false
```

Will copy a directory recursively.

#### `link-dir`

Links a directory recursively.

```yaml
from: ./some/dir
to: home:some/dir
```

Will create the corresponding directory structure, but all files will be symbolic links.

#### `install-packages`

Compares the set of installed packages, with a set of packages from the hierarchy to install and
installs any that are missing.

Will use `sudo` if needed to install packages.

```yaml
type: install-packages
# The provider of the package manager to use.
provider: pip3
# Hierarchy key to lookup for packages to install.
key: pip3::packages
```

The simplest example of this system is the one that uses the primary provider:

```yaml
systems:
  - type: install-packages
```

This will look up packages under the `packages` key and install it using the primary provider for
the system that you are currently running.

These are the supported providers:

 * `debian`: For Debian-based systems.

#### `download-and-run`

Downloads a script of the internet and runs it once.

```yaml
type: download-and-run
id: install-oh-my-zsh
url: https://raw.githubusercontent.com/robbyrussell/oh-my-zsh/master/tools/install.sh
shell: true
```

The `id` is to uniquely identify that this system has only been run once.

#### `link`

Creates a symlink.

```
type: link
path: home:.vimrc
link: .vim/vimrc
```

This creates a symbolic link at `path` which contains whatever is specified in `link`.

## Packages

We support installing packages on the following platforms:

* Debian, through `dpkg-query` and `apt` (fact: `distro=debian`).

## Templating

Any file being copied is treated as a [`handlebars`] template.

Any template file can make use of hierarchy data, by specifying their dependencies using
a `quickcfg:` tag at the top of the file, like this:

```
# quickcfg: name
Hi, my name is {{name}}
```

`quickcfg` will scan the first 5 lines of any file being copied for this.

[`handlebars`]: https://handlebarsjs.com/
