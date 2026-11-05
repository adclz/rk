TYPE
    A : ARRAY[0..1] OF REF_TO A;
    P : REF_TO P;
    Q : REF_TO R;
    R : REF_TO Q;
END_TYPE

PROGRAM Main
VAR
    a1, a2 : A;
    p1, p2 : P;
    q1 : Q;
    r1 : R;
    same : BOOL;
END_VAR
    a1[0] := REF(a2);
    a1[0]^[1] := REF(a1);
    p1 := REF(p2);
    p2 := p1;
    q1 := REF(r1);
    r1 := REF(q1);
    same := q1^ = r1;
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Main;
    END_RESOURCE
END_CONFIGURATION
