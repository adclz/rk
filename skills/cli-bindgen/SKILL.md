---
name: cli-bindgen
description: Write the ST declarations of a library's component, or of a host's WIT interfaces, with `rk bindgen`, so that ST code calls the functions a component exports or a host provides. Use when a library carries a component built from another language, when a library must call an interface of a WIT package, or when either changes.
---

> **Output format.** Every `rk` command takes `--output-format full|concise|json-lines`.
> - `full` is the human report
> - `concise` one line per diagnostic (`FILE:LINE:COL: severity[CODE]: message`)
> - `json-lines` one JSON object per line
> The exit code is the same either way.

## Summary

Writes the ST declarations of WIT interfaces, one `.st` file per interface, so that ST code calls them as FUNCTIONs and FUNCTION_BLOCKs.

## A library's component

A library can carry a component, a `.wasm` its language built, in a `component/` folder at its root, with the component's LICENSE beside it: one component a library.

```
drives/
├─ config.toml
├─ Axis.st              the library's own blocks
├─ component/
│  ├─ drive.wasm        the component, as its language built it
│  └─ LICENSE
└─ generated/           what rk bindgen writes
```

The component's binary carries its types, so no WIT is shipped beside it.
Run in the library's folder, `rk bindgen` reads the component and writes what it exports into `generated/`, as the program imports it: each interface it exports, and those they take types from.
A `func` is a FUNCTION, an `async func` a FUNCTION_BLOCK called until its `DONE`.
A stream's or a future's built-ins are written for the ways the interface passes it: those that make and write one when a function takes one, those that read one when a function gives one.
A future a function gives comes with a `_GET` block, `U32_FUTURE_GET` for a `future<u32>`, which reads it and lets go of its reading end in the call that sees `DONE`, setting the handle to 0.
A function that takes a stream of bytes comes with a writer of its own, `SEND_WRITER` for `send`: at the first rising edge of `REQ` it makes the stream and calls the function with it and with the block's other inputs, then writes `IN` at each rising edge of `REQ` and ends the stream on a rising edge of `CLOSE`.
A `func` that answers with a future or nothing is called once, and the writer keeps the future's value, `ENDED` once it has come; an `async func` is called until it returns, and the writer keeps its result, `ENDED` once it has returned.
When that value is a `result`, an error turns the writer's `ERROR` TRUE and its value is `ERRORID`, to read when `ERROR` is TRUE.
Each input and output of a writer says on hover what it is, in WIT's terms.
It is built on two blocks a program may also use on their own: the stream's `BYTES_STREAM_WRITER`, which makes a stream and gives `readable`, the end to hand to a function, and the future's `_GET`.
What the component imports itself is its own, and is not written.
A `component/` folder with no `.wasm`, or with two, is refused.

## A host's interfaces

A library that calls a host's functions keeps the WIT of the host's package in a `wit/` folder at its root, as WIT's tools lay one out: the package's files at the top of the folder, and in `deps/` one for each package it uses, a folder, a `.wit` file or a binary `.wasm` package.
No world is written for it.
A library may carry both a component and a `wit/`.

```
drives/
├─ config.toml
├─ Axis.st              the library's own blocks
├─ generated/           what rk bindgen writes
└─ wit/
   ├─ axis.wit          the host's package, as the host ships it
   └─ deps/             the packages it uses, if any
```

Run in the library's folder, `rk bindgen` writes into `generated/` what the component exports and every interface of the host's package in `wit/`, with those it takes types from.
A library that calls the packages of several hosts keeps them all in `deps/`, with no file at the top of `wit/`, and every interface of each package there is written.
When the host's package declares a world, what that world imports is written instead, and `--world` names one of several.

The declarations are under the package's namespace, `Acme.Drive.Axis` for `acme:drive/axis`, and `INTERNAL`: only the library's own blocks call them, the way the standard library's blocks call WASI.

The build reads each library's `wit/` on its own, and types each import of a package's interface with it, so the folder ships with the library and holds in its `deps/` what its own packages use.
An interface the program calls that no `wit/` ships stops the build; a declaration nothing calls costs nothing.
A program carries no WIT and no component: it calls a host's interfaces and a component through a library.
A `wit/` or a `component/` folder in a program is refused, and `rk bindgen` in a program reads only the folder `--wit` names.

## Usage

`--wit <FOLDER>` A WIT folder to read instead. By default, the library's component and its `wit/` folder.

`-o, --output <FOLDER>` Where the declarations go, `generated/` of the workspace by default.

`--world <WORLD>` The world whose imports are written, when the folder's package declares several.

`--public` Declarations every program reaches, where by default only the library that holds them does.

`--workspace <WORKSPACE>` Workspace path, `.` by default.

Running it again after the WIT changes rewrites the files.
A `.st` file that `rk bindgen` wrote before and no longer writes is removed; any other file of the output folder is left as it is.

What ST cannot pass yet is left out: each such function is listed when the command runs, and at the end of its file, with the reason.
