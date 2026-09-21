/*
Selection functions.

Each has one overload per type it applies to, and returns the type it was
given.
*/
NAMESPACE Std.Selection

	/* Returns `IN0` when `G` is `FALSE`, and `IN1` when it is `TRUE`.

	   # Examples

	   ```iecst
	   n := SEL(TRUE, 10, 42);   // 42
	   n := SEL(FALSE, 10, 42);  // 10
	   ```
	*/
	FUNCTION SEL: BOOL
		VAR_INPUT
			G: BOOL;
			IN0: BOOL;
			IN1: BOOL;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: SINT
		VAR_INPUT
			G: BOOL;
			IN0: SINT;
			IN1: SINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: INT
		VAR_INPUT
			G: BOOL;
			IN0: INT;
			IN1: INT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: DINT
		VAR_INPUT
			G: BOOL;
			IN0: DINT;
			IN1: DINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: LINT
		VAR_INPUT
			G: BOOL;
			IN0: LINT;
			IN1: LINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: USINT
		VAR_INPUT
			G: BOOL;
			IN0: USINT;
			IN1: USINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: UINT
		VAR_INPUT
			G: BOOL;
			IN0: UINT;
			IN1: UINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: UDINT
		VAR_INPUT
			G: BOOL;
			IN0: UDINT;
			IN1: UDINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: ULINT
		VAR_INPUT
			G: BOOL;
			IN0: ULINT;
			IN1: ULINT;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: BYTE
		VAR_INPUT
			G: BOOL;
			IN0: BYTE;
			IN1: BYTE;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: WORD
		VAR_INPUT
			G: BOOL;
			IN0: WORD;
			IN1: WORD;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: DWORD
		VAR_INPUT
			G: BOOL;
			IN0: DWORD;
			IN1: DWORD;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: LWORD
		VAR_INPUT
			G: BOOL;
			IN0: LWORD;
			IN1: LWORD;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: REAL
		VAR_INPUT
			G: BOOL;
			IN0: REAL;
			IN1: REAL;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: LREAL
		VAR_INPUT
			G: BOOL;
			IN0: LREAL;
			IN1: LREAL;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: TIME
		VAR_INPUT
			G: BOOL;
			IN0: TIME;
			IN1: TIME;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	FUNCTION SEL: STRING
		VAR_INPUT
			G: BOOL;
			IN0: STRING;
			IN1: STRING;
		END_VAR

		IF G THEN
			SEL := IN1;
		ELSE
			SEL := IN0;
		END_IF;
	END_FUNCTION

	/* Returns `IN` kept within `MN` and `MX`.

	   `MX` wins when `MN` is greater than `MX`.

	   # Examples

	   ```iecst
	   n := LIMIT(0, 15, 10);  // 10
	   n := LIMIT(0, -5, 10);  // 0
	   n := LIMIT(0, 5, 10);   // 5
	   ```
	*/
	FUNCTION LIMIT: SINT
		VAR_INPUT
			MN: SINT;
			IN: SINT;
			MX: SINT;
		END_VAR

		(* IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX), so MX wins when MN > MX. *)
		LIMIT := IN;
		IF LIMIT < MN THEN
			LIMIT := MN;
		END_IF;
		IF LIMIT > MX THEN
			LIMIT := MX;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: INT
		VAR_INPUT
			MN: INT;
			IN: INT;
			MX: INT;
		END_VAR

		(* IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX), so MX wins when MN > MX. *)
		LIMIT := IN;
		IF LIMIT < MN THEN
			LIMIT := MN;
		END_IF;
		IF LIMIT > MX THEN
			LIMIT := MX;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: DINT
		VAR_INPUT
			MN: DINT;
			IN: DINT;
			MX: DINT;
		END_VAR

		(* IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX), so MX wins when MN > MX. *)
		LIMIT := IN;
		IF LIMIT < MN THEN
			LIMIT := MN;
		END_IF;
		IF LIMIT > MX THEN
			LIMIT := MX;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: LINT
		VAR_INPUT
			MN: LINT;
			IN: LINT;
			MX: LINT;
		END_VAR

		(* IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX), so MX wins when MN > MX. *)
		LIMIT := IN;
		IF LIMIT < MN THEN
			LIMIT := MN;
		END_IF;
		IF LIMIT > MX THEN
			LIMIT := MX;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: USINT
		VAR_INPUT
			MN: USINT;
			IN: USINT;
			MX: USINT;
		END_VAR

		(* IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX), so MX wins when MN > MX. *)
		LIMIT := IN;
		IF LIMIT < MN THEN
			LIMIT := MN;
		END_IF;
		IF LIMIT > MX THEN
			LIMIT := MX;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: UINT
		VAR_INPUT
			MN: UINT;
			IN: UINT;
			MX: UINT;
		END_VAR

		(* IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX), so MX wins when MN > MX. *)
		LIMIT := IN;
		IF LIMIT < MN THEN
			LIMIT := MN;
		END_IF;
		IF LIMIT > MX THEN
			LIMIT := MX;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: UDINT
		VAR_INPUT
			MN: UDINT;
			IN: UDINT;
			MX: UDINT;
		END_VAR

		(* IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX), so MX wins when MN > MX. *)
		LIMIT := IN;
		IF LIMIT < MN THEN
			LIMIT := MN;
		END_IF;
		IF LIMIT > MX THEN
			LIMIT := MX;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: ULINT
		VAR_INPUT
			MN: ULINT;
			IN: ULINT;
			MX: ULINT;
		END_VAR

		(* IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX), so MX wins when MN > MX. *)
		LIMIT := IN;
		IF LIMIT < MN THEN
			LIMIT := MN;
		END_IF;
		IF LIMIT > MX THEN
			LIMIT := MX;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: REAL
		VAR_INPUT
			MN: REAL;
			IN: REAL;
			MX: REAL;
		END_VAR

		(* IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX), so MX wins when MN > MX. *)
		LIMIT := IN;
		IF LIMIT < MN THEN
			LIMIT := MN;
		END_IF;
		IF LIMIT > MX THEN
			LIMIT := MX;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: LREAL
		VAR_INPUT
			MN: LREAL;
			IN: LREAL;
			MX: LREAL;
		END_VAR

		(* IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX), so MX wins when MN > MX. *)
		LIMIT := IN;
		IF LIMIT < MN THEN
			LIMIT := MN;
		END_IF;
		IF LIMIT > MX THEN
			LIMIT := MX;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: TIME
		VAR_INPUT
			MN: TIME;
			IN: TIME;
			MX: TIME;
		END_VAR

		(* IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX), so MX wins when MN > MX. *)
		LIMIT := IN;
		IF LIMIT < MN THEN
			LIMIT := MN;
		END_IF;
		IF LIMIT > MX THEN
			LIMIT := MX;
		END_IF;
	END_FUNCTION

	FUNCTION LIMIT: LTIME
		VAR_INPUT
			MN: LTIME;
			IN: LTIME;
			MX: LTIME;
		END_VAR

		(* IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX), so MX wins when MN > MX. *)
		LIMIT := IN;
		IF LIMIT < MN THEN
			LIMIT := MN;
		END_IF;
		IF LIMIT > MX THEN
			LIMIT := MX;
		END_IF;
	END_FUNCTION


	/* Returns the smaller of `IN1` and `IN2`.

	   Nest the calls for more than two values.

	   # Examples

	   ```iecst
	   n := MIN(3, 5);          // 3
	   n := MIN(7, MIN(9, 4));  // 4
	   ```
	*/
	FUNCTION MIN: SINT
		VAR_INPUT
			IN1: SINT;
			IN2: SINT;
		END_VAR

		IF IN1 <= IN2 THEN
			MIN := IN1;
		ELSE
			MIN := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MIN: INT
		VAR_INPUT
			IN1: INT;
			IN2: INT;
		END_VAR

		IF IN1 <= IN2 THEN
			MIN := IN1;
		ELSE
			MIN := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MIN: DINT
		VAR_INPUT
			IN1: DINT;
			IN2: DINT;
		END_VAR

		IF IN1 <= IN2 THEN
			MIN := IN1;
		ELSE
			MIN := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MIN: LINT
		VAR_INPUT
			IN1: LINT;
			IN2: LINT;
		END_VAR

		IF IN1 <= IN2 THEN
			MIN := IN1;
		ELSE
			MIN := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MIN: USINT
		VAR_INPUT
			IN1: USINT;
			IN2: USINT;
		END_VAR

		IF IN1 <= IN2 THEN
			MIN := IN1;
		ELSE
			MIN := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MIN: UINT
		VAR_INPUT
			IN1: UINT;
			IN2: UINT;
		END_VAR

		IF IN1 <= IN2 THEN
			MIN := IN1;
		ELSE
			MIN := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MIN: UDINT
		VAR_INPUT
			IN1: UDINT;
			IN2: UDINT;
		END_VAR

		IF IN1 <= IN2 THEN
			MIN := IN1;
		ELSE
			MIN := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MIN: ULINT
		VAR_INPUT
			IN1: ULINT;
			IN2: ULINT;
		END_VAR

		IF IN1 <= IN2 THEN
			MIN := IN1;
		ELSE
			MIN := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MIN: REAL
		VAR_INPUT
			IN1: REAL;
			IN2: REAL;
		END_VAR

		IF IN1 <= IN2 THEN
			MIN := IN1;
		ELSE
			MIN := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MIN: LREAL
		VAR_INPUT
			IN1: LREAL;
			IN2: LREAL;
		END_VAR

		IF IN1 <= IN2 THEN
			MIN := IN1;
		ELSE
			MIN := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MIN: TIME
		VAR_INPUT
			IN1: TIME;
			IN2: TIME;
		END_VAR

		IF IN1 <= IN2 THEN
			MIN := IN1;
		ELSE
			MIN := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MIN: LTIME
		VAR_INPUT
			IN1: LTIME;
			IN2: LTIME;
		END_VAR

		IF IN1 <= IN2 THEN
			MIN := IN1;
		ELSE
			MIN := IN2;
		END_IF;
	END_FUNCTION

	/* Returns the larger of `IN1` and `IN2`.

	   Nest the calls for more than two values.

	   # Examples

	   ```iecst
	   n := MAX(3, 5);          // 5
	   n := MAX(7, MAX(9, 4));  // 9
	   ```
	*/
	FUNCTION MAX: SINT
		VAR_INPUT
			IN1: SINT;
			IN2: SINT;
		END_VAR

		IF IN1 >= IN2 THEN
			MAX := IN1;
		ELSE
			MAX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MAX: INT
		VAR_INPUT
			IN1: INT;
			IN2: INT;
		END_VAR

		IF IN1 >= IN2 THEN
			MAX := IN1;
		ELSE
			MAX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MAX: DINT
		VAR_INPUT
			IN1: DINT;
			IN2: DINT;
		END_VAR

		IF IN1 >= IN2 THEN
			MAX := IN1;
		ELSE
			MAX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MAX: LINT
		VAR_INPUT
			IN1: LINT;
			IN2: LINT;
		END_VAR

		IF IN1 >= IN2 THEN
			MAX := IN1;
		ELSE
			MAX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MAX: USINT
		VAR_INPUT
			IN1: USINT;
			IN2: USINT;
		END_VAR

		IF IN1 >= IN2 THEN
			MAX := IN1;
		ELSE
			MAX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MAX: UINT
		VAR_INPUT
			IN1: UINT;
			IN2: UINT;
		END_VAR

		IF IN1 >= IN2 THEN
			MAX := IN1;
		ELSE
			MAX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MAX: UDINT
		VAR_INPUT
			IN1: UDINT;
			IN2: UDINT;
		END_VAR

		IF IN1 >= IN2 THEN
			MAX := IN1;
		ELSE
			MAX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MAX: ULINT
		VAR_INPUT
			IN1: ULINT;
			IN2: ULINT;
		END_VAR

		IF IN1 >= IN2 THEN
			MAX := IN1;
		ELSE
			MAX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MAX: REAL
		VAR_INPUT
			IN1: REAL;
			IN2: REAL;
		END_VAR

		IF IN1 >= IN2 THEN
			MAX := IN1;
		ELSE
			MAX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MAX: LREAL
		VAR_INPUT
			IN1: LREAL;
			IN2: LREAL;
		END_VAR

		IF IN1 >= IN2 THEN
			MAX := IN1;
		ELSE
			MAX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MAX: TIME
		VAR_INPUT
			IN1: TIME;
			IN2: TIME;
		END_VAR

		IF IN1 >= IN2 THEN
			MAX := IN1;
		ELSE
			MAX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MAX: LTIME
		VAR_INPUT
			IN1: LTIME;
			IN2: LTIME;
		END_VAR

		IF IN1 >= IN2 THEN
			MAX := IN1;
		ELSE
			MAX := IN2;
		END_IF;
	END_FUNCTION

	/* Returns the input that `K` selects, counting from 0.

	   There are overloads for two, three and four inputs. A `K` below 0 selects
	   the first, and a `K` past the last selects the last.

	   # Examples

	   ```iecst
	   n := MUX(1, 10, 20, 30);   // 20
	   n := MUX(-1, 10, 20, 30);  // 10
	   n := MUX(9, 10, 20, 30);   // 30
	   ```
	*/
	FUNCTION MUX: BOOL
		VAR_INPUT
			K: INT;
			IN0: BOOL;
			IN1: BOOL;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSE
			MUX := IN1;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: BOOL
		VAR_INPUT
			K: INT;
			IN0: BOOL;
			IN1: BOOL;
			IN2: BOOL;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSE
			MUX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: BOOL
		VAR_INPUT
			K: INT;
			IN0: BOOL;
			IN1: BOOL;
			IN2: BOOL;
			IN3: BOOL;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSIF K = 2 THEN
			MUX := IN2;
		ELSE
			MUX := IN3;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: INT
		VAR_INPUT
			K: INT;
			IN0: INT;
			IN1: INT;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSE
			MUX := IN1;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: INT
		VAR_INPUT
			K: INT;
			IN0: INT;
			IN1: INT;
			IN2: INT;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSE
			MUX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: INT
		VAR_INPUT
			K: INT;
			IN0: INT;
			IN1: INT;
			IN2: INT;
			IN3: INT;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSIF K = 2 THEN
			MUX := IN2;
		ELSE
			MUX := IN3;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: DINT
		VAR_INPUT
			K: INT;
			IN0: DINT;
			IN1: DINT;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSE
			MUX := IN1;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: DINT
		VAR_INPUT
			K: INT;
			IN0: DINT;
			IN1: DINT;
			IN2: DINT;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSE
			MUX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: DINT
		VAR_INPUT
			K: INT;
			IN0: DINT;
			IN1: DINT;
			IN2: DINT;
			IN3: DINT;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSIF K = 2 THEN
			MUX := IN2;
		ELSE
			MUX := IN3;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: LINT
		VAR_INPUT
			K: INT;
			IN0: LINT;
			IN1: LINT;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSE
			MUX := IN1;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: LINT
		VAR_INPUT
			K: INT;
			IN0: LINT;
			IN1: LINT;
			IN2: LINT;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSE
			MUX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: LINT
		VAR_INPUT
			K: INT;
			IN0: LINT;
			IN1: LINT;
			IN2: LINT;
			IN3: LINT;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSIF K = 2 THEN
			MUX := IN2;
		ELSE
			MUX := IN3;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: REAL
		VAR_INPUT
			K: INT;
			IN0: REAL;
			IN1: REAL;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSE
			MUX := IN1;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: REAL
		VAR_INPUT
			K: INT;
			IN0: REAL;
			IN1: REAL;
			IN2: REAL;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSE
			MUX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: REAL
		VAR_INPUT
			K: INT;
			IN0: REAL;
			IN1: REAL;
			IN2: REAL;
			IN3: REAL;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSIF K = 2 THEN
			MUX := IN2;
		ELSE
			MUX := IN3;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: LREAL
		VAR_INPUT
			K: INT;
			IN0: LREAL;
			IN1: LREAL;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSE
			MUX := IN1;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: LREAL
		VAR_INPUT
			K: INT;
			IN0: LREAL;
			IN1: LREAL;
			IN2: LREAL;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSE
			MUX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: LREAL
		VAR_INPUT
			K: INT;
			IN0: LREAL;
			IN1: LREAL;
			IN2: LREAL;
			IN3: LREAL;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSIF K = 2 THEN
			MUX := IN2;
		ELSE
			MUX := IN3;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: TIME
		VAR_INPUT
			K: INT;
			IN0: TIME;
			IN1: TIME;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSE
			MUX := IN1;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: TIME
		VAR_INPUT
			K: INT;
			IN0: TIME;
			IN1: TIME;
			IN2: TIME;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSE
			MUX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: TIME
		VAR_INPUT
			K: INT;
			IN0: TIME;
			IN1: TIME;
			IN2: TIME;
			IN3: TIME;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSIF K = 2 THEN
			MUX := IN2;
		ELSE
			MUX := IN3;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: STRING
		VAR_INPUT
			K: INT;
			IN0: STRING;
			IN1: STRING;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSE
			MUX := IN1;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: STRING
		VAR_INPUT
			K: INT;
			IN0: STRING;
			IN1: STRING;
			IN2: STRING;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSE
			MUX := IN2;
		END_IF;
	END_FUNCTION

	FUNCTION MUX: STRING
		VAR_INPUT
			K: INT;
			IN0: STRING;
			IN1: STRING;
			IN2: STRING;
			IN3: STRING;
		END_VAR

		IF K <= 0 THEN
			MUX := IN0;
		ELSIF K = 1 THEN
			MUX := IN1;
		ELSIF K = 2 THEN
			MUX := IN2;
		ELSE
			MUX := IN3;
		END_IF;
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit
		USING Std.Selection

		// SEL tests

		{test}
		FUNCTION test_sel_true_returns_in1 ASSERT_EQ(SEL(TRUE, 10, 42), 42);
		END_FUNCTION

		{test}
		FUNCTION test_sel_false_returns_in0 ASSERT_EQ(SEL(FALSE, 10, 42), 10);
		END_FUNCTION

		{test}
		FUNCTION test_sel_real_true ASSERT_EQ(SEL(TRUE, 1.0, 3.14), 3.14);
		END_FUNCTION

		{test}
		FUNCTION test_sel_real_false ASSERT_EQ(SEL(FALSE, 1.0, 3.14), 1.0);
		END_FUNCTION

		// LIMIT tests

		{test}
		FUNCTION test_limit_in_range ASSERT_EQ(LIMIT(0, 5, 10), 5);
		END_FUNCTION

		{test}
		FUNCTION test_limit_below_min ASSERT_EQ(LIMIT(0, -5, 10), 0);
		END_FUNCTION

		{test}
		FUNCTION test_limit_above_max ASSERT_EQ(LIMIT(0, 15, 10), 10);
		END_FUNCTION

		{test}
		FUNCTION test_limit_at_min ASSERT_EQ(LIMIT(0, 0, 10), 0);
		END_FUNCTION

		{test}
		FUNCTION test_limit_at_max ASSERT_EQ(LIMIT(0, 10, 10), 10);
		END_FUNCTION

		{test}
		FUNCTION test_limit_real_clamp_low ASSERT_EQ(LIMIT(0.0, -1.5, 10.0), 0.0);
		END_FUNCTION

		{test}
		FUNCTION test_limit_real_clamp_high ASSERT_EQ(LIMIT(0.0, 15.5, 10.0), 10.0);
		END_FUNCTION

		{test}
		FUNCTION test_limit_real_in_range ASSERT_EQ(LIMIT(0.0, 5.5, 10.0), 5.5);
		END_FUNCTION

		// --- nonzero-MN clamps: a LIMIT body that never assigns its result
		// would return the zero-initialized local and still pass every
		// clamp-low test above. Pin clamps whose expected value is NOT 0 ---

		{test}
		FUNCTION test_limit_nonzero_mn_clamp
			ASSERT_EQ(LIMIT(INT#20, INT#5, INT#30), INT#20, 'clamp low to a nonzero MN');
			ASSERT_EQ(LIMIT(INT#-30, INT#-50, INT#-10), INT#-30, 'negative bounds clamp low');
			ASSERT_EQ(LIMIT(INT#-30, INT#0, INT#-10), INT#-10, 'negative bounds clamp high');
		END_FUNCTION

		// IEC 61131-3 Table 32.5: OUT := MIN(MAX(IN, MN), MX). With the
		// degenerate MN > MX, applying MX last means MX wins.
		{test}
		FUNCTION test_limit_mn_greater_than_mx
			ASSERT_EQ(LIMIT(INT#10, INT#5, INT#0), INT#0, 'MX wins when MN > MX');
		END_FUNCTION

		// --- typed overload coverage: each overload is independent compiled
		// code; the untyped-literal tests above only ever reach INT/REAL ---

		{test}
		FUNCTION test_limit_lint_i64_lane
			ASSERT_EQ(LIMIT(LINT#-5000000000, LINT#-6000000000, LINT#5000000000), LINT#-5000000000, 'i64 clamp low beyond i32 range');
			ASSERT_EQ(LIMIT(LINT#0, LINT#6000000000, LINT#5000000000), LINT#5000000000, 'i64 clamp high beyond i32 range');
		END_FUNCTION

		{test}
		FUNCTION test_limit_udint_above_signed_max
			ASSERT_EQ(LIMIT(UDINT#0, UDINT#4000000000, UDINT#3000000000), UDINT#3000000000, 'unsigned compare above 2^31 must not go signed');
		END_FUNCTION

		{test}
		FUNCTION test_limit_sint_boundaries
			ASSERT_EQ(LIMIT(SINT#-128, SINT#-128, SINT#127), SINT#-128, 'SINT MIN passes through');
			ASSERT_EQ(LIMIT(SINT#-100, SINT#127, SINT#100), SINT#100, 'SINT clamp high');
		END_FUNCTION

		{test}
		FUNCTION test_limit_time
			ASSERT(LIMIT(T#1s, T#500ms, T#10s) = T#1s, 'TIME clamp low');
			ASSERT(LIMIT(T#1s, T#20s, T#10s) = T#10s, 'TIME clamp high');
		END_FUNCTION

		{test}
		FUNCTION test_sel_byte
			ASSERT_EQ(SEL(FALSE, BYTE#16#AA, BYTE#16#55), BYTE#16#AA, 'SEL BYTE G=FALSE');
			ASSERT_EQ(SEL(TRUE, BYTE#16#AA, BYTE#16#55), BYTE#16#55, 'SEL BYTE G=TRUE');
		END_FUNCTION

		{test}
		FUNCTION test_sel_lword_i64_lane
			ASSERT_EQ(SEL(TRUE, LWORD#1, LWORD#16#8000_0000_0000_0000), LWORD#16#8000_0000_0000_0000, 'SEL LWORD picks a sign-bit i64 value');
		END_FUNCTION

		{test}
		FUNCTION test_sel_lint_boundaries
			ASSERT_EQ(SEL(FALSE, LINT#-9223372036854775807, LINT#0), LINT#-9223372036854775807, 'SEL LINT near i64 MIN');
		END_FUNCTION

		{test}
		FUNCTION test_sel_time
			ASSERT(SEL(TRUE, T#1s, T#2s) = T#2s, 'SEL TIME');
		END_FUNCTION

		{test}
		FUNCTION test_sel_bool
			ASSERT_EQ(SEL(TRUE, FALSE, TRUE), TRUE, 'SEL BOOL both branches distinct');
			ASSERT_EQ(SEL(FALSE, FALSE, TRUE), FALSE, 'SEL BOOL G=FALSE');
		END_FUNCTION

		// -- MIN / MAX ------------------------------------------------

		{test}
		FUNCTION test_min_max_int
			ASSERT_EQ(MIN(3, 5), 3);
			ASSERT_EQ(MAX(3, 5), 5);
			ASSERT_EQ(MIN(-2, 2), -2);
			ASSERT_EQ(MAX(-2, 2), 2);
		END_FUNCTION

		{test}
		FUNCTION test_min_max_real
			ASSERT_EQ(MIN(1.5, 2.5), 1.5);
			ASSERT_EQ(MAX(1.5, 2.5), 2.5);
		END_FUNCTION

		{test}
		FUNCTION test_min_max_time
			ASSERT_EQ(MIN(T#1s, T#2s), T#1s);
			ASSERT_EQ(MAX(T#1s, T#2s), T#2s);
		END_FUNCTION

		{test}
		FUNCTION test_min_max_nest
			ASSERT_EQ(MIN(7, MIN(9, 4)), 4);
			ASSERT_EQ(MAX(7, MAX(9, 4)), 9);
		END_FUNCTION

		// -- MUX ------------------------------------------------------

		{test}
		FUNCTION test_mux_selects
			ASSERT_EQ(MUX(0, 10, 20), 10);
			ASSERT_EQ(MUX(1, 10, 20), 20);
			ASSERT_EQ(MUX(1, 10, 20, 30), 20);
			ASSERT_EQ(MUX(2, 10, 20, 30), 30);
			ASSERT_EQ(MUX(3, 10, 20, 30, 40), 40);
		END_FUNCTION

		{test}
		FUNCTION test_mux_clamps
			ASSERT_EQ(MUX(-1, 10, 20, 30), 10);
			ASSERT_EQ(MUX(9, 10, 20, 30), 30);
		END_FUNCTION

		{test}
		FUNCTION test_mux_string
			VAR
				s: STRING;
			END_VAR
			s := MUX(1, 'low', 'high');
			ASSERT_EQ(s, 'high');
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
