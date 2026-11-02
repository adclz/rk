/*
Comparison functions: the comparison operators under a name.

`GT`, `GE`, `EQ`, `LE` and `LT` compare each input with the next, however many
there are. `NE` compares two.

Each has one overload per type it applies to.
*/
NAMESPACE Std.Comparison

	/* Returns `TRUE` when each input is greater than the next.

	   # Examples

	   ```iecst
	   b := GT(3, 2, 1);  // TRUE
	   b := GT(3, 3, 1);  // FALSE
	   ```
	*/
	FUNCTION GT: BOOL
		VAR_INPUT
			IN: SINT...;
		END_VAR

		GT := ...IN>;
	END_FUNCTION

	FUNCTION GT: BOOL
		VAR_INPUT
			IN: INT...;
		END_VAR

		GT := ...IN>;
	END_FUNCTION

	FUNCTION GT: BOOL
		VAR_INPUT
			IN: DINT...;
		END_VAR

		GT := ...IN>;
	END_FUNCTION

	FUNCTION GT: BOOL
		VAR_INPUT
			IN: LINT...;
		END_VAR

		GT := ...IN>;
	END_FUNCTION

	FUNCTION GT: BOOL
		VAR_INPUT
			IN: USINT...;
		END_VAR

		GT := ...IN>;
	END_FUNCTION

	FUNCTION GT: BOOL
		VAR_INPUT
			IN: UINT...;
		END_VAR

		GT := ...IN>;
	END_FUNCTION

	FUNCTION GT: BOOL
		VAR_INPUT
			IN: UDINT...;
		END_VAR

		GT := ...IN>;
	END_FUNCTION

	FUNCTION GT: BOOL
		VAR_INPUT
			IN: ULINT...;
		END_VAR

		GT := ...IN>;
	END_FUNCTION

	FUNCTION GT: BOOL
		VAR_INPUT
			IN: REAL...;
		END_VAR

		GT := ...IN>;
	END_FUNCTION

	FUNCTION GT: BOOL
		VAR_INPUT
			IN: LREAL...;
		END_VAR

		GT := ...IN>;
	END_FUNCTION

	FUNCTION GT: BOOL
		VAR_INPUT
			IN: TIME...;
		END_VAR

		GT := ...IN>;
	END_FUNCTION

	FUNCTION GT: BOOL
		VAR_INPUT
			IN: LTIME...;
		END_VAR

		GT := ...IN>;
	END_FUNCTION

	/* Returns `TRUE` when each input is greater than or equal to the next.

	   # Examples

	   ```iecst
	   b := GE(3, 3, 1);  // TRUE
	   b := GE(1, 3, 3);  // FALSE
	   ```
	*/
	FUNCTION GE: BOOL
		VAR_INPUT
			IN: SINT...;
		END_VAR

		GE := ...IN>=;
	END_FUNCTION

	FUNCTION GE: BOOL
		VAR_INPUT
			IN: INT...;
		END_VAR

		GE := ...IN>=;
	END_FUNCTION

	FUNCTION GE: BOOL
		VAR_INPUT
			IN: DINT...;
		END_VAR

		GE := ...IN>=;
	END_FUNCTION

	FUNCTION GE: BOOL
		VAR_INPUT
			IN: LINT...;
		END_VAR

		GE := ...IN>=;
	END_FUNCTION

	FUNCTION GE: BOOL
		VAR_INPUT
			IN: USINT...;
		END_VAR

		GE := ...IN>=;
	END_FUNCTION

	FUNCTION GE: BOOL
		VAR_INPUT
			IN: UINT...;
		END_VAR

		GE := ...IN>=;
	END_FUNCTION

	FUNCTION GE: BOOL
		VAR_INPUT
			IN: UDINT...;
		END_VAR

		GE := ...IN>=;
	END_FUNCTION

	FUNCTION GE: BOOL
		VAR_INPUT
			IN: ULINT...;
		END_VAR

		GE := ...IN>=;
	END_FUNCTION

	FUNCTION GE: BOOL
		VAR_INPUT
			IN: REAL...;
		END_VAR

		GE := ...IN>=;
	END_FUNCTION

	FUNCTION GE: BOOL
		VAR_INPUT
			IN: LREAL...;
		END_VAR

		GE := ...IN>=;
	END_FUNCTION

	FUNCTION GE: BOOL
		VAR_INPUT
			IN: TIME...;
		END_VAR

		GE := ...IN>=;
	END_FUNCTION

	FUNCTION GE: BOOL
		VAR_INPUT
			IN: LTIME...;
		END_VAR

		GE := ...IN>=;
	END_FUNCTION

	/* Returns `TRUE` when its inputs are all equal.

	   # Examples

	   ```iecst
	   b := EQ(2, 2, 2);  // TRUE
	   b := EQ(2, 2, 3);  // FALSE
	   ```
	*/
	FUNCTION EQ: BOOL
		VAR_INPUT
			IN: SINT...;
		END_VAR

		EQ := ...IN=;
	END_FUNCTION

	FUNCTION EQ: BOOL
		VAR_INPUT
			IN: INT...;
		END_VAR

		EQ := ...IN=;
	END_FUNCTION

	FUNCTION EQ: BOOL
		VAR_INPUT
			IN: DINT...;
		END_VAR

		EQ := ...IN=;
	END_FUNCTION

	FUNCTION EQ: BOOL
		VAR_INPUT
			IN: LINT...;
		END_VAR

		EQ := ...IN=;
	END_FUNCTION

	FUNCTION EQ: BOOL
		VAR_INPUT
			IN: USINT...;
		END_VAR

		EQ := ...IN=;
	END_FUNCTION

	FUNCTION EQ: BOOL
		VAR_INPUT
			IN: UINT...;
		END_VAR

		EQ := ...IN=;
	END_FUNCTION

	FUNCTION EQ: BOOL
		VAR_INPUT
			IN: UDINT...;
		END_VAR

		EQ := ...IN=;
	END_FUNCTION

	FUNCTION EQ: BOOL
		VAR_INPUT
			IN: ULINT...;
		END_VAR

		EQ := ...IN=;
	END_FUNCTION

	FUNCTION EQ: BOOL
		VAR_INPUT
			IN: REAL...;
		END_VAR

		EQ := ...IN=;
	END_FUNCTION

	FUNCTION EQ: BOOL
		VAR_INPUT
			IN: LREAL...;
		END_VAR

		EQ := ...IN=;
	END_FUNCTION

	FUNCTION EQ: BOOL
		VAR_INPUT
			IN: TIME...;
		END_VAR

		EQ := ...IN=;
	END_FUNCTION

	FUNCTION EQ: BOOL
		VAR_INPUT
			IN: LTIME...;
		END_VAR

		EQ := ...IN=;
	END_FUNCTION

	/* Returns `TRUE` when each input is less than or equal to the next.

	   # Examples

	   ```iecst
	   b := LE(1, 3, 3);  // TRUE
	   b := LE(3, 3, 1);  // FALSE
	   ```
	*/
	FUNCTION LE: BOOL
		VAR_INPUT
			IN: SINT...;
		END_VAR

		LE := ...IN<=;
	END_FUNCTION

	FUNCTION LE: BOOL
		VAR_INPUT
			IN: INT...;
		END_VAR

		LE := ...IN<=;
	END_FUNCTION

	FUNCTION LE: BOOL
		VAR_INPUT
			IN: DINT...;
		END_VAR

		LE := ...IN<=;
	END_FUNCTION

	FUNCTION LE: BOOL
		VAR_INPUT
			IN: LINT...;
		END_VAR

		LE := ...IN<=;
	END_FUNCTION

	FUNCTION LE: BOOL
		VAR_INPUT
			IN: USINT...;
		END_VAR

		LE := ...IN<=;
	END_FUNCTION

	FUNCTION LE: BOOL
		VAR_INPUT
			IN: UINT...;
		END_VAR

		LE := ...IN<=;
	END_FUNCTION

	FUNCTION LE: BOOL
		VAR_INPUT
			IN: UDINT...;
		END_VAR

		LE := ...IN<=;
	END_FUNCTION

	FUNCTION LE: BOOL
		VAR_INPUT
			IN: ULINT...;
		END_VAR

		LE := ...IN<=;
	END_FUNCTION

	FUNCTION LE: BOOL
		VAR_INPUT
			IN: REAL...;
		END_VAR

		LE := ...IN<=;
	END_FUNCTION

	FUNCTION LE: BOOL
		VAR_INPUT
			IN: LREAL...;
		END_VAR

		LE := ...IN<=;
	END_FUNCTION

	FUNCTION LE: BOOL
		VAR_INPUT
			IN: TIME...;
		END_VAR

		LE := ...IN<=;
	END_FUNCTION

	FUNCTION LE: BOOL
		VAR_INPUT
			IN: LTIME...;
		END_VAR

		LE := ...IN<=;
	END_FUNCTION

	/* Returns `TRUE` when each input is less than the next.

	   # Examples

	   ```iecst
	   b := LT(1, 2, 3);  // TRUE
	   b := LT(1, 3, 3);  // FALSE
	   ```
	*/
	FUNCTION LT: BOOL
		VAR_INPUT
			IN: SINT...;
		END_VAR

		LT := ...IN<;
	END_FUNCTION

	FUNCTION LT: BOOL
		VAR_INPUT
			IN: INT...;
		END_VAR

		LT := ...IN<;
	END_FUNCTION

	FUNCTION LT: BOOL
		VAR_INPUT
			IN: DINT...;
		END_VAR

		LT := ...IN<;
	END_FUNCTION

	FUNCTION LT: BOOL
		VAR_INPUT
			IN: LINT...;
		END_VAR

		LT := ...IN<;
	END_FUNCTION

	FUNCTION LT: BOOL
		VAR_INPUT
			IN: USINT...;
		END_VAR

		LT := ...IN<;
	END_FUNCTION

	FUNCTION LT: BOOL
		VAR_INPUT
			IN: UINT...;
		END_VAR

		LT := ...IN<;
	END_FUNCTION

	FUNCTION LT: BOOL
		VAR_INPUT
			IN: UDINT...;
		END_VAR

		LT := ...IN<;
	END_FUNCTION

	FUNCTION LT: BOOL
		VAR_INPUT
			IN: ULINT...;
		END_VAR

		LT := ...IN<;
	END_FUNCTION

	FUNCTION LT: BOOL
		VAR_INPUT
			IN: REAL...;
		END_VAR

		LT := ...IN<;
	END_FUNCTION

	FUNCTION LT: BOOL
		VAR_INPUT
			IN: LREAL...;
		END_VAR

		LT := ...IN<;
	END_FUNCTION

	FUNCTION LT: BOOL
		VAR_INPUT
			IN: TIME...;
		END_VAR

		LT := ...IN<;
	END_FUNCTION

	FUNCTION LT: BOOL
		VAR_INPUT
			IN: LTIME...;
		END_VAR

		LT := ...IN<;
	END_FUNCTION

	/* Returns `TRUE` when `IN1` and `IN2` differ.

	   # Examples

	   ```iecst
	   b := NE(1, 2);  // TRUE
	   b := NE(2, 2);  // FALSE
	   ```
	*/
	FUNCTION NE: BOOL
		VAR_INPUT
			IN1: SINT;
			IN2: SINT;
		END_VAR

		NE := IN1 <> IN2;
	END_FUNCTION

	FUNCTION NE: BOOL
		VAR_INPUT
			IN1: INT;
			IN2: INT;
		END_VAR

		NE := IN1 <> IN2;
	END_FUNCTION

	FUNCTION NE: BOOL
		VAR_INPUT
			IN1: DINT;
			IN2: DINT;
		END_VAR

		NE := IN1 <> IN2;
	END_FUNCTION

	FUNCTION NE: BOOL
		VAR_INPUT
			IN1: LINT;
			IN2: LINT;
		END_VAR

		NE := IN1 <> IN2;
	END_FUNCTION

	FUNCTION NE: BOOL
		VAR_INPUT
			IN1: USINT;
			IN2: USINT;
		END_VAR

		NE := IN1 <> IN2;
	END_FUNCTION

	FUNCTION NE: BOOL
		VAR_INPUT
			IN1: UINT;
			IN2: UINT;
		END_VAR

		NE := IN1 <> IN2;
	END_FUNCTION

	FUNCTION NE: BOOL
		VAR_INPUT
			IN1: UDINT;
			IN2: UDINT;
		END_VAR

		NE := IN1 <> IN2;
	END_FUNCTION

	FUNCTION NE: BOOL
		VAR_INPUT
			IN1: ULINT;
			IN2: ULINT;
		END_VAR

		NE := IN1 <> IN2;
	END_FUNCTION

	{allow 'float-equality'}
	FUNCTION NE: BOOL
		VAR_INPUT
			IN1: REAL;
			IN2: REAL;
		END_VAR

		NE := IN1 <> IN2;
	END_FUNCTION

	{allow 'float-equality'}
	FUNCTION NE: BOOL
		VAR_INPUT
			IN1: LREAL;
			IN2: LREAL;
		END_VAR

		NE := IN1 <> IN2;
	END_FUNCTION

	FUNCTION NE: BOOL
		VAR_INPUT
			IN1: TIME;
			IN2: TIME;
		END_VAR

		NE := IN1 <> IN2;
	END_FUNCTION

	FUNCTION NE: BOOL
		VAR_INPUT
			IN1: LTIME;
			IN2: LTIME;
		END_VAR

		NE := IN1 <> IN2;
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit

		{test}
		FUNCTION test_gt
			ASSERT(GT(3, 2, 1));
			ASSERT(NOT GT(3, 3, 1));
		END_FUNCTION

		{test}
		FUNCTION test_ge
			ASSERT(GE(3, 3, 1));
			ASSERT(NOT GE(1, 3, 3));
		END_FUNCTION

		{test}
		FUNCTION test_eq
			ASSERT(EQ(2, 2, 2));
			ASSERT(NOT EQ(2, 2, 3));
		END_FUNCTION

		{test}
		FUNCTION test_le
			ASSERT(LE(1, 3, 3));
			ASSERT(NOT LE(3, 3, 1));
		END_FUNCTION

		{test}
		FUNCTION test_lt
			ASSERT(LT(1, 2, 3));
			ASSERT(NOT LT(1, 3, 3));
		END_FUNCTION

		{test}
		FUNCTION test_ne
			ASSERT(NE(1, 2));
			ASSERT(NOT NE(2, 2));
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
