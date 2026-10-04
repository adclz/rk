
        TYPE
            Matrix: ARRAY[1..2, 1..3] OF INT{export};
        END_TYPE

        FUNCTION Test
            VAR
                Data : Matrix := [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
            END_VAR
        END_FUNCTION
        