
        PROGRAM P
        VAR RETAIN seen : STRING[10]; END_VAR
        VAR src : STRING[8]; END_VAR
            src := 'abcdef';
            g := src;   (* from a variable: an over-long literal is E0306 *)
            seen := g;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL g : STRING[4]; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    