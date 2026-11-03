
        FUNCTION run : DINT
        VAR inner : STRING; outer : STRING; END_VAR
            __TRY
                __TRY
                    __RAISE('first');
                __CATCH(inner)
                    run := 1;
                __END_TRY;
                __TRY
                    __RAISE('second');
                __CATCH(inner)
                    __RAISE(inner);
                __END_TRY;
  + 10;
                END_IF;
            __END_TRY;
        END_FUN/*ION
    