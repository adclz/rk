/*
# Standard Type Conversion Functions

IEC 61131-3 **Table 22** - Explicit type conversion (narrowing casts).

These functions handle conversions that are **not** covered by implicit casting,
typically narrowing conversions where precision or range may be lost.

Each function follows the naming convention `<SOURCE>_TO_<TARGET>`.
All conversions use {wasm} intrinsics - zero overhead, single instruction.
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
	// LINT to unsigned: same i64 bit pattern
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
	// LINT to bit types: reinterpret
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
	// DINT to smaller signed: identity (all i32 in WASM)
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

	// DINT to unsigned: same i32 bit pattern
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
	// DINT to bit types
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

	/* ## INT (i32 signed, 16-bit semantic) - all i32 in WASM */

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

	/* ## UINT / USINT - all i32 in WASM, identity casts */

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

	/* ## WORD / BYTE - all i32 identity */

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

	/* ## BOOL Conversions - BOOL is i32 (0 or 1) */

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

	/* ## Duration narrowings (LTIME / LDATE / LTOD / LDT → 32-bit) */

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LTIME_TO_TIME: TIME
		VAR_INPUT
			IN: LTIME;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LTIME_TO_TIME)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LDATE_TO_DATE: DATE
		VAR_INPUT
			IN: LDATE;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LDATE_TO_DATE)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LTOD_TO_TOD: TOD
		VAR_INPUT
			IN: LTOD;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LTOD_TO_TOD)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LDT_TO_DT: DATE_AND_TIME
		VAR_INPUT
			IN: LDATE_AND_TIME;
		END_VAR
		{wasm 'i32.wrap_i64' (params IN) (result LDT_TO_DT)}
	END_FUNCTION

	/* ## Date / time-of-day decompositions

	   `DT` and `LDT` carry both a date and a time-of-day component. With
	   the integer encoding (`DT` = i32 secs since 1970, `LDT` = i64 ns
	   since 1970), each extraction is a single division or modulo: the
	   codegen lowers these casts directly, no host import required.
	*/

	FUNCTION DT_TO_DATE: DATE
		VAR_INPUT IN: DATE_AND_TIME; END_VAR
		{wasm 'cast' (params IN) (result DT_TO_DATE)}
	END_FUNCTION

	FUNCTION DT_TO_LDATE: LDATE
		VAR_INPUT IN: DATE_AND_TIME; END_VAR
		{wasm 'cast' (params IN) (result DT_TO_LDATE)}
	END_FUNCTION

	FUNCTION DT_TO_TOD: TIME_OF_DAY
		VAR_INPUT IN: DATE_AND_TIME; END_VAR
		{wasm 'cast' (params IN) (result DT_TO_TOD)}
	END_FUNCTION

	FUNCTION DT_TO_LTOD: LTIME_OF_DAY
		VAR_INPUT IN: DATE_AND_TIME; END_VAR
		{wasm 'cast' (params IN) (result DT_TO_LTOD)}
	END_FUNCTION

	FUNCTION LDT_TO_DATE: DATE
		VAR_INPUT IN: LDATE_AND_TIME; END_VAR
		{wasm 'cast' (params IN) (result LDT_TO_DATE)}
	END_FUNCTION

	FUNCTION LDT_TO_LDATE: LDATE
		VAR_INPUT IN: LDATE_AND_TIME; END_VAR
		{wasm 'cast' (params IN) (result LDT_TO_LDATE)}
	END_FUNCTION

	FUNCTION LDT_TO_TOD: TIME_OF_DAY
		VAR_INPUT IN: LDATE_AND_TIME; END_VAR
		{wasm 'cast' (params IN) (result LDT_TO_TOD)}
	END_FUNCTION

	FUNCTION LDT_TO_LTOD: LTIME_OF_DAY
		VAR_INPUT IN: LDATE_AND_TIME; END_VAR
		{wasm 'cast' (params IN) (result LDT_TO_LTOD)}
	END_FUNCTION

	/* ## CHAR conversions */

	FUNCTION BYTE_TO_CHAR: CHAR
		VAR_INPUT IN: BYTE; END_VAR
		{wasm 'nop' (params IN) (result BYTE_TO_CHAR)}
	END_FUNCTION

	/* ## CHAR_TO_STRING — encode a CHAR (UTF-32 code point) as a STRING.

	   The only path from a CHAR scalar to a STRING-shaped value. CHAR is
	   stored as an i32 code point at the WASM ABI; this writes the UTF-8
	   encoding (1-4 bytes) into the function's STRING return slot.

	   Invalid code points (surrogates, > U+10FFFF) yield an empty string. */
	FUNCTION CHAR_TO_STRING: STRING
		VAR_INPUT
			IN: CHAR;
		END_VAR
		{wasm 'rk.str_from_char' (params IN) (result CHAR_TO_STRING)}
	END_FUNCTION

	/* ## Scalar → STRING conversions

	   Each function formats its input as decimal text (or `"TRUE"`/`"FALSE"`
	   for BOOL) into a fresh STRING return slot. Truncation is silent if
	   the result exceeds the default STRING capacity. */

	FUNCTION BOOL_TO_STRING: STRING
		VAR_INPUT IN: BOOL; END_VAR
		{wasm 'rk.str_from_bool' (params IN) (result BOOL_TO_STRING)}
	END_FUNCTION

	FUNCTION SINT_TO_STRING: STRING
		VAR_INPUT IN: SINT; END_VAR
		{wasm 'rk.str_from_i32' (params IN) (result SINT_TO_STRING)}
	END_FUNCTION

	FUNCTION INT_TO_STRING: STRING
		VAR_INPUT IN: INT; END_VAR
		{wasm 'rk.str_from_i32' (params IN) (result INT_TO_STRING)}
	END_FUNCTION

	FUNCTION DINT_TO_STRING: STRING
		VAR_INPUT IN: DINT; END_VAR
		{wasm 'rk.str_from_i32' (params IN) (result DINT_TO_STRING)}
	END_FUNCTION

	FUNCTION LINT_TO_STRING: STRING
		VAR_INPUT IN: LINT; END_VAR
		{wasm 'rk.str_from_i64' (params IN) (result LINT_TO_STRING)}
	END_FUNCTION

	FUNCTION USINT_TO_STRING: STRING
		VAR_INPUT IN: USINT; END_VAR
		{wasm 'rk.str_from_u32' (params IN) (result USINT_TO_STRING)}
	END_FUNCTION

	FUNCTION UINT_TO_STRING: STRING
		VAR_INPUT IN: UINT; END_VAR
		{wasm 'rk.str_from_u32' (params IN) (result UINT_TO_STRING)}
	END_FUNCTION

	FUNCTION UDINT_TO_STRING: STRING
		VAR_INPUT IN: UDINT; END_VAR
		{wasm 'rk.str_from_u32' (params IN) (result UDINT_TO_STRING)}
	END_FUNCTION

	FUNCTION ULINT_TO_STRING: STRING
		VAR_INPUT IN: ULINT; END_VAR
		{wasm 'rk.str_from_u64' (params IN) (result ULINT_TO_STRING)}
	END_FUNCTION

	FUNCTION BYTE_TO_STRING: STRING
		VAR_INPUT IN: BYTE; END_VAR
		{wasm 'rk.str_from_u32' (params IN) (result BYTE_TO_STRING)}
	END_FUNCTION

	FUNCTION WORD_TO_STRING: STRING
		VAR_INPUT IN: WORD; END_VAR
		{wasm 'rk.str_from_u32' (params IN) (result WORD_TO_STRING)}
	END_FUNCTION

	FUNCTION DWORD_TO_STRING: STRING
		VAR_INPUT IN: DWORD; END_VAR
		{wasm 'rk.str_from_u32' (params IN) (result DWORD_TO_STRING)}
	END_FUNCTION

	FUNCTION LWORD_TO_STRING: STRING
		VAR_INPUT IN: LWORD; END_VAR
		{wasm 'rk.str_from_u64' (params IN) (result LWORD_TO_STRING)}
	END_FUNCTION

	FUNCTION REAL_TO_STRING: STRING
		VAR_INPUT IN: REAL; END_VAR
		{wasm 'rk.str_from_f32' (params IN) (result REAL_TO_STRING)}
	END_FUNCTION

	FUNCTION LREAL_TO_STRING: STRING
		VAR_INPUT IN: LREAL; END_VAR
		{wasm 'rk.str_from_f64' (params IN) (result LREAL_TO_STRING)}
	END_FUNCTION

	/* ## ANY_TO_STRING - generic textual formatter

	   Dispatches to the concrete `*_TO_STRING` for any elementary type with
	   a formatter. STRING inputs pass through unchanged. Types without a
	   formatter (DATE / TIME / DT / TOD families) fall through to the
	   default empty result. */
	FUNCTION ANY_TO_STRING: STRING
		VAR_INPUT IN: ANY; END_VAR

		ANY_TO_STRING := '';

		{#if IN is BOOL}
			ANY_TO_STRING := BOOL_TO_STRING(IN);
		{#elif IN is SINT}
			ANY_TO_STRING := SINT_TO_STRING(IN);
		{#elif IN is INT}
			ANY_TO_STRING := INT_TO_STRING(IN);
		{#elif IN is DINT}
			ANY_TO_STRING := DINT_TO_STRING(IN);
		{#elif IN is LINT}
			ANY_TO_STRING := LINT_TO_STRING(IN);
		{#elif IN is USINT}
			ANY_TO_STRING := USINT_TO_STRING(IN);
		{#elif IN is UINT}
			ANY_TO_STRING := UINT_TO_STRING(IN);
		{#elif IN is UDINT}
			ANY_TO_STRING := UDINT_TO_STRING(IN);
		{#elif IN is ULINT}
			ANY_TO_STRING := ULINT_TO_STRING(IN);
		{#elif IN is BYTE}
			ANY_TO_STRING := BYTE_TO_STRING(IN);
		{#elif IN is WORD}
			ANY_TO_STRING := WORD_TO_STRING(IN);
		{#elif IN is DWORD}
			ANY_TO_STRING := DWORD_TO_STRING(IN);
		{#elif IN is LWORD}
			ANY_TO_STRING := LWORD_TO_STRING(IN);
		{#elif IN is REAL}
			ANY_TO_STRING := REAL_TO_STRING(IN);
		{#elif IN is LREAL}
			ANY_TO_STRING := LREAL_TO_STRING(IN);
		{#elif IN is CHAR}
			ANY_TO_STRING := CHAR_TO_STRING(IN);
		{#elif IN is STRING}
			ANY_TO_STRING := IN;
		{#endif}

	END_FUNCTION

	/* ## Tests */

	NAMESPACE Test
		USING Std.Unit
		USING Std.Strings

		// Float to Int (truncation)
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

		// Int to Float (promotion)
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

		// ----- Scalar → STRING ------------------------------------------------------------------------------

		{test}
		FUNCTION test_bool_to_string_true
			VAR s: STRING; END_VAR
			s := BOOL_TO_STRING(TRUE);
			ASSERT(LEN(s) = 4);
			ASSERT(FIND(s, 'TRUE') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_bool_to_string_false
			VAR s: STRING; END_VAR
			s := BOOL_TO_STRING(FALSE);
			ASSERT(LEN(s) = 5);
			ASSERT(FIND(s, 'FALSE') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_int_to_string_positive
			VAR s: STRING; END_VAR
			s := INT_TO_STRING(42);
			ASSERT(LEN(s) = 2);
			ASSERT(FIND(s, '42') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_int_to_string_zero
			VAR s: STRING; END_VAR
			s := INT_TO_STRING(0);
			ASSERT(LEN(s) = 1);
			ASSERT(FIND(s, '0') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_int_to_string_negative
			VAR s: STRING; END_VAR
			s := INT_TO_STRING(-7);
			ASSERT(LEN(s) = 2);
			ASSERT(FIND(s, '-7') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_dint_to_string_large
			VAR s: STRING; END_VAR
			s := DINT_TO_STRING(2147483647);
			ASSERT(LEN(s) = 10);
			ASSERT(FIND(s, '2147483647') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_dint_to_string_negative_large
			VAR s: STRING; END_VAR
			s := DINT_TO_STRING(-2147483647);
			ASSERT(LEN(s) = 11);
			ASSERT(FIND(s, '-2147483647') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_lint_to_string_large
			VAR s: STRING; END_VAR
			s := LINT_TO_STRING(LINT#9223372036854775807);
			ASSERT(LEN(s) = 19);
			ASSERT(FIND(s, '9223372036854775807') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_uint_to_string
			VAR s: STRING; END_VAR
			s := UINT_TO_STRING(65535);
			ASSERT(LEN(s) = 5);
			ASSERT(FIND(s, '65535') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_udint_to_string
			VAR s: STRING; END_VAR
			s := UDINT_TO_STRING(1000000);
			ASSERT(LEN(s) = 7);
			ASSERT(FIND(s, '1000000') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_byte_to_string
			VAR s: STRING; END_VAR
			s := BYTE_TO_STRING(255);
			ASSERT(LEN(s) = 3);
			ASSERT(FIND(s, '255') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_real_to_string_basic
			VAR s: STRING; END_VAR
			s := REAL_TO_STRING(1.5);
			// "1.500000" — 8 bytes
			ASSERT(LEN(s) = 8);
			ASSERT(FIND(s, '1.500000') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_real_to_string_negative
			VAR s: STRING; END_VAR
			s := REAL_TO_STRING(-3.25);
			ASSERT(FIND(s, '-3.250000') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_real_to_string_zero
			VAR s: STRING; END_VAR
			s := REAL_TO_STRING(0.0);
			ASSERT(FIND(s, '0.000000') = 1);
		END_FUNCTION

		{test}
		FUNCTION test_lreal_to_string_basic
			VAR s: STRING; END_VAR
			s := LREAL_TO_STRING(2.5);
			ASSERT(FIND(s, '2.500000') = 1);
		END_FUNCTION

		// Composition with string ops — convert and concatenate.
		{test}
		FUNCTION test_int_to_string_in_concat
			VAR s: STRING; END_VAR
			s := CONCAT('Temperature: ', INT_TO_STRING(42));
			ASSERT(FIND(s, 'Temperature: 42') = 1);
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
