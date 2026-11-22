
        TYPE Tally : INT := 5; END_TYPE
        TYPE B8 : BYTE := 16#FF; END_TYPE

        FUNCTION_BLOCK Drive
        VAR
            m AT %M* : Tally;
            b AT %Q* : B8;
            c AT %M* : Tally;
        END_VAR
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR d : Drive; END_VAR
            d();
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            g AT %MW0 : INT := 7;
            w AT %QD0 : DWORD := 16#11223344;
        END_VAR
        VAR_CONFIG
            Res.P1.d.m AT %MW0 : Tally;
            Res.P1.d.b AT %QB2 : B8;
            Res.P1.d.c AT %MW1 : Tally;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    
// v2
