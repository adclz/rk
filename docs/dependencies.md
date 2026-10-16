# Dependencies

A dependency is a library your workspace uses.

A library is a folder of source files with its own `config.toml`.
You list the libraries you need in the `config.toml` of your workspace, and the compiler reads them with your own files.

```toml
[project]
name = "press-line"
version = "1.4.0"

[dependencies]
drives = { url = "https://github.com/acme/drives/archive/refs/tags/v2.1.0.tar.gz", hash = "sha256:9f2c…" }
shared = { path = "../shared" }
```

- **`path`** is a folder on your machine.
- **`url`** is an archive to download, or a commit of a git repository.
- **`hash`** says what the library contains. `rk fetch --save` writes it for you.

> [!NOTE]
> There is no package registry.
> A library is published by putting an archive at a URL, a tag on a git host for example.

## A library

A library is written like a workspace.
The name in `[dependencies]` is the `name` of its `config.toml`:

```toml
# drives/config.toml
[project]
name = "drives"
version = "2.1.0"
```

```st
// drives/Axis.st
NAMESPACE Drives
    FUNCTION_BLOCK Axis
    END_FUNCTION_BLOCK
END_NAMESPACE
```

You use it like any other namespace, see [Namespaces](namespaces.md):

```st
USING Drives;
```

A library can have a `[dependencies]` table of its own.
Those libraries are read too.

A library can also list the files it is made of:

```toml
[project]
name = "drives"
version = "2.1.0"
paths = ["src", "LICENSE"]
```

When it is fetched, the rest is left out: its tests, its build folder.
Without `paths`, every file is part of the library.

## A folder

```toml
shared = { path = "../shared" }
```

- The path is relative to the `config.toml` it is written in.
- The folder can be inside your workspace. Its files then belong to the library, and `rk fmt` leaves them alone.

## An archive

```sh
rk fetch --save https://github.com/acme/drives/archive/refs/tags/v2.1.0.tar.gz
```

This downloads the archive and writes the line in your `config.toml`, with its hash.

- **The hash** is computed from the files of the library, not from the archive itself.
- **The cache** keeps each library under its hash. A library that is in the cache is not downloaded again.
- **A new version** is another URL. Run `rk fetch --save` with it and the line is replaced.

`rk env cache` prints where the cache is.

An archive is a `.tar.gz` or a `.tar`.
Its URL can be `https`, `http` or `file`.
A `file` URL is an archive on your machine or on a shared drive.

## A git repository

A URL that starts with `git+https://` is a git repository.
What follows the `#` is a tag, a branch or a commit:

```sh
rk fetch --save git+https://github.com/acme/drives#v2.1.0
```

`rk fetch --save` finds the commit of that tag and writes it in the URL:

```toml
drives = { url = "git+https://github.com/acme/drives?ref=v2.1.0#4f1c…", hash = "sha256:9f2c…" }
```

The build then fetches that commit, even if the tag is moved later.
Only that commit is downloaded, not the history.

You do not need git installed.

## When a library is downloaded

`rk check`, `rk compile` and `rk test` download a library that is missing from the cache.

`rk fetch` downloads them all without building anything.

The editor never downloads.
It reports a missing library on its line of `config.toml`, and `rk fetch` fixes it.

> [!TIP]
> Set `RK_NO_DOWNLOAD` and only `rk fetch` downloads, see [Environment variables](environment-variables.md).

If a library does not match its hash, the build stops:

```sh
Error: 'drives' could not be fetched
Note: hash mismatch: config.toml declares sha256:9f2c… but the fetched library has sha256:41b0…
```

This also happens when you change a URL and keep the old hash.

## What a library keeps for itself

A `NAMESPACE INTERNAL` or a `FUNCTION PRIVATE` of a library can only be used by that library.

Your workspace cannot use it, and another library cannot either.

## The same library twice

All the libraries share the same namespaces, so a library is only read once.

If two of your dependencies need the same library, they must take it from the same place: the same `url` and `hash`, or the same `path`.

Two libraries cannot declare the same name either (`E0102`).

## The standard library

The standard library is not a dependency, it comes with the compiler.
It is kept in the cache like a fetched library, under the hash of its files.
See [StdLib](stdlib.md).
