/*
# Standard Type Conversion Functions

IEC 61131-3 **Table 22** - Explicit type conversion (narrowing casts).

These functions handle conversions that are **not** covered by implicit casting,
typically narrowing conversions where precision or range may be lost.

Each function follows the naming convention `<SOURCE>_TO_<TARGET>`.
Most are one {wasm} instruction. The float-to-integer ones are two, round
to nearest and then convert with saturation, as described at their section.
*/
NAMESPACE Std.Convert
	USING Std.Strings

	/* ## LREAL (f64) Conversions */

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_REAL: REAL
		VAR_INPUT
			IN: LREAL;
		END_VAR
		{wasm 'f32.demote_f64' (params IN) (result LREAL_TO_REAL)}
	END_FUNCTION

	/*
	## Rounding and saturation
	A float becomes an integer the way IEC 61131-3 says: rounded to the nearest
	integer, halfway cases to the even neighbour (2.5 gives 2, 3.5 gives 4).
	A value past the target's range saturates to its nearest bound; NaN becomes
	0. `TRUNC` alone truncates toward zero. Nothing here can stop the scan:
	the raw wasm truncation trapped on NaN and past 2^31, and its narrowing
	wrapped 40000.0 to -25536. Two instructions each: `nearest` picks the
	integer, `trunc_sat` moves it into the integer lane; the sub-width targets
	clamp between the two, since wasm only saturates at 32 and 64 bits.
	*/
	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_LINT: LINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		VAR
			r: LREAL;
		END_VAR
		{wasm 'f64.nearest' (params IN) (result r)}
		{wasm 'i64.trunc_sat_f64_s' (params r) (result LREAL_TO_LINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_DINT: DINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		VAR
			r: LREAL;
		END_VAR
		{wasm 'f64.nearest' (params IN) (result r)}
		{wasm 'i32.trunc_sat_f64_s' (params r) (result LREAL_TO_DINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_INT: INT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		VAR
			r: LREAL;
		END_VAR
		{wasm 'f64.nearest' (params IN) (result r)}
		IF r >= 32767.0 THEN
			LREAL_TO_INT := INT#32767;
		ELSIF r <= -32768.0 THEN
			LREAL_TO_INT := INT#-32768;
		ELSE
			{wasm 'i32.trunc_sat_f64_s' (params r) (result LREAL_TO_INT)}
		END_IF;
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_SINT: SINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		VAR
			r: LREAL;
		END_VAR
		{wasm 'f64.nearest' (params IN) (result r)}
		IF r >= 127.0 THEN
			LREAL_TO_SINT := SINT#127;
		ELSIF r <= -128.0 THEN
			LREAL_TO_SINT := SINT#-128;
		ELSE
			{wasm 'i32.trunc_sat_f64_s' (params r) (result LREAL_TO_SINT)}
		END_IF;
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_ULINT: ULINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		VAR
			r: LREAL;
		END_VAR
		{wasm 'f64.nearest' (params IN) (result r)}
		{wasm 'i64.trunc_sat_f64_u' (params r) (result LREAL_TO_ULINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_UDINT: UDINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		VAR
			r: LREAL;
		END_VAR
		{wasm 'f64.nearest' (params IN) (result r)}
		{wasm 'i32.trunc_sat_f64_u' (params r) (result LREAL_TO_UDINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_UINT: UINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		VAR
			r: LREAL;
		END_VAR
		{wasm 'f64.nearest' (params IN) (result r)}
		IF r >= 65535.0 THEN
			LREAL_TO_UINT := UINT#65535;
		ELSE
			{wasm 'i32.trunc_sat_f64_u' (params r) (result LREAL_TO_UINT)}
		END_IF;
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION LREAL_TO_USINT: USINT
		VAR_INPUT
			IN: LREAL;
		END_VAR
		VAR
			r: LREAL;
		END_VAR
		{wasm 'f64.nearest' (params IN) (result r)}
		IF r >= 255.0 THEN
			LREAL_TO_USINT := USINT#255;
		ELSE
			{wasm 'i32.trunc_sat_f64_u' (params r) (result LREAL_TO_USINT)}
		END_IF;
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
		VAR
			r: REAL;
		END_VAR
		{wasm 'f32.nearest' (params IN) (result r)}
		{wasm 'i64.trunc_sat_f32_s' (params r) (result REAL_TO_LINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_DINT: DINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		VAR
			r: REAL;
		END_VAR
		{wasm 'f32.nearest' (params IN) (result r)}
		{wasm 'i32.trunc_sat_f32_s' (params r) (result REAL_TO_DINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_INT: INT
		VAR_INPUT
			IN: REAL;
		END_VAR
		VAR
			r: REAL;
		END_VAR
		{wasm 'f32.nearest' (params IN) (result r)}
		IF r >= 32767.0 THEN
			REAL_TO_INT := INT#32767;
		ELSIF r <= -32768.0 THEN
			REAL_TO_INT := INT#-32768;
		ELSE
			{wasm 'i32.trunc_sat_f32_s' (params r) (result REAL_TO_INT)}
		END_IF;
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_SINT: SINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		VAR
			r: REAL;
		END_VAR
		{wasm 'f32.nearest' (params IN) (result r)}
		IF r >= 127.0 THEN
			REAL_TO_SINT := SINT#127;
		ELSIF r <= -128.0 THEN
			REAL_TO_SINT := SINT#-128;
		ELSE
			{wasm 'i32.trunc_sat_f32_s' (params r) (result REAL_TO_SINT)}
		END_IF;
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_ULINT: ULINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		VAR
			r: REAL;
		END_VAR
		{wasm 'f32.nearest' (params IN) (result r)}
		{wasm 'i64.trunc_sat_f32_u' (params r) (result REAL_TO_ULINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_UDINT: UDINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		VAR
			r: REAL;
		END_VAR
		{wasm 'f32.nearest' (params IN) (result r)}
		{wasm 'i32.trunc_sat_f32_u' (params r) (result REAL_TO_UDINT)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_UINT: UINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		VAR
			r: REAL;
		END_VAR
		{wasm 'f32.nearest' (params IN) (result r)}
		IF r >= 65535.0 THEN
			REAL_TO_UINT := UINT#65535;
		ELSE
			{wasm 'i32.trunc_sat_f32_u' (params r) (result REAL_TO_UINT)}
		END_IF;
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION REAL_TO_USINT: USINT
		VAR_INPUT
			IN: REAL;
		END_VAR
		VAR
			r: REAL;
		END_VAR
		{wasm 'f32.nearest' (params IN) (result r)}
		IF r >= 255.0 THEN
			REAL_TO_USINT := USINT#255;
		ELSE
			{wasm 'i32.trunc_sat_f32_u' (params r) (result REAL_TO_USINT)}
		END_IF;
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

	/* ## CHAR conversions
	   A CHAR is one character, its code point in an i32. BYTE_TO_CHAR reads
	   a byte as the Latin-1 character it is; CHAR_TO_BYTE keeps the low byte,
	   which is the whole character for the Latin-1 range and a truncation
	   past it. */

	FUNCTION BYTE_TO_CHAR: CHAR
		VAR_INPUT IN: BYTE; END_VAR
		{wasm 'nop' (params IN) (result BYTE_TO_CHAR)}
	END_FUNCTION

	{info = 'narrowing conversion, possible loss of value'}
	FUNCTION CHAR_TO_BYTE: BYTE
		VAR_INPUT IN: CHAR; END_VAR
		{wasm 'cast' (params IN) (result CHAR_TO_BYTE)}
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
	   the result exceeds the default STRING capacity.

	   REAL_TO_STRING and LREAL_TO_STRING print the shortest text that reads
	   back to the same value, spelled as a REAL literal: '1.5', '50.1',
	   '100.0', '1.0E20', '2.5E-9'. A REAL is formatted as the REAL it is, so
	   50.1 is '50.1' and not its f64 widening '50.099998'; an LREAL shows
	   every digit its value has, so 0.1 + 0.2 is '0.30000000000000004'.
	   'NaN', 'Inf' and '-Inf' spell the values no literal can. */

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

	/* ## Tests */


	/*
    ## TRUNC - Truncation toward zero

    The integer part of a real value, discarding the fraction:
    `TRUNC(3.7)` is 3 and `TRUNC(-3.7)` is -3. A `REAL` truncates to
    `DINT`, an `LREAL` to `LINT` - the same conversions `REAL_TO_DINT`
    and `LREAL_TO_LINT` perform.
    */
	FUNCTION TRUNC: DINT
		VAR_INPUT
			IN: REAL;
		END_VAR

		{wasm 'i32.trunc_sat_f32_s' (params IN) (result TRUNC)}
	END_FUNCTION

	FUNCTION TRUNC: LINT
		VAR_INPUT
			IN: LREAL;
		END_VAR

		{wasm 'i64.trunc_sat_f64_s' (params IN) (result TRUNC)}
	END_FUNCTION


	/* ## Date/time widenings, as named functions (the implicit casts' twins) */

	FUNCTION TIME_TO_LTIME: LTIME
		VAR_INPUT IN: TIME; END_VAR
		{wasm 'cast' (params IN) (result TIME_TO_LTIME)}
	END_FUNCTION

	FUNCTION TOD_TO_LTOD: LTIME_OF_DAY
		VAR_INPUT IN: TIME_OF_DAY; END_VAR
		{wasm 'cast' (params IN) (result TOD_TO_LTOD)}
	END_FUNCTION

	FUNCTION DATE_TO_LDATE: LDATE
		VAR_INPUT IN: DATE; END_VAR
		{wasm 'cast' (params IN) (result DATE_TO_LDATE)}
	END_FUNCTION

	FUNCTION DT_TO_LDT: LDATE_AND_TIME
		VAR_INPUT IN: DATE_AND_TIME; END_VAR
		{wasm 'cast' (params IN) (result DT_TO_LDT)}
	END_FUNCTION

	/* ## Date/time to number and back.

	Each exposes the documented encoding as a plain number: TIME/TOD in ms,
	DATE in days since 1970-01-01, DT in seconds since 1970-01-01, and the
	L-variants in ns (LDATE in days). Same-lane reinterpretations, zero cost.
	Until the date/time arithmetic functions exist, this is the sanctioned
	workaround: convert, compute in the encoding, convert back. */

	FUNCTION TIME_TO_DINT: DINT
		VAR_INPUT IN: TIME; END_VAR
		{wasm 'cast' (params IN) (result TIME_TO_DINT)}
	END_FUNCTION

	FUNCTION DINT_TO_TIME: TIME
		VAR_INPUT IN: DINT; END_VAR
		{wasm 'cast' (params IN) (result DINT_TO_TIME)}
	END_FUNCTION

	FUNCTION LTIME_TO_LINT: LINT
		VAR_INPUT IN: LTIME; END_VAR
		{wasm 'cast' (params IN) (result LTIME_TO_LINT)}
	END_FUNCTION

	FUNCTION LINT_TO_LTIME: LTIME
		VAR_INPUT IN: LINT; END_VAR
		{wasm 'cast' (params IN) (result LINT_TO_LTIME)}
	END_FUNCTION

	FUNCTION DATE_TO_DINT: DINT
		VAR_INPUT IN: DATE; END_VAR
		{wasm 'cast' (params IN) (result DATE_TO_DINT)}
	END_FUNCTION

	FUNCTION DINT_TO_DATE: DATE
		VAR_INPUT IN: DINT; END_VAR
		{wasm 'cast' (params IN) (result DINT_TO_DATE)}
	END_FUNCTION

	FUNCTION LDATE_TO_LINT: LINT
		VAR_INPUT IN: LDATE; END_VAR
		{wasm 'cast' (params IN) (result LDATE_TO_LINT)}
	END_FUNCTION

	FUNCTION LINT_TO_LDATE: LDATE
		VAR_INPUT IN: LINT; END_VAR
		{wasm 'cast' (params IN) (result LINT_TO_LDATE)}
	END_FUNCTION

	FUNCTION TOD_TO_DINT: DINT
		VAR_INPUT IN: TIME_OF_DAY; END_VAR
		{wasm 'cast' (params IN) (result TOD_TO_DINT)}
	END_FUNCTION

	FUNCTION DINT_TO_TOD: TIME_OF_DAY
		VAR_INPUT IN: DINT; END_VAR
		{wasm 'cast' (params IN) (result DINT_TO_TOD)}
	END_FUNCTION

	FUNCTION LTOD_TO_LINT: LINT
		VAR_INPUT IN: LTIME_OF_DAY; END_VAR
		{wasm 'cast' (params IN) (result LTOD_TO_LINT)}
	END_FUNCTION

	FUNCTION LINT_TO_LTOD: LTIME_OF_DAY
		VAR_INPUT IN: LINT; END_VAR
		{wasm 'cast' (params IN) (result LINT_TO_LTOD)}
	END_FUNCTION

	FUNCTION DT_TO_LINT: LINT
		VAR_INPUT IN: DATE_AND_TIME; END_VAR
		{wasm 'cast' (params IN) (result DT_TO_LINT)}
	END_FUNCTION

	FUNCTION LINT_TO_DT: DATE_AND_TIME
		VAR_INPUT IN: LINT; END_VAR
		{wasm 'cast' (params IN) (result LINT_TO_DT)}
	END_FUNCTION

	FUNCTION LDT_TO_LINT: LINT
		VAR_INPUT IN: LDATE_AND_TIME; END_VAR
		{wasm 'cast' (params IN) (result LDT_TO_LINT)}
	END_FUNCTION

	FUNCTION LINT_TO_LDT: LDATE_AND_TIME
		VAR_INPUT IN: LINT; END_VAR
		{wasm 'cast' (params IN) (result LINT_TO_LDT)}
	END_FUNCTION

	/* ## Date/time to STRING, in IEC literal form.

	Every result parses back as a literal of the same type and value, so the
	canonical spelling may differ from how the value was written (T#90s prints
	as 'T#1m30s'). A negative TOD only arises from a pre-epoch decomposition
	(a domain escape, see the compiler's emit_datetime_cast); it prints with a
	leading '-', which no TOD literal accepts, so the escape stays visible. */

	FUNCTION PRIVATE PAD_MIN: STRING
		VAR_INPUT n: LINT; width: UDINT; END_VAR
		VAR s: STRING; END_VAR
		s := LINT_TO_STRING(IN := n);
		WHILE LEN(IN := s) < width DO
			s := CONCAT(A := '0', B := s);
		END_WHILE
		PAD_MIN := s;
	END_FUNCTION

	/* days since 1970-01-01 to 'YYYY-MM-DD' (proleptic Gregorian, era-based
	civil algorithm; the explicit era adjustment makes truncating division
	behave as floor, so pre-epoch days are exact) */
	FUNCTION PRIVATE CIVIL_YMD: STRING
		VAR_INPUT days: LINT; END_VAR
		VAR z, era, doe, yoe, y, doy, mp, dd, mm: LINT; s: STRING; END_VAR
		z := days + LINT#719468;
		IF z >= LINT#0 THEN
			era := z / LINT#146097;
		ELSE
			era := (z - LINT#146096) / LINT#146097;
		END_IF
		doe := z - era * LINT#146097;
		yoe := (doe - doe / LINT#1460 + doe / LINT#36524 - doe / LINT#146096) / LINT#365;
		y := yoe + era * LINT#400;
		doy := doe - (LINT#365 * yoe + yoe / LINT#4 - yoe / LINT#100);
		mp := (LINT#5 * doy + LINT#2) / LINT#153;
		dd := doy - (LINT#153 * mp + LINT#2) / LINT#5 + LINT#1;
		IF mp < LINT#10 THEN
			mm := mp + LINT#3;
		ELSE
			mm := mp - LINT#9;
		END_IF
		IF mm <= LINT#2 THEN
			y := y + LINT#1;
		END_IF
		IF y < LINT#0 THEN
			s := CONCAT(A := '-', B := PAD_MIN(n := -y, width := UDINT#4));
		ELSE
			s := PAD_MIN(n := y, width := UDINT#4);
		END_IF
		s := CONCAT(A := s, B := '-');
		s := CONCAT(A := s, B := PAD_MIN(n := mm, width := UDINT#2));
		s := CONCAT(A := s, B := '-');
		CIVIL_YMD := CONCAT(A := s, B := PAD_MIN(n := dd, width := UDINT#2));
	END_FUNCTION

	/* 'HH:MM:SS' from seconds-of-day */
	FUNCTION PRIVATE HMS: STRING
		VAR_INPUT sod: LINT; END_VAR
		VAR s: STRING; END_VAR
		s := PAD_MIN(n := sod / LINT#3600, width := UDINT#2);
		s := CONCAT(A := s, B := ':');
		s := CONCAT(A := s, B := PAD_MIN(n := (sod / LINT#60) MOD LINT#60, width := UDINT#2));
		s := CONCAT(A := s, B := ':');
		HMS := CONCAT(A := s, B := PAD_MIN(n := sod MOD LINT#60, width := UDINT#2));
	END_FUNCTION

	FUNCTION TIME_TO_STRING: STRING
		VAR_INPUT IN: TIME; END_VAR
		VAR t, d, h, m, sec, ms: LINT; s: STRING; any: BOOL; END_VAR
		s := 'T#';
		t := TIME_TO_DINT(IN := IN);
		IF t < LINT#0 THEN
			s := CONCAT(A := s, B := '-');
			t := -t;
		END_IF
		d := t / LINT#86400000;
		t := t MOD LINT#86400000;
		h := t / LINT#3600000;
		t := t MOD LINT#3600000;
		m := t / LINT#60000;
		t := t MOD LINT#60000;
		sec := t / LINT#1000;
		ms := t MOD LINT#1000;
		IF d > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := LINT_TO_STRING(IN := d), B := 'd'));
			any := TRUE;
		END_IF
		IF h > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := LINT_TO_STRING(IN := h), B := 'h'));
			any := TRUE;
		END_IF
		IF m > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := LINT_TO_STRING(IN := m), B := 'm'));
			any := TRUE;
		END_IF
		IF sec > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := LINT_TO_STRING(IN := sec), B := 's'));
			any := TRUE;
		END_IF
		IF ms > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := LINT_TO_STRING(IN := ms), B := 'ms'));
			any := TRUE;
		END_IF
		IF NOT any THEN
			s := CONCAT(A := s, B := '0s');
		END_IF
		TIME_TO_STRING := s;
	END_FUNCTION

	FUNCTION LTIME_TO_STRING: STRING
		VAR_INPUT IN: LTIME; END_VAR
		VAR t, d, h, m, sec, ms, us, ns: LINT; s: STRING; any: BOOL; END_VAR
		t := LTIME_TO_LINT(IN := IN);
		IF (t < LINT#0) AND (-t < LINT#0) THEN
			(* i64 minimum: negation wraps, spell it directly *)
			LTIME_TO_STRING := 'LT#-106751d23h47m16s854ms775us808ns';
			RETURN;
		END_IF
		s := 'LT#';
		IF t < LINT#0 THEN
			s := CONCAT(A := s, B := '-');
			t := -t;
		END_IF
		d := t / LINT#86400000000000;
		t := t MOD LINT#86400000000000;
		h := t / LINT#3600000000000;
		t := t MOD LINT#3600000000000;
		m := t / LINT#60000000000;
		t := t MOD LINT#60000000000;
		sec := t / LINT#1000000000;
		t := t MOD LINT#1000000000;
		ms := t / LINT#1000000;
		t := t MOD LINT#1000000;
		us := t / LINT#1000;
		ns := t MOD LINT#1000;
		IF d > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := LINT_TO_STRING(IN := d), B := 'd'));
			any := TRUE;
		END_IF
		IF h > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := LINT_TO_STRING(IN := h), B := 'h'));
			any := TRUE;
		END_IF
		IF m > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := LINT_TO_STRING(IN := m), B := 'm'));
			any := TRUE;
		END_IF
		IF sec > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := LINT_TO_STRING(IN := sec), B := 's'));
			any := TRUE;
		END_IF
		IF ms > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := LINT_TO_STRING(IN := ms), B := 'ms'));
			any := TRUE;
		END_IF
		IF us > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := LINT_TO_STRING(IN := us), B := 'us'));
			any := TRUE;
		END_IF
		IF ns > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := LINT_TO_STRING(IN := ns), B := 'ns'));
			any := TRUE;
		END_IF
		IF NOT any THEN
			s := CONCAT(A := s, B := '0s');
		END_IF
		LTIME_TO_STRING := s;
	END_FUNCTION

	FUNCTION DATE_TO_STRING: STRING
		VAR_INPUT IN: DATE; END_VAR
		DATE_TO_STRING := CONCAT(A := 'D#', B := CIVIL_YMD(days := DATE_TO_DINT(IN := IN)));
	END_FUNCTION

	FUNCTION LDATE_TO_STRING: STRING
		VAR_INPUT IN: LDATE; END_VAR
		LDATE_TO_STRING := CONCAT(A := 'LD#', B := CIVIL_YMD(days := LDATE_TO_LINT(IN := IN)));
	END_FUNCTION

	FUNCTION TOD_TO_STRING: STRING
		VAR_INPUT IN: TIME_OF_DAY; END_VAR
		VAR t, ms: LINT; s: STRING; END_VAR
		s := 'TOD#';
		t := TOD_TO_DINT(IN := IN);
		IF t < LINT#0 THEN
			s := CONCAT(A := s, B := '-');
			t := -t;
		END_IF
		ms := t MOD LINT#1000;
		s := CONCAT(A := s, B := HMS(sod := t / LINT#1000));
		IF ms > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := '.', B := PAD_MIN(n := ms, width := UDINT#3)));
		END_IF
		TOD_TO_STRING := s;
	END_FUNCTION

	FUNCTION LTOD_TO_STRING: STRING
		VAR_INPUT IN: LTIME_OF_DAY; END_VAR
		VAR t, ns: LINT; s: STRING; END_VAR
		s := 'LTOD#';
		t := LTOD_TO_LINT(IN := IN);
		IF t < LINT#0 THEN
			s := CONCAT(A := s, B := '-');
			t := -t;
		END_IF
		ns := t MOD LINT#1000000000;
		s := CONCAT(A := s, B := HMS(sod := t / LINT#1000000000));
		IF ns > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := '.', B := PAD_MIN(n := ns, width := UDINT#9)));
		END_IF
		LTOD_TO_STRING := s;
	END_FUNCTION

	FUNCTION DT_TO_STRING: STRING
		VAR_INPUT IN: DATE_AND_TIME; END_VAR
		VAR t, days, sod: LINT; s: STRING; END_VAR
		t := DT_TO_LINT(IN := IN);
		(* floor split, exact pre-epoch, matching the cast intrinsics *)
		days := t / LINT#86400;
		sod := t - days * LINT#86400;
		IF sod < LINT#0 THEN
			days := days - LINT#1;
			sod := sod + LINT#86400;
		END_IF
		s := CONCAT(A := 'DT#', B := CIVIL_YMD(days := days));
		s := CONCAT(A := s, B := '-');
		DT_TO_STRING := CONCAT(A := s, B := HMS(sod := sod));
	END_FUNCTION

	FUNCTION LDT_TO_STRING: STRING
		VAR_INPUT IN: LDATE_AND_TIME; END_VAR
		VAR t, days, nsod, ns: LINT; s: STRING; END_VAR
		t := LDT_TO_LINT(IN := IN);
		days := t / LINT#86400000000000;
		nsod := t - days * LINT#86400000000000;
		IF nsod < LINT#0 THEN
			days := days - LINT#1;
			nsod := nsod + LINT#86400000000000;
		END_IF
		ns := nsod MOD LINT#1000000000;
		s := CONCAT(A := 'LDT#', B := CIVIL_YMD(days := days));
		s := CONCAT(A := s, B := '-');
		s := CONCAT(A := s, B := HMS(sod := nsod / LINT#1000000000));
		IF ns > LINT#0 THEN
			s := CONCAT(A := s, B := CONCAT(A := '.', B := PAD_MIN(n := ns, width := UDINT#9)));
		END_IF
		LDT_TO_STRING := s;
	END_FUNCTION


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
			ASSERT(r = 4);
		END_FUNCTION

		{test}
		FUNCTION test_real_to_int_halfway_to_even
			ASSERT(REAL_TO_INT(2.5) = 2 AND REAL_TO_INT(3.5) = 4 AND REAL_TO_INT(-2.5) = -2 AND REAL_TO_INT(0.5) = 0);
		END_FUNCTION

		{test}
		FUNCTION test_real_to_int_saturates
			ASSERT(REAL_TO_INT(40000.0) = 32767 AND REAL_TO_INT(-40000.0) = INT#-32768);
			ASSERT(REAL_TO_SINT(300.0) = 127 AND REAL_TO_SINT(-300.0) = SINT#-128);
		END_FUNCTION

		{test}
		FUNCTION test_real_to_dint_saturates
			ASSERT(REAL_TO_DINT(1.0E10) = 2147483647 AND REAL_TO_DINT(-1.0E10) = DINT#-2147483647 - DINT#1);
		END_FUNCTION

		{test}
		FUNCTION test_real_to_unsigned_clamps_at_zero_and_at_the_top
			ASSERT(REAL_TO_UINT(-5.0) = 0 AND REAL_TO_UINT(70000.0) = 65535 AND REAL_TO_USINT(300.0) = 255);
			ASSERT(REAL_TO_UDINT(-1.0) = 0 AND REAL_TO_UDINT(1.0E10) = 4294967295);
		END_FUNCTION

		{test}
		FUNCTION test_real_nan_converts_to_zero
			VAR
				z: REAL := 0.0;
				nan: REAL;
			END_VAR
			nan := z / z;
			ASSERT(REAL_TO_INT(nan) = 0 AND REAL_TO_DINT(nan) = 0 AND REAL_TO_LINT(nan) = 0 AND REAL_TO_UDINT(nan) = 0);
		END_FUNCTION

		{test}
		FUNCTION test_trunc_is_toward_zero
			ASSERT(TRUNC(3.7) = 3 AND TRUNC(-3.7) = -3 AND TRUNC(2.5) = 2);
			ASSERT(TRUNC(LREAL#3.7) = 3 AND TRUNC(LREAL#-2.5) = -2);
		END_FUNCTION

		{test}
		FUNCTION test_real_to_int_negative
			VAR
				r: INT;
			END_VAR
			r := REAL_TO_INT(-3.7);
			ASSERT(r = -4);
		END_FUNCTION

		{test}
		FUNCTION test_lreal_to_dint
			VAR
				r: DINT;
			END_VAR
			r := LREAL_TO_DINT(42.9);
			ASSERT(r = 43);
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
		FUNCTION test_lreal_to_int_rounds_and_saturates
			ASSERT(LREAL_TO_INT(3.5) = 4 AND LREAL_TO_INT(2.5) = 2 AND LREAL_TO_INT(-2.5) = -2);
			ASSERT(LREAL_TO_INT(40000.0) = 32767 AND LREAL_TO_INT(-40000.0) = INT#-32768);
			ASSERT(LREAL_TO_SINT(127.5) = 127 AND LREAL_TO_SINT(-128.5) = SINT#-128 AND LREAL_TO_SINT(300.0) = 127);
		END_FUNCTION

		{test}
		FUNCTION test_lreal_to_unsigned_clamps_at_zero_and_at_the_top
			ASSERT(LREAL_TO_UINT(2.5) = 2 AND LREAL_TO_UINT(65535.0) = 65535 AND LREAL_TO_UINT(70000.0) = 65535 AND LREAL_TO_UINT(-5.0) = 0);
			ASSERT(LREAL_TO_USINT(3.5) = 4 AND LREAL_TO_USINT(300.0) = 255 AND LREAL_TO_USINT(-1.0) = 0);
			ASSERT(LREAL_TO_UDINT(1.0E10) = 4294967295 AND LREAL_TO_UDINT(-1.0) = 0);
			ASSERT(LREAL_TO_ULINT(1.0E20) = ULINT#18446744073709551615 AND LREAL_TO_ULINT(-1.0) = 0);
			ASSERT(REAL_TO_ULINT(1.0E20) = ULINT#18446744073709551615 AND REAL_TO_ULINT(-1.0) = 0 AND REAL_TO_ULINT(2.5) = 2 AND REAL_TO_ULINT(3.5) = 4);
		END_FUNCTION

		{test}
		FUNCTION test_lreal_nan_converts_to_zero
			VAR
				z: LREAL := 0.0;
				nan: LREAL;
			END_VAR
			nan := z / z;
			ASSERT(LREAL_TO_INT(nan) = 0 AND LREAL_TO_SINT(nan) = 0 AND LREAL_TO_UINT(nan) = 0 AND LREAL_TO_USINT(nan) = 0);
			ASSERT(LREAL_TO_UDINT(nan) = 0 AND LREAL_TO_ULINT(nan) = 0 AND REAL_TO_ULINT(LREAL_TO_REAL(nan)) = 0);
		END_FUNCTION

		{test}
		FUNCTION test_char_literal_is_a_code_point
			ASSERT(CHAR_TO_STRING(CHAR#'A') = 'A' AND CHAR_TO_STRING(CHAR#'é') = 'é' AND CHAR_TO_STRING(CHAR#'中') = '中');
			ASSERT(CHAR_TO_STRING(CHAR#'$E9') = 'é' AND CHAR#'$E9' = CHAR#'é');
		END_FUNCTION

		{test}
		FUNCTION test_char_and_byte_meet_on_the_latin1_range
			ASSERT(BYTE_TO_CHAR(BYTE#16#E9) = CHAR#'é' AND CHAR_TO_BYTE(CHAR#'é') = BYTE#16#E9);
			ASSERT(CHAR_TO_BYTE(CHAR#'中') = BYTE#16#2D);
		END_FUNCTION

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
			a := 42.7; // rounds to 43, then back to 43.0
			b := REAL_TO_INT(a);
			c := DINT_TO_REAL(b);
			ASSERT(c = 43.0);
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
			ASSERT(s = '1.5');
		END_FUNCTION

		{test}
		FUNCTION test_real_to_string_negative
			VAR s: STRING; END_VAR
			s := REAL_TO_STRING(-3.25);
			ASSERT(s = '-3.25');
		END_FUNCTION

		{test}
		FUNCTION test_real_to_string_zero
			VAR s: STRING; END_VAR
			s := REAL_TO_STRING(0.0);
			ASSERT(s = '0.0');
		END_FUNCTION

		{test}
		FUNCTION test_lreal_to_string_basic
			VAR s: STRING; END_VAR
			s := LREAL_TO_STRING(2.5);
			ASSERT(s = '2.5');
		END_FUNCTION

		{test}
		FUNCTION test_real_to_string_is_the_shortest_round_trip
			ASSERT(REAL_TO_STRING(50.1) = '50.1', 'a REAL is formatted as a REAL, not as its f64 widening');
			ASSERT(REAL_TO_STRING(100.0) = '100.0' AND REAL_TO_STRING(3.14159) = '3.14159');
			ASSERT(LREAL_TO_STRING(123456789012.345) = '123456789012.345');
			ASSERT(LREAL_TO_STRING(1.0E15) = '1000000000000000.0');
		END_FUNCTION

		{test}
		FUNCTION test_real_to_string_exponent_form_is_a_literal
			ASSERT(REAL_TO_STRING(1.0E20) = '1.0E20' AND LREAL_TO_STRING(1.0E16) = '1.0E16');
			ASSERT(REAL_TO_STRING(1.0E-7) = '1.0E-7' AND LREAL_TO_STRING(-2.5E-9) = '-2.5E-9');
			ASSERT(LREAL_TO_STRING(1.0E-5) = '0.00001');
		END_FUNCTION

		{test}
		FUNCTION test_lreal_to_string_shows_the_value_it_has
			VAR
				a: LREAL := 0.1;
				b: LREAL := 0.2;
			END_VAR
			ASSERT(LREAL_TO_STRING(a + b) = '0.30000000000000004');
		END_FUNCTION

		{test}
		FUNCTION test_real_to_string_non_finite
			VAR
				z: REAL := 0.0;
			END_VAR
			ASSERT(REAL_TO_STRING(z / z) = 'NaN');
			ASSERT(REAL_TO_STRING(1.0 / z) = 'Inf' AND REAL_TO_STRING(-1.0 / z) = '-Inf');
		END_FUNCTION

		{test}
		FUNCTION test_lreal_to_string_non_finite
			VAR
				z: LREAL := 0.0;
			END_VAR
			ASSERT(LREAL_TO_STRING(z / z) = 'NaN');
			ASSERT(LREAL_TO_STRING(1.0 / z) = 'Inf' AND LREAL_TO_STRING(-1.0 / z) = '-Inf');
		END_FUNCTION

		// Composition with string ops — convert and concatenate.
		{test}
		FUNCTION test_int_to_string_in_concat
			VAR s: STRING; END_VAR
			s := CONCAT('Temperature: ', INT_TO_STRING(42));
			ASSERT(FIND(s, 'Temperature: 42') = 1);
		END_FUNCTION

		// --- narrowing truncates to the target width (two's complement),
		// it must never let a value escape the target's domain ---

		{test}
		FUNCTION test_int_to_sint_truncates
			ASSERT_EQ(INT_TO_SINT(INT#200), SINT#-56, '200 = 16#C8, sign bit set as SINT');
			ASSERT_EQ(INT_TO_SINT(INT#127), SINT#127, 'in-range value unchanged');
		END_FUNCTION

		{test}
		FUNCTION test_int_to_uint_masks
			ASSERT_EQ(INT_TO_UINT(INT#-1), UINT#65535, '-1 reinterprets as all-ones 16-bit');
		END_FUNCTION

		{test}
		FUNCTION test_lint_to_int_truncates
			ASSERT_EQ(LINT_TO_INT(LINT#70000), INT#4464, '70000 = 16#11170, low 16 bits = 16#1170');
			ASSERT_EQ(LINT_TO_INT(LINT#-70000), INT#-4464, 'negative truncation is twos-complement');
		END_FUNCTION

		{test}
		FUNCTION test_dint_to_int_truncates
			ASSERT_EQ(DINT_TO_INT(DINT#70000), INT#4464, 'same truncation on the i32 lane');
		END_FUNCTION

		{test}
		FUNCTION test_ulint_to_usint_truncates
			ASSERT_EQ(ULINT_TO_USINT(ULINT#300), USINT#44, '300 = 16#12C, low byte = 16#2C');
		END_FUNCTION

		// --- signed <-> unsigned at the same width is a bit-pattern
		// reinterpretation of the low bits ---

		{test}
		FUNCTION test_byte_sint_reinterpret
			ASSERT_EQ(BYTE_TO_SINT(BYTE#16#C8), SINT#-56, 'byte 16#C8 reads as -56 signed');
			ASSERT_EQ(SINT_TO_BYTE(SINT#-56), BYTE#16#C8, 'and back');
		END_FUNCTION

		{test}
		FUNCTION test_usint_to_sint_reinterpret
			ASSERT_EQ(USINT_TO_SINT(USINT#200), SINT#-56, 'same-width unsigned to signed');
		END_FUNCTION

		{test}
		FUNCTION test_dint_to_word_truncates
			ASSERT_EQ(DINT_TO_WORD(DINT#-1), WORD#16#FFFF, 'low 16 bits of all-ones');
		END_FUNCTION

		// --- float <-> bits reinterprets are BIT PATTERNS, not numeric casts ---

		{test}
		FUNCTION test_real_dword_reinterpret
			ASSERT_EQ(REAL_TO_DWORD(REAL#1.5), DWORD#16#3FC00000, 'IEEE 754 bits of 1.5f');
			ASSERT_EQ(DWORD_TO_REAL(DWORD#16#3FC00000), REAL#1.5, 'and back');
		END_FUNCTION

		{test}
		FUNCTION test_lreal_lword_reinterpret
			ASSERT_EQ(LREAL_TO_LWORD(LREAL#1.5), LWORD#16#3FF8_0000_0000_0000, 'IEEE 754 bits of 1.5');
			ASSERT_EQ(LWORD_TO_LREAL(LWORD#16#3FF8_0000_0000_0000), LREAL#1.5, 'and back');
		END_FUNCTION

		// --- float -> int at the edges: saturation, never a wrap ---

		{test}
		FUNCTION test_real_to_int_overflow_saturates
			(* 40000.0 used to truncate to i32 and then wrap to -25536 in the
			   16-bit narrowing; a magnitude past the target saturates now *)
			ASSERT_EQ(REAL_TO_INT(REAL#40000.0), INT#32767, 'out-of-range REAL saturates to the bound');
		END_FUNCTION

		{test}
		FUNCTION test_real_to_udint_above_signed_max
			ASSERT_EQ(REAL_TO_UDINT(REAL#3000000000.0), UDINT#3000000000, 'unsigned trunc above 2^31');
		END_FUNCTION

		// --- int -> float precision: f32 has a 24-bit mantissa ---

		{test}
		FUNCTION test_dint_to_real_precision_loss
			ASSERT(DINT_TO_REAL(DINT#16777217) = 16777216.0, '2^24+1 is not representable in REAL');
			ASSERT(DINT_TO_REAL(DINT#16777216) = 16777216.0, '2^24 is exact');
		END_FUNCTION

		// --- duration narrowing scales units (ns -> ms), it is not a raw wrap ---

		{test}
		FUNCTION test_ltime_to_time_scales
			ASSERT(LTIME_TO_TIME(LTIME#1s) = T#1s, 'LTIME ns scale to TIME ms');
			ASSERT(LTIME_TO_TIME(LTIME#1500ms) = T#1500ms, 'sub-second precision survives');
		END_FUNCTION

		// -- TRUNC ----------------------------------------------------

		{test}
		FUNCTION test_trunc_real_toward_zero
			ASSERT_EQ(TRUNC(3.7), DINT#3);
			ASSERT_EQ(TRUNC(-3.7), DINT#-3);
			ASSERT_EQ(TRUNC(0.9), DINT#0);
		END_FUNCTION

		{test}
		FUNCTION test_trunc_lreal_toward_zero
			ASSERT_EQ(TRUNC(LREAL#3.7), LINT#3);
			ASSERT_EQ(TRUNC(LREAL#-3.7), LINT#-3);
		END_FUNCTION

		{test}
		FUNCTION test_time_numeric_round_trip
			ASSERT_EQ(value := TIME_TO_DINT(IN := T#1s500ms), target := DINT#1500);
			ASSERT_EQ(value := DINT_TO_TIME(IN := DINT#1500), target := T#1s500ms);
			ASSERT_EQ(value := LTIME_TO_LINT(IN := LT#1s), target := LINT#1000000000);
			ASSERT_EQ(value := TOD_TO_DINT(IN := TOD#00:00:01.5), target := DINT#1500);
			ASSERT_EQ(value := DT_TO_LINT(IN := DT#1970-01-01-00:01:00), target := LINT#60);
			ASSERT_EQ(value := LDT_TO_LINT(IN := LDT#1970-01-01-00:00:01), target := LINT#1000000000);
			ASSERT(value := DINT_TO_DATE(IN := DINT#1) = D#1970-01-02);
			ASSERT(value := LINT_TO_DT(IN := LINT#60) = DT#1970-01-01-00:01:00);
		END_FUNCTION

		{test}
		FUNCTION test_datetime_widening_functions
			ASSERT_EQ(value := TIME_TO_LTIME(IN := T#5ms), target := LT#5ms);
			ASSERT(value := TOD_TO_LTOD(IN := TOD#00:00:01.5) = LTOD#00:00:01.5);
			ASSERT(value := DATE_TO_LDATE(IN := D#1969-12-31) = LD#1969-12-31);
			ASSERT(value := DT_TO_LDT(IN := DT#2038-01-19-03:14:07) = LDT#2038-01-19-03:14:07);
		END_FUNCTION

		{test}
		FUNCTION test_time_to_string
			ASSERT_EQ(value := TIME_TO_STRING(IN := T#0s), target := 'T#0s');
			ASSERT_EQ(value := TIME_TO_STRING(IN := T#1s500ms), target := 'T#1s500ms');
			ASSERT_EQ(value := TIME_TO_STRING(IN := T#1d2h3m4s5ms), target := 'T#1d2h3m4s5ms');
			ASSERT_EQ(value := TIME_TO_STRING(IN := T#-5s), target := 'T#-5s');
			ASSERT_EQ(value := TIME_TO_STRING(IN := T#90s), target := 'T#1m30s');
		END_FUNCTION

		{test}
		FUNCTION test_ltime_to_string
			ASSERT_EQ(value := LTIME_TO_STRING(IN := LT#0s), target := 'LT#0s');
			ASSERT_EQ(value := LTIME_TO_STRING(IN := LT#500us), target := 'LT#500us');
			ASSERT_EQ(value := LTIME_TO_STRING(IN := LT#5d14h12m18s3ms500us), target := 'LT#5d14h12m18s3ms500us');
			ASSERT_EQ(value := LTIME_TO_STRING(IN := LT#-42ns), target := 'LT#-42ns');
		END_FUNCTION

		{test}
		FUNCTION test_date_to_string
			ASSERT_EQ(value := DATE_TO_STRING(IN := D#1970-01-01), target := 'D#1970-01-01');
			ASSERT_EQ(value := DATE_TO_STRING(IN := D#2026-08-28), target := 'D#2026-08-28');
			ASSERT_EQ(value := DATE_TO_STRING(IN := D#2024-02-29), target := 'D#2024-02-29');
			ASSERT_EQ(value := DATE_TO_STRING(IN := D#1969-12-31), target := 'D#1969-12-31');
			ASSERT_EQ(value := LDATE_TO_STRING(IN := LD#2024-02-29), target := 'LD#2024-02-29');
		END_FUNCTION

		{test}
		FUNCTION test_tod_to_string
			ASSERT_EQ(value := TOD_TO_STRING(IN := TOD#00:00:00), target := 'TOD#00:00:00');
			ASSERT_EQ(value := TOD_TO_STRING(IN := TOD#12:30:45.123), target := 'TOD#12:30:45.123');
			ASSERT_EQ(value := TOD_TO_STRING(IN := TOD#23:59:59), target := 'TOD#23:59:59');
			ASSERT_EQ(value := LTOD_TO_STRING(IN := LTOD#12:30:45.123456789), target := 'LTOD#12:30:45.123456789');
		END_FUNCTION

		{test}
		FUNCTION test_dt_to_string
			ASSERT_EQ(value := DT_TO_STRING(IN := DT#1970-01-01-00:00:00), target := 'DT#1970-01-01-00:00:00');
			ASSERT_EQ(value := DT_TO_STRING(IN := DT#2026-08-28-12:30:45), target := 'DT#2026-08-28-12:30:45');
			(* floor split: exact pre-epoch, where the cast intrinsic is off by one *)
			ASSERT_EQ(value := DT_TO_STRING(IN := DT#1969-12-31-23:59:59), target := 'DT#1969-12-31-23:59:59');
			ASSERT_EQ(value := LDT_TO_STRING(IN := LDT#2026-08-28-12:30:45.5), target := 'LDT#2026-08-28-12:30:45.500000000');
		END_FUNCTION

	END_NAMESPACE
END_NAMESPACE
