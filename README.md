# quickcfg

This is a system to quickly apply a configuration to a machine.

It reads a configuration and template structure from a [dotfiles] directory and tries to normalize
the machine that it is run on from this configuration.

[dotfiles]: https://github.com/udoprog/dotfiles

## Features

* Zero dependencies, all you need is the `quickcfg` binary.
* Blazingly fast! We will normalize your machine and keep the configuration in sync with the remote
  repository, no questions asked!
* Dependency graph. Builds a dependency graph internally, making sure everything happens _in the
  right order. And that anything that _can_ be parallelized - will!
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
    from: home
    to: ..
```
