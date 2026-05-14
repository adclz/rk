use crate::tests::{
    lsp::formatter::fmt,
    utils::{add_sources, with_db},
};
use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

#[rstest]
pub fn counters_ctu(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK CTUD
  VAR_INPUT
    CU : BOOL;
    CD : BOOL;
    R  : BOOL;
    LD : BOOL;
    PV : INT;
  END_VAR
  VAR_OUTPUT
    QU : BOOL;
    QD : BOOL;
    CV : INT;
  END_VAR
  VAR
    CD_T: R_TRIG;
    CU_T: R_TRIG;
  END_VAR

  CD_T(CD);
  CU_T(CU);

  IF R THEN CV := 0 ;
  ELSIF LD THEN CV := PV ;
  ELSE
    IF NOT (CU_T.Q AND CD_T.Q) THEN
      IF CU_T.Q AND (CV < PV)
      THEN CV := CV+1;
      ELSIF CD_T.Q AND (CV > 0)
      THEN CV := CV-1;
      END_IF;
    END_IF;
  END_IF ;
  QU := (CV >= PV) ;
  QD := (CV <= 0) ;
END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let document = with_db
        .get_files()
        .iter()
        .last()
        .unwrap()
        .document(&with_db);

    assert_snapshot!(fmt(document), @r"
    FUNCTION_BLOCK CTUD
    	VAR_INPUT
    		CU: BOOL;
    		CD: BOOL;
    		R: BOOL;
    		LD: BOOL;
    		PV: INT;
    	END_VAR
    	VAR_OUTPUT
    		QU: BOOL;
    		QD: BOOL;
    		CV: INT;
    	END_VAR
    	VAR
    		CD_T: R_TRIG;
    		CU_T: R_TRIG;
    	END_VAR

    	CD_T(CD);
    	CU_T(CU);
    	IF R THEN
    		CV := 0;
    	ELSIF LD THEN
    		CV := PV;
    	ELSE
    		IF NOT (CU_T.Q AND CD_T.Q) THEN
    			IF CU_T.Q AND (CV < PV) THEN
    				CV := CV + 1;
    			ELSIF CD_T.Q AND (CV > 0) THEN
    				CV := CV - 1;
    			END_IF;
    		END_IF;
    	END_IF;
    	QU := (CV >= PV);
    	QD := (CV <= 0);
    END_FUNCTION_BLOCK
    ");
}
