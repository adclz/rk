---
name: programming-pointers
description: Pointers in Structured Text — REF_TO declarations, REF() to take an
  address, ^ to dereference, NULL, and the possibly-null analysis (E0902). Use
  when passing a buffer by address, or when a deref is refused as possibly null.
---

## Summary

`REF_TO T` is a reference to a `T`.
`REF(x)` takes the address of a variable, `^` reads through it, and `NULL` is the empty reference.

```iecst
FUNCTION Sum : INT
	VAR
		a : ARRAY[0..2] OF INT := [1, 2, 3];
		p : REF_TO INT;
	END_VAR
	p := REF(a[0]);
	Sum := p^;
END_FUNCTION
```

A reference is a value: it assigns, compares against `NULL`, and passes into a `VAR_INPUT` like any scalar.
Writing through it uses the same `^`:

```iecst fragment
p^ := INT#9;                 // writes the pointee, not the pointer
```

`REF()` takes an address, so it needs a VARIABLE.
`REF(5)` is not an expression the language has.

## Binding is invariant

A value converts; a reference does not.
`INT` widens to `REAL` when it is ASSIGNED, but a reference hands out the slot itself, so its pointee must be EXACTLY the declared type.
Every way of binding one is E0301 otherwise:

```iecst fragment
VAR x : INT; p : REF_TO REAL; q : REF_TO INT; END_VAR
p := REF(x);                 // E0301: expected REF_TO REAL, got REF_TO INT
p := q;                      // E0301: a reference from a reference, same rule
g(p := REF(x));              // E0301: a REF_TO REAL parameter, named or positional
```

`VAR_IN_OUT` aliases the caller's storage the same way, so it follows the same rule: a `REAL` in-out cannot be bound to an `INT` variable, even though `REAL := INT` is fine as a value.
A `=>` output is a VALUE copy out of the callee, so it may widen: a `REAL` output into an `LREAL` variable, an `INT` into a `REAL`, a `DINT` into a `LINT` all convert on the way out.

"Exactly the type" is by shape, not by spelling: a `TYPE` alias of `INT`, a second `ARRAY[0..2] OF INT` declared elsewhere, or the same `STRUCT` all match.
The one deliberate exception is an INTERFACE-typed parameter, which takes any implementer — that is dispatch, not a reinterpretation of the slot.

A SUBRANGE is part of the type here.
`REF_TO INT := REF(s)` with `s : INT (0..10)` is E0704 (as it is for a `VAR_IN_OUT`): a write through `p^` would go around the range check that guards `s`.
Bounds must be equal, whether spelled inline or through a `TYPE`; a plain `INT` behind a `REF_TO Small` is refused in the other direction for the same reason.

Before this rule, `REF_TO REAL := REF(x)` with `x : INT` checked clean and either failed to load or read a two-byte slot as four.

## Declaring

`REF_TO` composes with any type in a DECLARATION — `REF_TO INT`, `REF_TO STRING`, `REF_TO fb`, `REF_TO ARRAY[0..2] OF INT`, and `REF_TO REF_TO INT` (which derefs as `pp^^`).

Reaching INTO an aggregate through a dereference works, and addresses the pointee's own storage:

```iecst fragment
q^.x := 5;                   // REF_TO STRUCT field
r^[1] := 9;                  // REF_TO ARRAY element
t := f^.o;                   // REF_TO FB output
```

A type may hold a reference to itself (`Node.next : REF_TO Node`), which is what makes a linked structure expressible.

Pointing at the SCALAR — `REF(s.x)`, `REF(a[i])` — remains the right shape for an `{extern}` buffer argument, which wants one address.
Use `VAR_IN_OUT` when a whole struct or FB is shared with a callee that should not see a pointer.

A return type is a type NAME, so a function cannot say `FUNCTION f : REF_TO INT` — declare the reference type and return that:

```iecst
TYPE PInt : REF_TO INT; END_TYPE

FUNCTION borrow : PInt
VAR_IN_OUT t : INT; END_VAR
	borrow := REF(t);
END_FUNCTION
```

A method returns one the same way, and a reference to instance state stays valid after the call returns — that is the point of handing one out:

```iecst fragment
p := c.slot();
p^ := p^ + 1;                // reaches c's own storage
```

Returning a reference to the POU's own per-call storage — a `VAR`, `VAR_TEMP` or `VAR_INPUT` of the FUNCTION or METHOD itself — is `E0903`.
It would not fault: a local whose address is taken lives at a fixed address, so the reference stays readable and quietly observes whatever the next call leaves in that slot.
Hand out a reference to instance state, or to a `VAR_IN_OUT`, which names storage the caller owns.

## REF_TO versus VAR_IN_OUT

Both give the callee access to the caller's storage.
`VAR_IN_OUT` is the one to reach for: the compiler passes the address, the callee writes the name with no `^`, and it cannot be null.
Use `REF_TO` when the reference itself is DATA — stored in an instance, reseated between scans, compared to `NULL`, or handed to an `{extern}` host function as a buffer address.

A `REF_TO` crossing an `{extern}` is a plain machine address, which is how a driver library hands a buffer to its host functions.
Nothing carries a LENGTH with it, so a capacity always travels as its own argument.

## The possibly-null analysis (E0902)

Dereferencing a reference the compiler cannot prove is non-null is **E0902, an error that blocks the build**.
Two things clear it: an assignment, or a guard.

```iecst fragment
VAR p : REF_TO INT := REF(x); END_VAR    // assigned at declaration
p := REF(x);   r := p^;                  // assigned before the deref

IF p <> NULL THEN  r := p^;  END_IF;     // guarded
IF p = NULL THEN RETURN; END_IF;  r := p^;   // the returning branch is dropped
IF p = NULL THEN  r := 0;  ELSE  r := p^;  END_IF;
IF NOT (p = NULL) THEN  r := p^;  END_IF;
WHILE p <> NULL DO  r := p^;  p := NULL;  END_WHILE;
```

The guard narrows into the BODY even when it is one term of a compound condition, in either order — `IF (p <> NULL) AND flag THEN p^` and `IF flag AND (p <> NULL) THEN p^` both pass.

**What does NOT narrow is a dereference in the SAME expression as its own guard**, and that refusal is correct:

```iecst fragment
b := (p <> NULL) AND (p^ > 0);           // E0902 — and rightly so
```

**`AND` and `OR` do not short-circuit.** Both operands are evaluated, so `p^` really executes when `p` is NULL and faults at runtime.
IEC 61131-3 does not mandate short-circuit evaluation, and this compiler does not provide it.
Write the guard as a nested `IF` instead:

```iecst fragment
IF p <> NULL THEN
	b := p^ > 0;
END_IF;
```

A `VAR_INPUT` or `VAR_IN_OUT` reference is not null-tracked, so a dereference there never raises E0902 — the callee carries an obligation the caller no longer states.
Should a null one arrive anyway, the dereference faults with `dereference of a null reference` rather than quietly reading address 0.

## Gotchas

`REF(x)` where `x` is an element or a field (`REF(a[i])`, `REF(s.f)`) is legal and common; it is how a buffer's base address reaches an extern.

Assigning a value where a reference is expected (`p := x` rather than `p := REF(x)`) is a type error (E0301), not a silent reinterpretation.

Comparing references compares ADDRESSES.
Two references to equal values are not equal; `p = q` asks whether they point at the same storage.
