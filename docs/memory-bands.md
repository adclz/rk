# Memory bands

A module has no memory of its own.
It keeps all of its state in the memory the host gives it, imported as `env.memory`.
What a host needs to reach is grouped into **bands**: contiguous ranges, each exported as two globals, where it starts (`_base`) and how long it is (`_size`).
The input and output bands are copied whole: a host never needs to know the program's variables to run it.

The whole memory, in order:

```schema
env.memory                          the host's, from address 0
├─ builtins                         the first 16 KiB
├─ state                            every other variable
├─ %I                               input band
├─ %Q                               output band
├─ %M                               marker band
├─ %M RETAIN                        marker band and retain band
├─ PROGRAM RETAIN                   retain band
├─ VAR_GLOBAL RETAIN                retain band and globals band
├─ VAR_GLOBAL                       globals band
├─ strings                          the string literals
└─ stack                            the frames of recursive calls
```

A POU that calls itself, directly or through others, gets a frame on the stack at each call for its arrays, strings, structures and instances, where any other POU has them at a fixed address (`L0122`).
The stack holds the largest such frame and 64 KiB more, only in a module with such a POU; a deeper recursion stops the program with `stack overflow`.
`stack_size` in `config.toml` gives it another size, in bytes, up to 4 GiB (4294967296):

```toml
[settings]
stack_size = 1048576 # 1 MiB
```

`rk check` refuses a frame larger than the stack (`E1430`), and a TYPE, a variable, an instance or a frame that takes more than 4 GiB on its own, or a configuration whose globals and programs do together (`E0322`).
A module whose memory does not fit in 4 GiB, with its stack, stops the build, which names its largest parts.

| Band | Exports | Holds | The host |
|---|---|---|---|
| input | `input_base`, `input_size` | `%I` | writes it before every scan |
| output | `output_base`, `output_size` | `%Q` | reads it after every scan |
| marker | `marker_base`, `marker_size` | `%M` | can read and write it |
| retain | `retain_base`, `retain_size` | everything `RETAIN` | keeps it across a power cycle |
| globals | `globals_base`, `globals_size` | the configuration's `VAR_GLOBAL` | reads and writes it by name |

## Inputs and outputs

```schema
env.memory
├─ …
├─ %I                               input band                    ◄
├─ %Q                               output band                   ◄
├─ %M                               marker band
├─ %M RETAIN                        marker band and retain band
├─ PROGRAM RETAIN                   retain band
├─ VAR_GLOBAL RETAIN                retain band and globals band
├─ VAR_GLOBAL                       globals band
└─ …
```

The host writes `%I` before every scan, and reads `%Q` after it.
Each direction is one copy, a whole band at a time:

```schema
▒ the physical world                sensors, switches
▼ the host copies it in
%I input band                       input_base, input_size
▼ read by
TASK …                              the tasks due this tick
▼ written to
%Q output band                      output_base, output_size
▼ the host copies it out
▒ the physical world                valves, motors, lamps
```

The input, output and marker bands are exported only for an area the program uses.

## Markers

```schema
env.memory
├─ …
├─ %I                               input band
├─ %Q                               output band
├─ %M                               marker band                   ◄
├─ %M RETAIN                        marker band and retain band   ◄
├─ PROGRAM RETAIN                   retain band
├─ VAR_GLOBAL RETAIN                retain band and globals band
├─ VAR_GLOBAL                       globals band
└─ …
```

`%M` is the program's own memory, and the host can read and write it too.
A retained marker is in the marker band and the retain band.

## Retention

```schema
env.memory
├─ …
├─ %I                               input band
├─ %Q                               output band
├─ %M                               marker band
├─ %M RETAIN                        marker band and retain band   ◄
├─ PROGRAM RETAIN                   retain band                   ◄
├─ VAR_GLOBAL RETAIN                retain band and globals band  ◄
├─ VAR_GLOBAL                       globals band
└─ …
```

The retain band is restored at startup, before the first scan.
A retained program can hold fields that are not `RETAIN`: the `retain-map` section lists the exact ranges that persist, and a host restores those.

`%I` and `%Q` are never retained: the first scan would run on the values of the last power cycle.

## Globals

```schema
env.memory
├─ …
├─ %I                               input band
├─ %Q                               output band
├─ %M                               marker band
├─ %M RETAIN                        marker band and retain band
├─ PROGRAM RETAIN                   retain band
├─ VAR_GLOBAL RETAIN                retain band and globals band  ◄
├─ VAR_GLOBAL                       globals band                  ◄
└─ …
```

The host reads and writes a global by name.
A retained global is in the retain band and the globals band.

A located `VAR_GLOBAL`, like `level AT %IW0 : INT`, is in its area's band, not in the globals band.

## Inside a band

An area is sorted by address, not by the order the program declares it: a new variable never moves the others.

A cell takes the size of its declared type: `sensor AT %IX0.0 : BOOL` is one bit wide and takes four bytes.
The numbers of an address order it, but they are not byte offsets: `%IW2` and `%IW8` sit side by side.

The `located-map` section gives each address its place: the cell it lands in, its width, its type, and for a part of a wider address, which address owns it and at which bit.
See [Direct variables](direct-variables.md).
