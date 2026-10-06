/*
Array functions.

`LOWER_BOUND` and `UPPER_BOUND` read the bounds of an array. They take an
array of any type and any number of dimensions, so a FUNCTION with an
`ARRAY[*]` parameter can walk whatever array its caller passed.
*/
NAMESPACE Std.Arrays

	/* Returns the lower bound of dimension `DIM` of `ARR`.

	   Dimensions are numbered from 1. A `DIM` outside them raises.

	   # Examples

	   ```iecst
	   FOR i := LOWER_BOUND(values, 1) TO UPPER_BOUND(values, 1) DO
	       total := total + values[i];
	   END_FOR;
	   ```
	*/
	FUNCTION LOWER_BOUND: DINT
		VAR_INPUT
			ARR: ARRAY[*];
			DIM: DINT;
		END_VAR
		VAR
			dimensions: DINT;
		END_VAR

		{wasm 'array.dimensions' (params ARR) (result dimensions)}
		IF DIM < 1 OR DIM > dimensions THEN
			__RAISE('array dimension out of range');
		END_IF
		{wasm 'array.lower_bound' (params ARR DIM) (result LOWER_BOUND)}
	END_FUNCTION

	/* Returns the upper bound of dimension `DIM` of `ARR`.

	   Dimensions are numbered from 1. A `DIM` outside them raises.

	   # Examples

	   ```iecst
	   // m : ARRAY[1..3, 0..9] OF REAL
	   rows := UPPER_BOUND(m, 1) - LOWER_BOUND(m, 1) + 1;     // 3
	   columns := UPPER_BOUND(m, 2) - LOWER_BOUND(m, 2) + 1;  // 10
	   ```
	*/
	FUNCTION UPPER_BOUND: DINT
		VAR_INPUT
			ARR: ARRAY[*];
			DIM: DINT;
		END_VAR
		VAR
			dimensions: DINT;
		END_VAR

		{wasm 'array.dimensions' (params ARR) (result dimensions)}
		IF DIM < 1 OR DIM > dimensions THEN
			__RAISE('array dimension out of range');
		END_IF
		{wasm 'array.upper_bound' (params ARR DIM) (result UPPER_BOUND)}
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit

		{test}
		FUNCTION test_bounds_of_one_dimension
			VAR
				a: ARRAY[1..10] OF INT;
			END_VAR
			ASSERT(LOWER_BOUND(a, 1) = 1 AND UPPER_BOUND(a, 1) = 10);
		END_FUNCTION

		{test}
		FUNCTION test_bounds_of_each_dimension
			VAR
				a: ARRAY[1..20, -2..2] OF INT;
			END_VAR
			ASSERT(LOWER_BOUND(a, 1) = 1 AND UPPER_BOUND(a, 1) = 20);
			ASSERT(LOWER_BOUND(a, 2) = -2 AND UPPER_BOUND(a, 2) = 2);
		END_FUNCTION

		{test}
		FUNCTION test_bounds_by_name
			VAR
				a: ARRAY[5..7] OF STRING[4];
			END_VAR
			ASSERT(LOWER_BOUND(ARR := a, DIM := 1) = 5);
			ASSERT(UPPER_BOUND(DIM := 1, ARR := a) = 7);
		END_FUNCTION

		FUNCTION Sum: DINT
			VAR_IN_OUT
				values: ARRAY[*] OF INT;
			END_VAR
			VAR
				i: DINT;
			END_VAR
			FOR i := LOWER_BOUND(values, 1) TO UPPER_BOUND(values, 1) DO
				Sum := Sum + values[i];
			END_FOR
		END_FUNCTION

		{test}
		FUNCTION test_bounds_of_an_array_any_call_binds
			VAR
				a: ARRAY[1..20, -2..2] OF INT := [20(5(1))];
				b: ARRAY[0..2] OF INT := [1, 2, 3];
			END_VAR
			ASSERT(Sum(a[2]) = 5, 'a row of a');
			ASSERT(Sum(b) = 6, 'b');
		END_FUNCTION
	END_NAMESPACE

END_NAMESPACE
