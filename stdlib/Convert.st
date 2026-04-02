/*
# Standard Type Conversion Functions

IEC 61131-3 **Table 22** — Explicit type conversion (narrowing casts).

These functions handle conversions that are **not** covered by implicit casting,
typically narrowing conversions where precision or range may be lost.

Each function follows the naming convention `<SOURCE>_TO_<TARGET>`.
All conversions use {wasm} intrinsics — zero overhead, single instruction.
*/
NAMESPACE Std.Convert

	/* ## LREAL (f64) Conversions */

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_REAL: REAL
		VAR_INPUT
			IN: LREAL;
		END_VAR
		{wasm 'f32.demote_f64' (params IN) (result LREAL_TO_REAL)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_LINT: LINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		{wasm 'i64.trunc_f64_s' (params IN) (result LREAL_TO_LINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_DINT: DINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		{wasm 'i32.trunc_f64_s' (params IN) (result LREAL_TO_DINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_INT: INT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		{wasm 'i32.trunc_f64_s' (params IN) (result LREAL_TO_INT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_SINT: SINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		{wasm 'i32.trunc_f64_s' (params IN) (result LREAL_TO_SINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_ULINT: ULINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		{wasm 'i64.trunc_f64_u' (params IN) (result LREAL_TO_ULINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_UDINT: UDINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		{wasm 'i32.trunc_f64_u' (params IN) (result LREAL_TO_UDINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_UINT: UINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		{wasm 'i32.trunc_f64_u' (params IN) (result LREAL_TO_UINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_USINT: USINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		{wasm 'i32.trunc_f64_u' (params IN) (result LREAL_TO_USINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_LWORD: LWORD
		VAR_INPUT
			IN: LREAL;
		END_VAR
		{wasm 'i64.reinterpret_f64' (params IN) (result LREAL_TO_LWORD)}
	END_FUNCTION

	/* ## REAL (f32) Conversions */

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_LINT: LINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		{wasm 'i64.trunc_f32_s' (params IN) (result REAL_TO_LINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_DINT: DINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		{wasm 'i32.trunc_f32_s' (params IN) (result REAL_TO_DINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_INT: INT
		VAR_INPUT
			IN: REAL;
		END_VAR
		{wasm 'i32.trunc_f32_s' (params IN) (result REAL_TO_INT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_SINT: SINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		{wasm 'i32.trunc_f32_s' (params IN) (result REAL_TO_SINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_ULINT: ULINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		{wasm 'i64.trunc_f32_u' (params IN) (result REAL_TO_ULINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_UDINT: UDINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		{wasm 'i32.trunc_f32_u' (params IN) (result REAL_TO_UDINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_UINT: UINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		{wasm 'i32.trunc_f32_u' (params IN) (result REAL_TO_UINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_USINT: USINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		{wasm 'i32.trunc_f32_u' (params IN) (result REAL_TO_USINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_DWORD: DWORD
		VAR_INPUT
			IN: REAL;
		END_VAR
		{wasm 'i32.reinterpret_f32' (params IN) (result REAL_TO_DWORD)}
	END_FUNCTION

	/* ## LINT (i64 signed) Conversions */

	FUNCTION LINT_TO_LREAL: LREAL
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'f64.convert_i64_s' (params IN) (result LINT_TO_LREAL)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LINT_TO_REAL: REAL
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'f32.convert_i64_s' (params IN) (result LINT_TO_REAL)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LINT_TO_DINT: DINT
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LINT_TO_DINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LINT_TO_INT: INT
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LINT_TO_INT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LINT_TO_SINT: SINT
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LINT_TO_SINT)}
	END_FUNCTION
	// LINT → unsigned: same i64 bit pattern
	FUNCTION LINT_TO_ULINT: ULINT
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'nop' (params IN) (result LINT_TO_ULINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LINT_TO_UDINT: UDINT
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LINT_TO_UDINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LINT_TO_UINT: UINT
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LINT_TO_UINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LINT_TO_USINT: USINT
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LINT_TO_USINT)}
	END_FUNCTION
	// LINT → bit types: reinterpret
	FUNCTION LINT_TO_LWORD: LWORD
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'nop' (params IN) (result LINT_TO_LWORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LINT_TO_DWORD: DWORD
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LINT_TO_DWORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LINT_TO_WORD: WORD
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LINT_TO_WORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LINT_TO_BYTE: BYTE
		VAR_INPUT
			IN: LINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LINT_TO_BYTE)}
	END_FUNCTION

	/* ## DINT (i32 signed) Conversions */

	FUNCTION DINT_TO_REAL: REAL
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm 'f32.convert_i32_s' (params IN) (result DINT_TO_REAL)}
	END_FUNCTION
	// DINT → smaller signed: identity (all i32 in WASM)
	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION DINT_TO_INT: INT
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm 'nop' (params IN) (result DINT_TO_INT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION DINT_TO_SINT: SINT
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm 'nop' (params IN) (result DINT_TO_SINT)}
	END_FUNCTION

	// DINT → unsigned: same i32 bit pattern
	FUNCTION DINT_TO_ULINT: ULINT
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm 'i64.extend_i32_s' (params IN) (result DINT_TO_ULINT)}
	END_FUNCTION

	FUNCTION DINT_TO_UDINT: UDINT
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm 'nop' (params IN) (result DINT_TO_UDINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION DINT_TO_UINT: UINT
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm 'nop' (params IN) (result DINT_TO_UINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION DINT_TO_USINT: USINT
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm 'nop' (params IN) (result DINT_TO_USINT)}
	END_FUNCTION
	// DINT → bit types
	FUNCTION DINT_TO_LWORD: LWORD
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm 'i64.extend_i32_s' (params IN) (result DINT_TO_LWORD)}
	END_FUNCTION

	FUNCTION DINT_TO_DWORD: DWORD
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm 'nop' (params IN) (result DINT_TO_DWORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION DINT_TO_WORD: WORD
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm 'nop' (params IN) (result DINT_TO_WORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION DINT_TO_BYTE: BYTE
		VAR_INPUT
			IN: DINT;
		END_VAR
		{wasm 'nop' (params IN) (result DINT_TO_BYTE)}
	END_FUNCTION

	/* ## INT (i32 signed, 16-bit semantic) — all i32 in WASM */

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION INT_TO_SINT: SINT
		VAR_INPUT
			IN: INT;
		END_VAR
		{wasm 'nop' (params IN) (result INT_TO_SINT)}
	END_FUNCTION

	FUNCTION INT_TO_ULINT: ULINT
		VAR_INPUT
			IN: INT;
		END_VAR
		{wasm 'i64.extend_i32_s' (params IN) (result INT_TO_ULINT)}
	END_FUNCTION

	FUNCTION INT_TO_UDINT: UDINT
		VAR_INPUT
			IN: INT;
		END_VAR
		{wasm 'nop' (params IN) (result INT_TO_UDINT)}
	END_FUNCTION

	FUNCTION INT_TO_UINT: UINT
		VAR_INPUT
			IN: INT;
		END_VAR
		{wasm 'nop' (params IN) (result INT_TO_UINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION INT_TO_USINT: USINT
		VAR_INPUT
			IN: INT;
		END_VAR
		{wasm 'nop' (params IN) (result INT_TO_USINT)}
	END_FUNCTION

	FUNCTION INT_TO_LWORD: LWORD
		VAR_INPUT
			IN: INT;
		END_VAR
		{wasm 'i64.extend_i32_s' (params IN) (result INT_TO_LWORD)}
	END_FUNCTION

	FUNCTION INT_TO_DWORD: DWORD
		VAR_INPUT
			IN: INT;
		END_VAR
		{wasm 'nop' (params IN) (result INT_TO_DWORD)}
	END_FUNCTION

	FUNCTION INT_TO_WORD: WORD
		VAR_INPUT
			IN: INT;
		END_VAR
		{wasm 'nop' (params IN) (result INT_TO_WORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION INT_TO_BYTE: BYTE
		VAR_INPUT
			IN: INT;
		END_VAR
		{wasm 'nop' (params IN) (result INT_TO_BYTE)}
	END_FUNCTION

	/* ## SINT (i32, 8-bit semantic) */

	FUNCTION SINT_TO_ULINT: ULINT
		VAR_INPUT
			IN: SINT;
		END_VAR
		{wasm 'i64.extend_i32_s' (params IN) (result SINT_TO_ULINT)}
	END_FUNCTION

	FUNCTION SINT_TO_UDINT: UDINT
		VAR_INPUT
			IN: SINT;
		END_VAR
		{wasm 'nop' (params IN) (result SINT_TO_UDINT)}
	END_FUNCTION

	FUNCTION SINT_TO_UINT: UINT
		VAR_INPUT
			IN: SINT;
		END_VAR
		{wasm 'nop' (params IN) (result SINT_TO_UINT)}
	END_FUNCTION

	FUNCTION SINT_TO_USINT: USINT
		VAR_INPUT
			IN: SINT;
		END_VAR
		{wasm 'nop' (params IN) (result SINT_TO_USINT)}
	END_FUNCTION

	FUNCTION SINT_TO_LWORD: LWORD
		VAR_INPUT
			IN: SINT;
		END_VAR
		{wasm 'i64.extend_i32_s' (params IN) (result SINT_TO_LWORD)}
	END_FUNCTION

	FUNCTION SINT_TO_DWORD: DWORD
		VAR_INPUT
			IN: SINT;
		END_VAR
		{wasm 'nop' (params IN) (result SINT_TO_DWORD)}
	END_FUNCTION

	FUNCTION SINT_TO_WORD: WORD
		VAR_INPUT
			IN: SINT;
		END_VAR
		{wasm 'nop' (params IN) (result SINT_TO_WORD)}
	END_FUNCTION

	FUNCTION SINT_TO_BYTE: BYTE
		VAR_INPUT
			IN: SINT;
		END_VAR
		{wasm 'nop' (params IN) (result SINT_TO_BYTE)}
	END_FUNCTION

	/* ## ULINT (i64 unsigned) */

	FUNCTION ULINT_TO_LREAL: LREAL
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'f64.convert_i64_u' (params IN) (result ULINT_TO_LREAL)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION ULINT_TO_REAL: REAL
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'f32.convert_i64_u' (params IN) (result ULINT_TO_REAL)}
	END_FUNCTION

	FUNCTION ULINT_TO_LINT: LINT
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'nop' (params IN) (result ULINT_TO_LINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION ULINT_TO_DINT: DINT
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result ULINT_TO_DINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION ULINT_TO_INT: INT
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result ULINT_TO_INT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION ULINT_TO_SINT: SINT
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result ULINT_TO_SINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION ULINT_TO_UDINT: UDINT
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result ULINT_TO_UDINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION ULINT_TO_UINT: UINT
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result ULINT_TO_UINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION ULINT_TO_USINT: USINT
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result ULINT_TO_USINT)}
	END_FUNCTION

	FUNCTION ULINT_TO_LWORD: LWORD
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'nop' (params IN) (result ULINT_TO_LWORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION ULINT_TO_DWORD: DWORD
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result ULINT_TO_DWORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION ULINT_TO_WORD: WORD
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result ULINT_TO_WORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION ULINT_TO_BYTE: BYTE
		VAR_INPUT
			IN: ULINT;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result ULINT_TO_BYTE)}
	END_FUNCTION

	/* ## UDINT (i32 unsigned) */

	FUNCTION UDINT_TO_REAL: REAL
		VAR_INPUT
			IN: UDINT;
		END_VAR
		{wasm 'f32.convert_i32_u' (params IN) (result UDINT_TO_REAL)}
	END_FUNCTION

	FUNCTION UDINT_TO_DINT: DINT
		VAR_INPUT
			IN: UDINT;
		END_VAR
		{wasm 'nop' (params IN) (result UDINT_TO_DINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION UDINT_TO_INT: INT
		VAR_INPUT
			IN: UDINT;
		END_VAR
		{wasm 'nop' (params IN) (result UDINT_TO_INT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION UDINT_TO_SINT: SINT
		VAR_INPUT
			IN: UDINT;
		END_VAR
		{wasm 'nop' (params IN) (result UDINT_TO_SINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION UDINT_TO_UINT: UINT
		VAR_INPUT
			IN: UDINT;
		END_VAR
		{wasm 'nop' (params IN) (result UDINT_TO_UINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION UDINT_TO_USINT: USINT
		VAR_INPUT
			IN: UDINT;
		END_VAR
		{wasm 'nop' (params IN) (result UDINT_TO_USINT)}
	END_FUNCTION

	FUNCTION UDINT_TO_LWORD: LWORD
		VAR_INPUT
			IN: UDINT;
		END_VAR
		{wasm 'i64.extend_i32_u' (params IN) (result UDINT_TO_LWORD)}
	END_FUNCTION

	FUNCTION UDINT_TO_DWORD: DWORD
		VAR_INPUT
			IN: UDINT;
		END_VAR
		{wasm 'nop' (params IN) (result UDINT_TO_DWORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION UDINT_TO_WORD: WORD
		VAR_INPUT
			IN: UDINT;
		END_VAR
		{wasm 'nop' (params IN) (result UDINT_TO_WORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION UDINT_TO_BYTE: BYTE
		VAR_INPUT
			IN: UDINT;
		END_VAR
		{wasm 'nop' (params IN) (result UDINT_TO_BYTE)}
	END_FUNCTION

	/* ## UINT / USINT — all i32 in WASM, identity casts */

	FUNCTION UINT_TO_INT: INT
		VAR_INPUT
			IN: UINT;
		END_VAR
		{wasm 'nop' (params IN) (result UINT_TO_INT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION UINT_TO_SINT: SINT
		VAR_INPUT
			IN: UINT;
		END_VAR
		{wasm 'nop' (params IN) (result UINT_TO_SINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION UINT_TO_USINT: USINT
		VAR_INPUT
			IN: UINT;
		END_VAR
		{wasm 'nop' (params IN) (result UINT_TO_USINT)}
	END_FUNCTION

	FUNCTION UINT_TO_LWORD: LWORD
		VAR_INPUT
			IN: UINT;
		END_VAR
		{wasm 'i64.extend_i32_u' (params IN) (result UINT_TO_LWORD)}
	END_FUNCTION

	FUNCTION UINT_TO_DWORD: DWORD
		VAR_INPUT
			IN: UINT;
		END_VAR
		{wasm 'nop' (params IN) (result UINT_TO_DWORD)}
	END_FUNCTION

	FUNCTION UINT_TO_WORD: WORD
		VAR_INPUT
			IN: UINT;
		END_VAR
		{wasm 'nop' (params IN) (result UINT_TO_WORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION UINT_TO_BYTE: BYTE
		VAR_INPUT
			IN: UINT;
		END_VAR
		{wasm 'nop' (params IN) (result UINT_TO_BYTE)}
	END_FUNCTION

	FUNCTION USINT_TO_SINT: SINT
		VAR_INPUT
			IN: USINT;
		END_VAR
		{wasm 'nop' (params IN) (result USINT_TO_SINT)}
	END_FUNCTION

	FUNCTION USINT_TO_LWORD: LWORD
		VAR_INPUT
			IN: USINT;
		END_VAR
		{wasm 'i64.extend_i32_u' (params IN) (result USINT_TO_LWORD)}
	END_FUNCTION

	FUNCTION USINT_TO_DWORD: DWORD
		VAR_INPUT
			IN: USINT;
		END_VAR
		{wasm 'nop' (params IN) (result USINT_TO_DWORD)}
	END_FUNCTION

	FUNCTION USINT_TO_WORD: WORD
		VAR_INPUT
			IN: USINT;
		END_VAR
		{wasm 'nop' (params IN) (result USINT_TO_WORD)}
	END_FUNCTION

	FUNCTION USINT_TO_BYTE: BYTE
		VAR_INPUT
			IN: USINT;
		END_VAR
		{wasm 'nop' (params IN) (result USINT_TO_BYTE)}
	END_FUNCTION

	/* ## LWORD (i64 bit) Conversions */

	FUNCTION LWORD_TO_LREAL: LREAL
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm 'f64.reinterpret_i64' (params IN) (result LWORD_TO_LREAL)}
	END_FUNCTION

	FUNCTION LWORD_TO_LINT: LINT
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm 'nop' (params IN) (result LWORD_TO_LINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LWORD_TO_DINT: DINT
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LWORD_TO_DINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LWORD_TO_INT: INT
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LWORD_TO_INT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LWORD_TO_SINT: SINT
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LWORD_TO_SINT)}
	END_FUNCTION

	FUNCTION LWORD_TO_ULINT: ULINT
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm 'nop' (params IN) (result LWORD_TO_ULINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LWORD_TO_UDINT: UDINT
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LWORD_TO_UDINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LWORD_TO_UINT: UINT
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LWORD_TO_UINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LWORD_TO_USINT: USINT
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LWORD_TO_USINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LWORD_TO_DWORD: DWORD
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LWORD_TO_DWORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LWORD_TO_WORD: WORD
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LWORD_TO_WORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LWORD_TO_BYTE: BYTE
		VAR_INPUT
			IN: LWORD;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LWORD_TO_BYTE)}
	END_FUNCTION

	/* ## DWORD (i32 bit) Conversions */

	FUNCTION DWORD_TO_REAL: REAL
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm 'f32.reinterpret_i32' (params IN) (result DWORD_TO_REAL)}
	END_FUNCTION

	FUNCTION DWORD_TO_LINT: LINT
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm 'i64.extend_i32_u' (params IN) (result DWORD_TO_LINT)}
	END_FUNCTION

	FUNCTION DWORD_TO_DINT: DINT
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm 'nop' (params IN) (result DWORD_TO_DINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION DWORD_TO_INT: INT
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm 'nop' (params IN) (result DWORD_TO_INT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION DWORD_TO_SINT: SINT
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm 'nop' (params IN) (result DWORD_TO_SINT)}
	END_FUNCTION

	FUNCTION DWORD_TO_ULINT: ULINT
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm 'i64.extend_i32_u' (params IN) (result DWORD_TO_ULINT)}
	END_FUNCTION

	FUNCTION DWORD_TO_UDINT: UDINT
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm 'nop' (params IN) (result DWORD_TO_UDINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION DWORD_TO_UINT: UINT
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm 'nop' (params IN) (result DWORD_TO_UINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION DWORD_TO_USINT: USINT
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm 'nop' (params IN) (result DWORD_TO_USINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION DWORD_TO_WORD: WORD
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm 'nop' (params IN) (result DWORD_TO_WORD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION DWORD_TO_BYTE: BYTE
		VAR_INPUT
			IN: DWORD;
		END_VAR
		{wasm 'nop' (params IN) (result DWORD_TO_BYTE)}
	END_FUNCTION

	/* ## WORD / BYTE — all i32 identity */

	FUNCTION WORD_TO_LINT: LINT
		VAR_INPUT
			IN: WORD;
		END_VAR
		{wasm 'i64.extend_i32_u' (params IN) (result WORD_TO_LINT)}
	END_FUNCTION

	FUNCTION WORD_TO_DINT: DINT
		VAR_INPUT
			IN: WORD;
		END_VAR
		{wasm 'nop' (params IN) (result WORD_TO_DINT)}
	END_FUNCTION

	FUNCTION WORD_TO_INT: INT
		VAR_INPUT
			IN: WORD;
		END_VAR
		{wasm 'nop' (params IN) (result WORD_TO_INT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION WORD_TO_SINT: SINT
		VAR_INPUT
			IN: WORD;
		END_VAR
		{wasm 'nop' (params IN) (result WORD_TO_SINT)}
	END_FUNCTION

	FUNCTION WORD_TO_ULINT: ULINT
		VAR_INPUT
			IN: WORD;
		END_VAR
		{wasm 'i64.extend_i32_u' (params IN) (result WORD_TO_ULINT)}
	END_FUNCTION

	FUNCTION WORD_TO_UDINT: UDINT
		VAR_INPUT
			IN: WORD;
		END_VAR
		{wasm 'nop' (params IN) (result WORD_TO_UDINT)}
	END_FUNCTION

	FUNCTION WORD_TO_UINT: UINT
		VAR_INPUT
			IN: WORD;
		END_VAR
		{wasm 'nop' (params IN) (result WORD_TO_UINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION WORD_TO_USINT: USINT
		VAR_INPUT
			IN: WORD;
		END_VAR
		{wasm 'nop' (params IN) (result WORD_TO_USINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION WORD_TO_BYTE: BYTE
		VAR_INPUT
			IN: WORD;
		END_VAR
		{wasm 'nop' (params IN) (result WORD_TO_BYTE)}
	END_FUNCTION

	FUNCTION BYTE_TO_LINT: LINT
		VAR_INPUT
			IN: BYTE;
		END_VAR
		{wasm 'i64.extend_i32_u' (params IN) (result BYTE_TO_LINT)}
	END_FUNCTION

	FUNCTION BYTE_TO_DINT: DINT
		VAR_INPUT
			IN: BYTE;
		END_VAR
		{wasm 'nop' (params IN) (result BYTE_TO_DINT)}
	END_FUNCTION

	FUNCTION BYTE_TO_INT: INT
		VAR_INPUT
			IN: BYTE;
		END_VAR
		{wasm 'nop' (params IN) (result BYTE_TO_INT)}
	END_FUNCTION

	FUNCTION BYTE_TO_SINT: SINT
		VAR_INPUT
			IN: BYTE;
		END_VAR
		{wasm 'nop' (params IN) (result BYTE_TO_SINT)}
	END_FUNCTION

	FUNCTION BYTE_TO_ULINT: ULINT
		VAR_INPUT
			IN: BYTE;
		END_VAR
		{wasm 'i64.extend_i32_u' (params IN) (result BYTE_TO_ULINT)}
	END_FUNCTION

	FUNCTION BYTE_TO_UDINT: UDINT
		VAR_INPUT
			IN: BYTE;
		END_VAR
		{wasm 'nop' (params IN) (result BYTE_TO_UDINT)}
	END_FUNCTION

	FUNCTION BYTE_TO_UINT: UINT
		VAR_INPUT
			IN: BYTE;
		END_VAR
		{wasm 'nop' (params IN) (result BYTE_TO_UINT)}
	END_FUNCTION

	FUNCTION BYTE_TO_USINT: USINT
		VAR_INPUT
			IN: BYTE;
		END_VAR
		{wasm 'nop' (params IN) (result BYTE_TO_USINT)}
	END_FUNCTION

	/* ## BOOL Conversions — BOOL is i32 (0 or 1) */

	FUNCTION BOOL_TO_LINT: LINT
		VAR_INPUT
			IN: BOOL;
		END_VAR
		{wasm 'i64.extend_i32_u' (params IN) (result BOOL_TO_LINT)}
	END_FUNCTION

	FUNCTION BOOL_TO_DINT: DINT
		VAR_INPUT
			IN: BOOL;
		END_VAR
		{wasm 'nop' (params IN) (result BOOL_TO_DINT)}
	END_FUNCTION

	FUNCTION BOOL_TO_INT: INT
		VAR_INPUT
			IN: BOOL;
		END_VAR
		{wasm 'nop' (params IN) (result BOOL_TO_INT)}
	END_FUNCTION

	FUNCTION BOOL_TO_SINT: SINT
		VAR_INPUT
			IN: BOOL;
		END_VAR
		{wasm 'nop' (params IN) (result BOOL_TO_SINT)}
	END_FUNCTION

	FUNCTION BOOL_TO_ULINT: ULINT
		VAR_INPUT
			IN: BOOL;
		END_VAR
		{wasm 'i64.extend_i32_u' (params IN) (result BOOL_TO_ULINT)}
	END_FUNCTION

	FUNCTION BOOL_TO_UDINT: UDINT
		VAR_INPUT
			IN: BOOL;
		END_VAR
		{wasm 'nop' (params IN) (result BOOL_TO_UDINT)}
	END_FUNCTION

	FUNCTION BOOL_TO_UINT: UINT
		VAR_INPUT
			IN: BOOL;
		END_VAR
		{wasm 'nop' (params IN) (result BOOL_TO_UINT)}
	END_FUNCTION

	FUNCTION BOOL_TO_USINT: USINT
		VAR_INPUT
			IN: BOOL;
		END_VAR
		{wasm 'nop' (params IN) (result BOOL_TO_USINT)}
	END_FUNCTION

	/* ## Tests */

	NAMESPACE Test
		USING Std.Unit

		// Float → Int (truncation)
		{test}
		FUNCTION test_real_to_int
			VAR
				r: INT;
			END_VAR
			r := REAL_TO_INT(3.7);
			ASSERT(r = 3);
		END_FUNCTION

		{test}
		FUNCTION test_real_to_int_negative
			VAR
				r: INT;
			END_VAR
			r := REAL_TO_INT(-3.7);
			ASSERT(r = -3);
		END_FUNCTION

		{test}
		FUNCTION test_lreal_to_dint
			VAR
				r: DINT;
			END_VAR
			r := LREAL_TO_DINT(42.9);
			ASSERT(r = 42);
		END_FUNCTION

		{test}
		FUNCTION test_lreal_to_lint
			VAR
				r: LINT;
			END_VAR
			r := LREAL_TO_LINT(1000000.5);
			ASSERT(r = 1000000);
		END_FUNCTION

		// Int → Float (promotion)
		{test}
		FUNCTION test_dint_to_real
			VAR
				r: REAL;
			END_VAR
			r := DINT_TO_REAL(100);
			ASSERT(r = 100.0);
		END_FUNCTION

		{test}
		FUNCTION test_lint_to_lreal
			VAR
				r: LREAL;
			END_VAR
			r := LINT_TO_LREAL(42);
			ASSERT(r = 42.0);
		END_FUNCTION

		// Float precision change
		{test}
		FUNCTION test_lreal_to_real
			VAR
				r: REAL;
			END_VAR
			r := LREAL_TO_REAL(3.14);
			ASSERT(r = 3.14);
		END_FUNCTION

		// Integer narrowing
		{test}
		FUNCTION test_lint_to_int
			VAR
				r: INT;
			END_VAR
			r := LINT_TO_INT(42);
			ASSERT(r = 42);
		END_FUNCTION

		{test}
		FUNCTION test_lint_to_dint
			VAR
				r: DINT;
			END_VAR
			r := LINT_TO_DINT(12345);
			ASSERT(r = 12345);
		END_FUNCTION

		// Same-width signed ↔ unsigned
		{test}
		FUNCTION test_dint_to_udint
			VAR
				r: UDINT;
			END_VAR
			r := DINT_TO_UDINT(255);
			ASSERT(r = 255);
		END_FUNCTION

		{test}
		FUNCTION test_uint_to_int
			VAR
				r: INT;
			END_VAR
			r := UINT_TO_INT(100);
			ASSERT(r = 100);
		END_FUNCTION

		// Bit type conversions
		{test}
		FUNCTION test_byte_to_int
			VAR
				r: INT;
			END_VAR
			r := BYTE_TO_INT(16#FF);
			ASSERT(r = 255);
		END_FUNCTION

		{test}
		FUNCTION test_word_to_dint
			VAR
				r: DINT;
			END_VAR
			r := WORD_TO_DINT(16#ABCD);
			ASSERT(r = 16#ABCD);
		END_FUNCTION

		// Bool conversions
		{test}
		FUNCTION test_bool_to_int_true
			VAR
				r: INT;
			END_VAR
			r := BOOL_TO_INT(TRUE);
			ASSERT(r = 1);
		END_FUNCTION

		{test}
		FUNCTION test_bool_to_int_false
			VAR
				r: INT;
			END_VAR
			r := BOOL_TO_INT(FALSE);
			ASSERT(r = 0);
		END_FUNCTION

		// Unsigned float conversions
		{test}
		FUNCTION test_udint_to_real
			VAR
				r: REAL;
			END_VAR
			r := UDINT_TO_REAL(50);
			ASSERT(r = 50.0);
		END_FUNCTION

		// Chained conversions
		{test}
		FUNCTION test_chained_conversion
			VAR
				a: REAL;
				b: INT;
				c: REAL;
			END_VAR
			a := 42.7;
			b := REAL_TO_INT(a);
			c := DINT_TO_REAL(b);
			ASSERT(c = 42.0);
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
