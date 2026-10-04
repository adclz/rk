
FUNCTION f : INT
VAR
    x : INT; // one
    y : INT; (* two *)
END_VAR // after the section
    x := 1; // one
    y := 2; (* two *)
    IF x = 1 THEN // why
        x := 3; // three
        // alone
    ELSE // other
        x := 4;
    END_IF; // closed
    // alone again
    f := x; /* last */
END_FUNCTION // done
