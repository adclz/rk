# Environment variables

| Variable | Used by | Effect |
|---|---|---|
| `RK_STDLIB_PATH` | CLI, language server | Where the standard library is. |
| `RK_NO_DOWNLOAD` | CLI | When set, Binaryen is never downloaded. |
| `NO_COLOR` | CLI | When set, the output is not colored, see [no-color.org](https://no-color.org). |

## The `.env` file

`RK_STDLIB_PATH` can also be written in a `.env` file at the root of your workspace:

```sh
# .env
RK_STDLIB_PATH="/path/to/stdlib"
```

> [!NOTE]
> The process environment wins when both are set.
>
> This is the only variable read from the `.env` file.

If it is not set anywhere, the standard library is looked up beside the `rk` binary, or in the `stdlib/` folder of the checkout when running from a build.

> [!TIP]
> `rk env` prints the standard library in use, and where it was found.
