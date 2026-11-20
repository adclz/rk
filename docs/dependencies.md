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
- **`hash`** says what the library contains. `rk fetch <url>` writes it for you.

> [!NOTE]
> There is no package registry.
> A library is published by putting an archive at a URL, a tag on a git host for example.

## A library

A library is written like a workspace, and its `config.toml` says it is one with `type = "library"`.
The name in `[dependencies]` is the `name` of its `config.toml`:

```toml
# drives/config.toml
[project]
name = "drives"
version = "2.1.0"
type = "library"
```

A folder without `type = "library"` is a program, and a program is not a dependency: it is refused.
In its own folder, `rk check` and `rk test` hold a library to the rules every program that lists it holds it to, and `rk compile` refuses it: a library is built into the programs that list it, never alone.
In your program, `rk check` reports a library's errors after your own diagnostics, as `rk compile` does.

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

The library's name is its namespace: everything `drives` declares is in `Drives`, and a `NAMESPACE Motion` of its files is `Drives.Motion`.
A name only one library has resolves bare; a name two libraries have takes the library's name before it.

Names are case-insensitive, so `drives` and `Drives` are one name.
Two entries whose names differ only in case are both refused, and so is one that differs from the standard library's name, `Std`, only in case.

A library has no `[dependencies]` of its own: your workspace lists every library the build reads.
One whose `config.toml` lists some is refused, with an error on the line of your `config.toml` that names it.

A library can also list the files it is made of:

```toml
[project]
name = "drives"
version = "2.1.0"
type = "library"
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
rk fetch https://github.com/acme/drives/archive/refs/tags/v2.1.0.tar.gz
```

This downloads the archive, checks the library, and writes the line in your `config.toml`, with its hash.
It checks the line as the build reads it, and the library's code as `rk check` holds a library to its rules: a library that would be refused is reported, and nothing is written.
The same URL again writes nothing: the line is up to date.

- **The hash** is computed from the files of the library, not from the archive itself.
- **The cache** keeps each library under its hash. A library that is in the cache is not downloaded again.
- **A new version** is another URL. Run `rk fetch` with it and the line is replaced.

`rk env cache` prints where the cache is.

An archive is a `.tar.gz` or a `.tar`.
Its URL can be `https`, `http` or `file`.
A `file` URL is an archive on your machine or on a shared drive.

## A git repository

A URL that starts with `git+https://` is a git repository.
What follows the `#` is a tag, a branch or a commit:

```sh
rk fetch git+https://github.com/acme/drives#v2.1.0
```

`rk fetch` finds the commit of that tag and writes it in the URL:

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

## What a library sees

A library's code sees its own declarations and the standard library's.
It does not see another library your program lists, nor your program's namespaces and globals (`E1008`), so what it names means the same in every program.
A namespace your program opens under the library's name is your program's too.

A block of the library takes what it needs through its inputs and in-outs, and its `AT %I*` variables are located by your `VAR_CONFIG`.

Your program's code sees every library it lists.

## What a library keeps for itself

A `NAMESPACE INTERNAL` or a `FUNCTION PRIVATE` of a library can only be used by that library.

Your workspace cannot use it, and another library cannot either.

## Hardware

A library names no address: it is code for any machine.
Its blocks declare `AT %I*`, `%Q*` or `%M*`, and your configuration gives each instance its address in `VAR_CONFIG`.
A complete address in a library is `E1432`, in a `path` as in a `url`.
A library declares no `CONFIGURATION` either (`E1433`): the configuration, its globals and its tasks are your program's.

See [VAR_CONFIG](var-config.md).

## A component

A library can carry a component, a `.wasm` built from another language, in a `component/` folder at its root, with the component's LICENSE beside it.
One library carries one component, with its code: the WIT of a host's package goes in `wit/` (below), and a library may carry both.
Your program carries none: it calls a component through a library, and a `component/` folder of its own is refused.

```schema
drives/
├─ config.toml
├─ Axis.st                          the library's own blocks
├─ component/
│  ├─ drive.wasm                    the component, as its language built it
│  └─ LICENSE
└─ generated/                       what rk bindgen writes from the component
```

The component's binary carries its types: no WIT is shipped beside it.
[`rk bindgen`](../skills/cli-bindgen/SKILL.md), run in the library's folder, reads the component and writes what it exports into `generated/`, as your program imports it: each interface it exports, one file each, with those they take types from.
A `func` is a `FUNCTION`, and an `async func` a block you call until its `DONE`.
What the component imports itself is its own, and is not written.

The component runs beside your program, not inside it.
`rk compile` types your program's imports of its interfaces from its binary, and writes it beside your program, in `components/`, named after its library:

```schema
rk_build/debug/
├─ press-line.wasm                  your program
└─ components/
   └─ drives.wasm                   the component of the library drives
```

The runtime runs each one beside your program, in a store of its own, and links each of your program's imports to the component that exports it.
A component your program does not call is not written.

- Two components that export the same interface are refused: an import is linked to one. The same component carried by two libraries is one.
- Only plain values cross, for now: a function that takes or gives a resource, a stream or a future is refused.

`rk test` runs them the same way.
It writes them beside its own build, in `rk_build/test/components/`, and starts each one for each test, in a store of its own, with the WASI 0.3 your program gets: its clocks and its command line.
Build a component for WASI 0.3, with Rust's `--target wasm32-wasip3` for example.
A trap in a component fails the test that called it, named after its library, at your program's call:

```console
    FAIL test_stuck (main.st:26): trapped in the component of 'drives': unknown import: `acme:drive/bus@1.0.0#read` has not been defined [at test_stuck (main.st:27:5)]
```

An import of the component that `rk test` does not give, as here, traps only when the component calls it.

## The host's interfaces

A library calls the functions a host provides, WASI's or a runtime's own, through the interfaces of their packages.
It ships the WIT of those packages in a `wit/` folder at its root, as WIT's tools lay one out: the host's package, its files at the top of the folder, and in `deps/` one for each package it uses.
A package in `deps/` may be a folder, a `.wit` file, or a binary `.wasm` package, as a registry hands one out.
No world is written for it.

```schema
drives/
├─ config.toml
├─ Axis.st                          the library's own blocks
├─ generated/                       what rk bindgen writes from wit/
└─ wit/
   ├─ axis.wit                      the host's package, as the host ships it
   └─ deps/                         the packages it uses, if any
```

[`rk bindgen`](../skills/cli-bindgen/SKILL.md), run in the library's folder, writes the ST declarations of every interface of the host's package into `generated/`, one file per interface, with those the package takes types from, beside what the library's component exports when it carries one:

```sh
rk bindgen
```

A library that calls the packages of several hosts keeps them all in `deps/`, with no file at the top of `wit/`, and every interface of each package there is written.
When the host's package declares a world, what that world imports is written instead, and `--world` names one of several.

The declarations are `INTERNAL`: only the library's own blocks call them, as the standard library's blocks call WASI's.
With `--public` they are written for every program.
Running it again after the WIT changes rewrites them, and leaves the library's own files alone.

The build reads the `wit/` of every library, each folder on its own, and types each import of a package's interface with it, see [WASM ABI](wasm-abi.md): a folder holds in its `deps/` the packages its own use.
An interface the program calls that no `wit/` ships stops the build, and so does one shipped at another version. A declaration nothing calls is neither imported nor checked.
An interface is the host's or a component's: one that a component exports and a library's `wit/` declares is refused.

Several libraries can ship parts of one package, the interfaces each one calls, as text or as binaries, and the build reads them as one package.
An interface two of them ship must be written the same way in both, docs and feature gates (`@since`, `@unstable`) aside.

Your program carries no WIT: it calls a host's interfaces through a library, and a `wit/` folder of its own is refused.

The standard library ships the WASI files its blocks call this way, in `stdlib/wit/`.
A library that calls more of WASI ships the files it needs, as WASI's repository writes them.

## The standard library

The standard library is not a dependency, it comes with the compiler.
It is kept in the cache like a fetched library, under the hash of its files.
See [StdLib](stdlib.md).

Your `[dependencies]` cannot replace it.
To read another standard library, set `RK_STDLIB_PATH`, see [Environment variables](environment-variables.md).
