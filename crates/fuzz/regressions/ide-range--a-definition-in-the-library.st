
        TYPE Holder : STRUCT p : REF_TO INT; END_STRUCT; END_TYPE

        FUNCTION test : INT
        VAR
            x : INT :=D_TYPE

        FUNCTION_BLOCK Holder
        VAR_OUTPUT r : REAL; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION Half : REAL
            Half := 0.5;
            Half.31 := TRUE; 42;
            h : Holder := (p := REF(x));
        END_VAR
            test := h.p^;
        END_NCTCTION
    