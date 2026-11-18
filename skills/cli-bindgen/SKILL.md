---
name: cli-bindgen
description: Write the ST declarations of a host's WIT interfaces with `rk bindgen`, so that a library can call the functions a host provides — WASI's or a runtime's own. Use when a library must call an interface of a WIT package, or when a host's package changes.
---

> **Output format.** Every `rk` command takes `--output-format full|concise|json-lines`.
> - `full` is the human report
> - `concise` one line per diagnostic (`FILE:LINE:COL: severity[CODE]: message`)
> - `json-lines` one JSON object per line
> The exit code is the same either way.

## Summary

Writes the ST declarations of a host's WIT interfaces, one `.st` file per interface, so that ST code calls them as FUNCTIONs and FUNCTION_BLOCKs.

A library that calls a host's functions keeps the WIT of the host's package in a `wit/` folder at its root, as the host ships it: the package's files at the top of the folder, and in `deps/` a folder for each package it uses.
No world is written for it.

```
drives/
├─ config.toml
├─ Axis.st              the library's own blocks
├─ generated/           what rk bindgen writes
└─ wit/
   ├─ axis.wit          the host's package, as the host ships it
   └─ deps/             the packages it uses, if any
```

Run in the library's folder, `rk bindgen` reads `wit/` and writes `generated/`: every interface of the host's package, and those it takes types from.
A library that calls the packages of several hosts keeps them all in `deps/`, with no file at the top of `wit/`, and every interface of each package there is written.
When the host's package declares a world, what that world imports is written instead, and `--world` names one of several.

The declarations are under the package's namespace, `Acme.Drive.Axis` for `acme:drive/axis`, and `INTERNAL`: only the library's own blocks call them, the way the standard library's blocks call WASI.

The build reads the same `wit/` folder, and types each import of a package's interface with it, so the folder ships with the library.
An interface that no `wit/` ships stops the build.

## Usage

`--wit <FOLDER>` The WIT folder, `wit/` of the workspace by default.

`-o, --output <FOLDER>` Where the declarations go, `generated/` of the workspace by default.

`--world <WORLD>` The world whose imports are written, when the folder's package declares several.

`--public` Declarations every program reaches, where by default only the library that holds them does.

`--workspace <WORKSPACE>` Workspace path, `.` by default.

Running it again after the WIT changes rewrites the files.
A `.st` file that `rk bindgen` wrote before and no longer writes is removed; any other file of the output folder is left as it is.

What ST cannot pass yet is left out: each such function is listed when the command runs, and at the end of its file, with the reason.
