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
  There are a couple of powerful primitives available (e.g. `copy_dir`), which does _a lot_ of work
  with very little configuration.

## Configuration

Create a repository with a `quickcfg.yml` in its root:

```
hierarchy:
  - secrets.yaml
  - db/common.yaml
  - db/{distro}.yaml

systems:
  # system to copy an entire directory to another.
  - type: copy_dir
    # directory relative to root of this project.
    from: home
    to_home: true
  # system to ensure that a set of packages are installed.
  - type: install-packages
    # data key to use when resolving packages
    # will look up this key in the specified hierarchy.
    key: packages
```

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
