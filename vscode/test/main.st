(* A small conveyor: a ramp drives the belt, a counter tallies parts. *)

FUNCTION_BLOCK Ramp522
	VAR_INPUT
		target: DINT;
		rate: DINT;
	END_VAR
	VAR_OUTPUT
		speed: DINT;
		at_top_changed: BOOL;
	END_VAR
	VAR
		acc: DINT;
	END_VAR
	acc := acc + rate; (* <- breakpoint here for an FB frame *)
	IF acc > target THEN
		acc := target;
	END_IF;
	speed := acc;
	at_top_changed := acc >= target;
END_FUNCTION_BLOCK

FUNCTION scale: DINT
	VAR_INPUT
		value: DINT;
		factor: DINT;
	END_VAR
	VAR
		doubled: DINT;
		biased: DINT;
	END_VAR
	doubled := value * factor; (* <- breakpoint here to see a Locals scope *)
	biased := doubled + 7;
	scale := biased;
END_FUNCTION
PROGRAM Main
	VAR
		belt: Ramp522;
		parts: DINT;
		scaled: DINT;
		running: BOOL;
		label: STRING[16] := 'conveyor-1';
	END_VAR
	belt(target := 1000, rate := 37);
	parts := parts + 1; (* <- breakpoint here, then F10/F11 *)

	scaled := scale(value := parts, factor := 3);
	running := belt.at_top_changed;
END_PROGRAM
CONFIGURATION Cfg
	RESOURCE Res ON CPU
		TASK Cycle(INTERVAL := T#100ms, PRIORITY := 1);
		PROGRAM P1 WITH Cycle: Main;
	END_RESOURCE
END_CONFIGURATION


NAMESPACE namespace.thing 
{test}
FUNCTION tets 
	VAR 
		test: INT := 0;
	END_VAR


	
END_FUNCTION
END_NAMESPACE
