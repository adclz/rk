TYPE
    Step : (A := 5, B, C);
    Holder : STRUCT s : Step; END_STRUCT;
END_TYPE
FUNCTION test : INT
VAR
    h : Holder;
END_VAR
    IF h.s ! Step#A THEN test := test + 1; END_IF;
END_FUNCTION
