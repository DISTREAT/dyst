# dyst

A distilled package manager for pulling executable assets from GitHub releases.

_This program was designed as an alternative to [stew](https://github.com/marwanhawari/stew)._

## Features

- Automatically determines the asset based on the computer's architecture and operating system
- Extracts downloaded archives autonomously
- Define a rule to automatically rename the executable
- Update all binaries at once
- Assets are downloaded per-user

![Preview Asset Installation](docs/install.png)

## Installation

```
# download the compiled binary
curl -s -L https://github.com/DISTREAT/dyst/releases/download/0.1.0/dyst-linux-x86_64.tar.gz | tar xz - -C /tmp

# 'bootstrap' dyst by installing dyst using dyst
/tmp/dyst install DISTREAT/dyst -p
```

Download compiled binaries from the [release page](https://github.com/DISTREAT/dyst/releases).

## Usage

### Basic Installation

```
# dyst will automatically try to detect the asset needed
dyst install oven-sh/bun

# install prereleases if available
dyst install -p oven-sh/bun
```

### Installing a specific asset

```
# get a list of all assets
dyst install -a oven-sh/bun

# install a specific asset
dyst install -f aarch64 oven-sh/bun
```

### Installing a specific tag

```
# install a specific tag
dyst install -t 0.1.0 oven-sh/bun

# lock the asset, preventing updates
dyst install -l -t 0.1.0 oven-sh/bun
```

### Removing a repository

```
# list all installed repositories
dyst list

# remove all downloaded files
dyst remove oven-sh/bun
```

### Renaming an executable

```
# list all executables
dyst list-execs oven-sh/bun

# rename an executable
dyst rename oven-sh/bun old-name/new-name
```

### Updating assets

```
# update all downloaded assts
dyst update
```

