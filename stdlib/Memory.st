NAMESPACE ns 
       // 7. Move (note g: IN=ANY, OUT=ANY)
    FUNCTION MOVE<T: ANY> : T
        VAR_INPUT
            args: T...
        END_VAR
    END_FUNCTION
END_NAMESPACE