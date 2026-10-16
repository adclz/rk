# Environment variables

| Variable | Used by | Effect |
|---|---|---|
| `RK_STDLIB_PATH` | CLI, language server | A folder to read as the standard library, in place of the one built in. Empty, no standard library is read. |
| `RK_NO_DOWNLOAD` | CLI | When set, Binaryen is never downloaded, and a missing library is only downloaded by `rk fetch`. |
| `RK_CACHE_DIR` | CLI, language server | Where the standard library, the downloaded libraries and Binaryen are kept. |
| `NO_COLOR` | CLI | When set, the output is not colored, see [no-color.org](https://no-color.org). |

## The standard library

The standard library is built into the compiler.
At each start, it is written in the cache under its hash, unless the copy there is the same already.

`RK_STDLIB_PATH` names another folder to read in its place, the `stdlib/` of a checkout for example.

> [!TIP]
> `rk env` prints the folder the standard library is read from.

## The `.env` file

`RK_STDLIB_PATH` can also be written in a `.env` file at the root of your workspace:

```sh
# .env
RK_STDLIB_PATH="/path/to/stdlib"
```

The process environment wins when both are set.
This is the only variable read from the `.env` file.

The standard library's own workspace has a `.env` that leaves it empty, so it reads no copy of itself.
