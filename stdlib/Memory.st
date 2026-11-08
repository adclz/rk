/*
Memory functions.
*/
NAMESPACE Std.Memory

	/* Returns `IN`, and copies it into `OUT`.

	   There is one overload per elementary type.

	   # Examples

	   ```iecst
	   x := MOVE(42);       // x is 42
	   MOVE(42, OUT => x);  // x is 42
	   ```
	*/
	FUNCTION MOVE: BOOL
		VAR_INPUT
			IN: BOOL;
		END_VAR
		VAR_OUTPUT
			OUT: BOOL;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: SINT
		VAR_INPUT
			IN: SINT;
		END_VAR
		VAR_OUTPUT
			OUT: SINT;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: INT
		VAR_INPUT
			IN: INT;
		END_VAR
		VAR_OUTPUT
			OUT: INT;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: DINT
		VAR_INPUT
			IN: DINT;
		END_VAR
		VAR_OUTPUT
			OUT: DINT;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: LINT
		VAR_INPUT
			IN: LINT;
		END_VAR
		VAR_OUTPUT
			OUT: LINT;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: USINT
		VAR_INPUT
			IN: USINT;
		END_VAR
		VAR_OUTPUT
			OUT: USINT;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: UINT
		VAR_INPUT
			IN: UINT;
		END_VAR
		VAR_OUTPUT
			OUT: UINT;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: UDINT
		VAR_INPUT
			IN: UDINT;
		END_VAR
		VAR_OUTPUT
			OUT: UDINT;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: ULINT
		VAR_INPUT
			IN: ULINT;
		END_VAR
		VAR_OUTPUT
			OUT: ULINT;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: BYTE
		VAR_INPUT
			IN: BYTE;
		END_VAR
		VAR_OUTPUT
			OUT: BYTE;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: WORD
		VAR_INPUT
			IN: WORD;
		END_VAR
		VAR_OUTPUT
			OUT: WORD;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: DWORD
		VAR_INPUT
			IN: DWORD;
		END_VAR
		VAR_OUTPUT
			OUT: DWORD;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: LWORD
		VAR_INPUT
			IN: LWORD;
		END_VAR
		VAR_OUTPUT
			OUT: LWORD;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: REAL
		VAR_INPUT
			IN: REAL;
		END_VAR
		VAR_OUTPUT
			OUT: REAL;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: LREAL
		VAR_INPUT
			IN: LREAL;
		END_VAR
		VAR_OUTPUT
			OUT: LREAL;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: TIME
		VAR_INPUT
			IN: TIME;
		END_VAR
		VAR_OUTPUT
			OUT: TIME;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: LTIME
		VAR_INPUT
			IN: LTIME;
		END_VAR
		VAR_OUTPUT
			OUT: LTIME;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: STRING[255]
		VAR_INPUT
			IN: STRING[255];
		END_VAR
		VAR_OUTPUT
			OUT: STRING[255];
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: CHAR
		VAR_INPUT
			IN: CHAR;
		END_VAR
		VAR_OUTPUT
			OUT: CHAR;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: DATE
		VAR_INPUT
			IN: DATE;
		END_VAR
		VAR_OUTPUT
			OUT: DATE;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: LDATE
		VAR_INPUT
			IN: LDATE;
		END_VAR
		VAR_OUTPUT
			OUT: LDATE;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: TOD
		VAR_INPUT
			IN: TOD;
		END_VAR
		VAR_OUTPUT
			OUT: TOD;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: LTOD
		VAR_INPUT
			IN: LTOD;
		END_VAR
		VAR_OUTPUT
			OUT: LTOD;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: DT
		VAR_INPUT
			IN: DT;
		END_VAR
		VAR_OUTPUT
			OUT: DT;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	FUNCTION MOVE: LDT
		VAR_INPUT
			IN: LDT;
		END_VAR
		VAR_OUTPUT
			OUT: LDT;
		END_VAR

		OUT := IN;
		MOVE := IN;
	END_FUNCTION

	NAMESPACE Test
		USING Std.Unit
		USING Std.Memory
		USING Std.Strings

		{test}
		FUNCTION test_move_int
			VAR
				x: INT := 0;
			END_VAR
			MOVE(42, OUT => x);
			ASSERT_EQ(x, 42);
		END_FUNCTION

		{test}
		FUNCTION test_move_real
			VAR
				x: REAL := 0.0;
			END_VAR
			MOVE(3.14, OUT => x);
			ASSERT_EQ(x, 3.14);
		END_FUNCTION

		{test}
		FUNCTION test_move_bool
			VAR
				x: BOOL := FALSE;
			END_VAR
			MOVE(TRUE, OUT => x);
			ASSERT(x);
		END_FUNCTION

		// --- every overload is independent compiled code: cover the 16 dark
		// ones, each with a boundary value for its type. The i64 lane
		// (LWORD/LINT/ULINT/LREAL/LTIME) is the historically bug-prone path ---

		{test}
		FUNCTION test_move_signed_boundaries
			VAR
				s: SINT;
				d: DINT;
			END_VAR
			MOVE(SINT#-128, OUT => s);
			ASSERT_EQ(s, SINT#-128, 'MOVE SINT at MIN');
			MOVE(DINT#-2147483648, OUT => d);
			ASSERT_EQ(d, DINT#-2147483648, 'MOVE DINT at MIN');
		END_FUNCTION

		{test}
		FUNCTION test_move_unsigned_boundaries
			VAR
				us: USINT;
				u: UINT;
				ud: UDINT;
			END_VAR
			MOVE(USINT#255, OUT => us);
			ASSERT_EQ(us, USINT#255, 'MOVE USINT at MAX');
			MOVE(UINT#65535, OUT => u);
			ASSERT_EQ(u, UINT#65535, 'MOVE UINT at MAX');
			MOVE(UDINT#4294967295, OUT => ud);
			ASSERT_EQ(ud, UDINT#4294967295, 'MOVE UDINT at MAX (above 2^31)');
		END_FUNCTION

		{test}
		FUNCTION test_move_bit_types_sign_bit
			VAR
				b: BYTE;
				w: WORD;
				dw: DWORD;
			END_VAR
			MOVE(BYTE#16#80, OUT => b);
			ASSERT_EQ(b, BYTE#16#80, 'MOVE BYTE with MSB set');
			MOVE(WORD#16#8000, OUT => w);
			ASSERT_EQ(w, WORD#16#8000, 'MOVE WORD with MSB set');
			MOVE(DWORD#16#80000000, OUT => dw);
			ASSERT_EQ(dw, DWORD#16#80000000, 'MOVE DWORD with sign bit set');
		END_FUNCTION

		{test}
		FUNCTION test_move_i64_lane
			VAR
				lw: LWORD;
				li: LINT;
				ul: ULINT;
				lr: LREAL;
			END_VAR
			MOVE(LWORD#16#8000_0000_0000_0000, OUT => lw);
			ASSERT_EQ(lw, LWORD#16#8000_0000_0000_0000, 'MOVE LWORD sign bit');
			MOVE(LINT#-9223372036854775807, OUT => li);
			ASSERT_EQ(li, LINT#-9223372036854775807, 'MOVE LINT near i64 MIN');
			MOVE(ULINT#18446744073709551615, OUT => ul);
			ASSERT_EQ(ul, ULINT#18446744073709551615, 'MOVE ULINT at u64 MAX');
			MOVE(LREAL#1.7976931348623157, OUT => lr);
			ASSERT_EQ(lr, LREAL#1.7976931348623157, 'MOVE LREAL f64 payload');
		END_FUNCTION

		{test}
		FUNCTION test_move_time_types
			VAR
				t: TIME;
				lt: LTIME;
			END_VAR
			MOVE(T#24d20h31m23s647ms, OUT => t);
			ASSERT(t = T#24d20h31m23s647ms, 'MOVE TIME at the signed-ms ceiling');
			MOVE(LTIME#1000d, OUT => lt);
			ASSERT(lt = LTIME#1000d, 'MOVE LTIME on the i64-ns lane');
		END_FUNCTION

		{test}
		FUNCTION test_move_char
			VAR
				c: CHAR;
			END_VAR
			MOVE(CHAR#'z', OUT => c);
			ASSERT_EQ(c, CHAR#'z', 'MOVE CHAR');
		END_FUNCTION

		{test}
		FUNCTION test_move_dates_and_times_of_day
			VAR
				d: DATE;
				ld: LDATE;
				t: TOD;
				lt: LTOD;
				dt1: DT;
				ldt1: LDT;
			END_VAR
			MOVE(D#2024-01-31, OUT => d);
			ASSERT(d = D#2024-01-31, 'MOVE DATE');
			ld := MOVE(LDATE#2024-02-29);
			ASSERT(ld = LDATE#2024-02-29, 'MOVE LDATE');
			MOVE(TOD#12:30:45.123, OUT => t);
			ASSERT(t = TOD#12:30:45.123, 'MOVE TOD');
			lt := MOVE(LTOD#23:59:59.999999999);
			ASSERT(lt = LTOD#23:59:59.999999999, 'MOVE LTOD on the i64-ns lane');
			MOVE(DT#2024-01-31-12:30:45, OUT => dt1);
			ASSERT(dt1 = DT#2024-01-31-12:30:45, 'MOVE DT');
			ldt1 := MOVE(LDT#2024-01-31-12:30:45.5);
			ASSERT(ldt1 = LDT#2024-01-31-12:30:45.5, 'MOVE LDT');
		END_FUNCTION

		{test}
		FUNCTION test_move_string
			VAR
				s: STRING[255];
			END_VAR
			MOVE('edge case', OUT => s);
			ASSERT(LEN(s) = 9, 'MOVE STRING length');
			ASSERT(FIND(s, 'edge case') = 1, 'MOVE STRING content');
		END_FUNCTION

		// Its result is OUT, on each type.
		{test}
		FUNCTION test_move_returns_each_type
			VAR
				b1: BOOL := TRUE;
				b2: BOOL;
				si1: SINT := -5;
				si2: SINT;
				i1: INT := -500;
				i2: INT;
				d1: DINT := -70000;
				d2: DINT;
				l1: LINT := -5000000000;
				l2: LINT;
				us1: USINT := 200;
				us2: USINT;
				u1: UINT := 60000;
				u2: UINT;
				ud1: UDINT := 4000000000;
				ud2: UDINT;
				ul1: ULINT := 18446744073709551615;
				ul2: ULINT;
				by1: BYTE := 16#AB;
				by2: BYTE;
				w1: WORD := 16#ABCD;
				w2: WORD;
				dw1: DWORD := 16#ABCDEF01;
				dw2: DWORD;
				lw1: LWORD := 16#ABCDEF0123456789;
				lw2: LWORD;
				r1: REAL := 1.5;
				r2: REAL;
				lr1: LREAL := 2.5;
				lr2: LREAL;
				t1: TIME := T#3s;
				t2: TIME;
				lt1: LTIME := LTIME#4s;
				lt2: LTIME;
				s1: STRING := 'text';
				s2: STRING;
				c1: CHAR := CHAR#'a';
				c2: CHAR;
			END_VAR
			b2 := MOVE(b1);
			ASSERT(b2);
			si2 := MOVE(si1);
			ASSERT(si2 = -5);
			i2 := MOVE(i1);
			ASSERT(i2 = -500);
			d2 := MOVE(d1);
			ASSERT(d2 = -70000);
			l2 := MOVE(l1);
			ASSERT(l2 = -5000000000);
			us2 := MOVE(us1);
			ASSERT(us2 = 200);
			u2 := MOVE(u1);
			ASSERT(u2 = 60000);
			ud2 := MOVE(ud1);
			ASSERT(ud2 = 4000000000);
			ul2 := MOVE(ul1);
			ASSERT(ul2 = 18446744073709551615);
			by2 := MOVE(by1);
			ASSERT(by2 = 16#AB);
			w2 := MOVE(w1);
			ASSERT(w2 = 16#ABCD);
			dw2 := MOVE(dw1);
			ASSERT(dw2 = 16#ABCDEF01);
			lw2 := MOVE(lw1);
			ASSERT(lw2 = 16#ABCDEF0123456789);
			r2 := MOVE(r1);
			ASSERT(r2 = 1.5);
			lr2 := MOVE(lr1);
			ASSERT(lr2 = 2.5);
			t2 := MOVE(t1);
			ASSERT(t2 = T#3s);
			lt2 := MOVE(lt1);
			ASSERT(lt2 = LTIME#4s);
			s2 := MOVE(s1);
			ASSERT(s2 = 'text');
			c2 := MOVE(c1);
			ASSERT(c2 = CHAR#'a');
		END_FUNCTION

		// A literal takes the type of what it is moved into.
		{test}
		FUNCTION test_move_a_literal
			VAR
				i: INT;
				d: DINT;
				l: LINT;
				u: UINT;
				x: REAL;
				y: LREAL;
				s: STRING;
				b: BOOL;
				w: WORD;
				t: TIME;
			END_VAR
			i := MOVE(5);
			ASSERT(i = 5);
			d := MOVE(70000);
			ASSERT(d = 70000);
			l := MOVE(5000000000);
			ASSERT(l = 5000000000);
			u := MOVE(60000);
			ASSERT(u = 60000);
			x := MOVE(1.5);
			ASSERT(x = 1.5);
			y := MOVE(2.5);
			ASSERT(y = 2.5);
			s := MOVE('abc');
			ASSERT(s = 'abc');
			b := MOVE(TRUE);
			ASSERT(b);
			w := MOVE(WORD#1);
			ASSERT(w = 1);
			t := MOVE(T#1s);
			ASSERT(t = T#1s);
		END_FUNCTION

		// The value widens into a wider target, as any value does.
		{test}
		FUNCTION test_move_into_a_wider_target
			VAR
				si: SINT := -3;
				i: INT := -3;
				d: DINT;
				l: LINT;
				x: REAL;
			END_VAR
			d := MOVE(i);
			ASSERT(d = -3);
			l := MOVE(si);
			ASSERT(l = -3);
			x := MOVE(i);
			ASSERT(x = -3.0);
		END_FUNCTION

		{test}
		FUNCTION test_move_in_an_expression
			VAR
				i: INT := 4;
				b: BOOL := TRUE;
				a: ARRAY[0..4] OF INT;
			END_VAR
			ASSERT(MOVE(i) = 4);
			ASSERT(MOVE(i) + 1 = 5);
			ASSERT(MOVE(MOVE(i)) = 4);
			ASSERT(MOVE(b));
			a[MOVE(i)] := 9;
			ASSERT(a[4] = 9);
			ASSERT(MOVE(a[MOVE(i)]) = 9);
		END_FUNCTION

		// A call may name its output as well.
		{test}
		FUNCTION test_move_returns_what_it_writes
			VAR
				x: INT;
				y: INT;
			END_VAR
			x := MOVE(IN := 6, OUT => y);
			ASSERT(x = 6);
			ASSERT(y = 6);
		END_FUNCTION

		{test}
		FUNCTION test_move_string_past_80_bytes
			VAR
				long: STRING[200] := '01234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789';
				s: STRING[255];
			END_VAR
			MOVE(long, OUT => s);
			ASSERT(LEN(s) = 200, 'MOVE keeps the bytes past 80');
		END_FUNCTION
	END_NAMESPACE
END_NAMESPACE
