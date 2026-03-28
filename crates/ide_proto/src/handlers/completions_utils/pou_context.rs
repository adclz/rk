use std::sync::LazyLock;

use auto_lsp::core::span::Span;
use auto_lsp::tree_sitter::{self, StreamingIterator};

static VAR_DECLS: &str = r#"
[
    (method_decl)
    (method_prototype)
] @method

[
    (var_decls)
] @var_decls

[
    (input_decls)
    (fb_input_decls)
] @input_decls

[
    (output_decls)
    (fb_output_decls)
] @output_decls

[
    (in_out_decls)
] @in_out_decls

 [
    (temp_var_decls)
] @temp_var_decls

[
    (func_body)
    (fb_body)
] @body
"#;

pub static FOLD_QUERY: LazyLock<tree_sitter::Query> = LazyLock::new(|| {
    tree_sitter::Query::new(&tree_sitter_rk::LANGUAGE.into(), VAR_DECLS)
        .expect("Failed to create fold query")
});

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct VarSection: u8 {
        const INPUTS = 1 << 0;
        const OUTPUTS =  1 << 1;
        const IN_OUTS = 1 << 2;
        const TEMPS = 1 << 3;
        const VARS = 1 << 4;
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeadLocation {
    BeforeVars, // suggests IMPLEMENTS/EXTENDS and var snippets
    InVars,     // suggests var snippets
    // Methods are always defined after vars
    BeforeMethods,      // suggests var and method snippets
    InMethods,          // suggests method snippets
    InBodyAfterVars,    // suggests var and method snippets
    InBodyAfterMethods, //suggests var and method snippets
    #[default]
    InBody, // suggests *nothing*
}

impl HeadLocation {
    /// Returns true if the location is inside the body of the POU (after vars and methods)
    pub fn is_in_body(&self) -> bool {
        matches!(
            self,
            HeadLocation::InBody | HeadLocation::InBodyAfterMethods | HeadLocation::InBodyAfterVars
        )
    }
}

#[derive(Debug)]
pub struct HeadResult {
    pub head_location: HeadLocation,
    pub inside_var_section: VarSection,
    pub inputs: Option<Span>,
    pub outputs: Option<Span>,
    pub in_outs: Option<Span>,
    pub temps: Option<Span>,
    pub vars: Option<Span>,
}

impl HeadResult {
    pub fn is_inside_var_section(&self) -> bool {
        !self.inside_var_section.is_empty()
    }

    pub fn active_variable_sections(&self) -> VarSection {
        let mut result = VarSection::empty();
        if self.inputs.is_some() {
            result |= VarSection::INPUTS;
        }
        if self.outputs.is_some() {
            result |= VarSection::OUTPUTS;
        }
        if self.in_outs.is_some() {
            result |= VarSection::IN_OUTS;
        }
        if self.temps.is_some() {
            result |= VarSection::TEMPS;
        }
        if self.vars.is_some() {
            result |= VarSection::VARS;
        }
        result
    }

    pub fn inactive_variable_sections(&self) -> VarSection {
        // Return sections that are NOT active, but only for INPUTS, OUTPUTS, IN_OUTS
        // (excludes TEMPS and VARS which are function block specific)
        let considered_sections = VarSection::INPUTS | VarSection::OUTPUTS | VarSection::IN_OUTS;
        considered_sections & !self.active_variable_sections()
    }

    pub fn query_var_decls(
        root_node: tree_sitter::Node,
        source: &str,
        range: Span,
        offset: usize,
    ) -> Self {
        let mut query_cursor = tree_sitter::QueryCursor::new();
        query_cursor.set_point_range(range.start_point..range.end_point);
        let mut captures = query_cursor.captures(&FOLD_QUERY, root_node, source.as_bytes());

        let mut inputs = None;
        let mut outputs = None;
        let mut in_outs = None;
        let mut temps = None;
        let mut vars = None;
        let mut inside_var_section = VarSection::empty();
        let mut method_ranges: Vec<Span> = Vec::new();
        let mut body = None;

        while let Some((m, capture_index)) = captures.next() {
            let capture = m.captures[*capture_index];
            let is_inside =
                offset >= capture.node.start_byte() && offset <= capture.node.end_byte();

            match FOLD_QUERY.capture_names()[capture.index as usize] {
                "input_decls" => {
                    if is_inside {
                        inside_var_section |= VarSection::INPUTS;
                    }
                    inputs = Some(capture.node.range().into());
                }
                "output_decls" => {
                    if is_inside {
                        inside_var_section |= VarSection::OUTPUTS;
                    }
                    outputs = Some(capture.node.range().into());
                }
                "in_out_decls" => {
                    if is_inside {
                        inside_var_section |= VarSection::IN_OUTS;
                    }
                    in_outs = Some(capture.node.range().into());
                }
                "temp_var_decls" => {
                    if is_inside {
                        inside_var_section |= VarSection::TEMPS;
                    }
                    temps = Some(capture.node.range().into());
                }
                "var_decls" => {
                    if is_inside {
                        inside_var_section |= VarSection::VARS;
                    }
                    vars = Some(capture.node.range().into());
                }
                "method" => {
                    method_ranges.push(capture.node.range().into());
                }
                "body" => {
                    body = Some(capture.node);
                }
                _ => {}
            }
        }

        let inside_head = Self::determine_head_location(
            offset,
            &inside_var_section,
            &[inputs, outputs, in_outs, temps, vars],
            &method_ranges,
            body,
            source,
        );

        HeadResult {
            inputs,
            outputs,
            in_outs,
            temps,
            vars,
            inside_var_section,
            head_location: inside_head,
        }
    }

    fn determine_head_location(
        offset: usize,
        inside_var_section: &VarSection,
        var_ranges: &[Option<Span>],
        method_ranges: &[Span],
        body: Option<tree_sitter::Node>,
        source: &str,
    ) -> HeadLocation {
        // Find the first and last var section positions
        let first_var_start = var_ranges
            .iter()
            .filter_map(|&r| r.map(|range| range.start_byte))
            .min();

        let last_var_end = var_ranges
            .iter()
            .filter_map(|&r| r.map(|range| range.end_byte))
            .max();

        // Find the first and last method positions
        let first_method_start = method_ranges.iter().map(|range| range.start_byte).min();

        let last_method_end = method_ranges.iter().map(|range| range.end_byte).max();

        match (
            first_var_start,
            last_var_end,
            first_method_start,
            last_method_end,
        ) {
            // No vars and no methods — POU is empty or has only body statements.
            (None, None, None, None) => {
                if let Some(body_node) = body
                    && offset >= body_node.start_byte() && offset <= body_node.end_byte() {
                        let body_text = &source[body_node.start_byte()..body_node.end_byte()];
                        let first_char = body_text.trim_start().chars().next();
                        if matches!(first_char, Some('V') | Some('E') | Some('M')) {
                            return HeadLocation::InBodyAfterVars;
                        }
                        return HeadLocation::InBody;
                    }
                // No vars, no methods, no body (or cursor outside body) — the POU
                // is effectively empty.  Offer both VAR snippets and body completions.
                HeadLocation::InBodyAfterVars
            }

            // No vars but methods exist
            (None, None, Some(method_start), Some(method_end)) => {
                if offset < method_start {
                    HeadLocation::BeforeVars
                } else if offset <= method_end {
                    HeadLocation::InMethods
                } else {
                    // Check if body is empty or starts with head keywords (V, E, M)
                    if let Some(body_node) = body {
                        let body_text = &source[body_node.start_byte()..body_node.end_byte()];
                        let first_char = body_text.trim_start().chars().next();
                        if matches!(first_char, Some('V') | Some('E') | Some('M')) {
                            return HeadLocation::InBodyAfterMethods;
                        }
                    }
                    HeadLocation::InBody
                }
            }

            // Vars exist but no methods
            (Some(var_start), Some(var_end), None, None) => {
                if offset < var_start {
                    HeadLocation::BeforeVars
                } else if offset <= var_end && inside_var_section.is_empty() {
                    // Between var sections (not inside any specific one)
                    HeadLocation::InVars
                } else if offset > var_end {
                    // Check if body is empty or starts with head keywords (V, E, M)
                    if let Some(body_node) = body {
                        let body_text = &source[body_node.start_byte()..body_node.end_byte()];
                        let first_char = body_text.trim_start().chars().next();
                        if matches!(first_char, Some('V') | Some('E') | Some('M')) {
                            return HeadLocation::InBodyAfterVars;
                        }
                    }
                    HeadLocation::InBody
                } else {
                    // Inside a var section content - before the first var or in undefined space
                    HeadLocation::BeforeVars
                }
            }

            // Both vars and methods exist
            (Some(var_start), Some(var_end), Some(method_start), Some(method_end)) => {
                if offset < var_start {
                    HeadLocation::BeforeVars
                } else if offset <= var_end && inside_var_section.is_empty() {
                    // Between var sections (not inside any specific one)
                    HeadLocation::InVars
                } else if offset < method_start {
                    HeadLocation::BeforeMethods
                } else if offset <= method_end {
                    HeadLocation::InMethods
                } else {
                    // Check if body is empty or starts with head keywords (V, E, M)
                    if let Some(body_node) = body {
                        let body_text = &source[body_node.start_byte()..body_node.end_byte()];
                        let first_char = body_text.trim_start().chars().next();
                        if matches!(first_char, Some('V') | Some('E') | Some('M')) {
                            return HeadLocation::InBodyAfterMethods;
                        }
                    }
                    // After all methods, in the main body
                    HeadLocation::InBody
                }
            }

            // Handle any remaining edge cases
            _ => HeadLocation::BeforeVars,
        }
    }
}

#[cfg(test)]
mod test {
    use auto_lsp::tree_sitter;

    use super::*;

    #[test]
    fn test_var_decls_query() {
        let source = r#"
FUNCTION_BLOCK FB1
VAR_INPUT
    in1 : INT;
    in2 : BOOL;
END_VAR

VAR_OUTPUT
    out1 : INT;
END_VAR

VAR_IN_OUT
    inout1 : INT;
END_VAR
END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();

        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let root_node = tree.root_node();

        // Test offset inside VAR_INPUT
        let offset = source.find("in1").unwrap();
        let results =
            HeadResult::query_var_decls(root_node, source, root_node.range().into(), offset);
        assert_eq!(results.inputs.is_some(), true);
        assert_eq!(results.outputs.is_some(), true);
        assert_eq!(results.in_outs.is_some(), true);
        assert_eq!(results.inside_var_section, VarSection::INPUTS);
        assert_eq!(
            results.active_variable_sections(),
            VarSection::INPUTS | VarSection::OUTPUTS | VarSection::IN_OUTS
        );
        assert_eq!(results.inactive_variable_sections(), VarSection::empty());
    }

    #[test]
    fn test_var_decls_query_with_range() {
        let source = r#"
FUNCTION_BLOCK FB1
VAR_INPUT
    in1 : INT;
    in2 : BOOL;
END_VAR

VAR_OUTPUT
    out1 : INT;
END_VAR

END_FUNCTION_BLOCK

FUNCTION_BLOCK FB2

VAR_IN_OUT
    inout1 : INT;
END_VAR

END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();

        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let root_node = tree.root_node();

        // Test offset inside VAR_INPUT
        let offset = source.find("inout1").unwrap();
        let results = HeadResult::query_var_decls(
            root_node,
            source,
            Span::from(tree_sitter::Range {
                start_byte: source.find("FUNCTION_BLOCK FB2").unwrap(),
                end_byte: source.len(),
                start_point: tree_sitter::Point { row: 13, column: 0 },
                end_point: tree_sitter::Point { row: 20, column: 0 },
            }),
            offset,
        );
        assert_eq!(results.inputs.is_none(), true);
        assert_eq!(results.outputs.is_none(), true);
        assert_eq!(results.in_outs.is_some(), true);
        assert_eq!(results.inside_var_section, VarSection::IN_OUTS);
        assert_eq!(results.active_variable_sections(), VarSection::IN_OUTS);
        assert_eq!(
            results.inactive_variable_sections(),
            VarSection::INPUTS | VarSection::OUTPUTS
        );
    }

    #[test]
    fn test_head_location_before_vars() {
        let source = r#"
FUNCTION_BLOCK FB1

VAR_INPUT
    in1 : INT;
END_VAR
END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();

        // Cursor right after FUNCTION_BLOCK declaration
        let offset = source.find("FUNCTION_BLOCK FB1").unwrap() + "FUNCTION_BLOCK FB1".len();
        let results =
            HeadResult::query_var_decls(root_node, source, root_node.range().into(), offset);

        assert_eq!(results.head_location, HeadLocation::BeforeVars);
    }

    #[test]
    fn test_head_location_in_vars() {
        let source = r#"
FUNCTION_BLOCK FB1
VAR_INPUT
    in1 : INT;
END_VAR

VAR_OUTPUT
    out1 : INT;
END_VAR
END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();

        // Cursor between VAR_INPUT and VAR_OUTPUT (after first END_VAR)
        let first_end_var = source.find("END_VAR").unwrap() + "END_VAR".len();
        let offset = first_end_var + 1;
        let results =
            HeadResult::query_var_decls(root_node, source, root_node.range().into(), offset);

        assert_eq!(results.head_location, HeadLocation::InVars);
        assert_eq!(results.inside_var_section, VarSection::empty());
    }

    #[test]
    fn test_head_location_before_methods() {
        let source = r#"
FUNCTION_BLOCK FB1
VAR_INPUT
    in1 : INT;
END_VAR

METHOD MyMethod
END_METHOD
END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();

        // Cursor between END_VAR and METHOD (at the blank line)
        let end_var_pos = source.find("END_VAR").unwrap() + "END_VAR".len();
        let offset = end_var_pos + 1; // Position at the first newline after END_VAR

        let results =
            HeadResult::query_var_decls(root_node, source, root_node.range().into(), offset);

        assert_eq!(results.head_location, HeadLocation::BeforeMethods);
    }

    #[test]
    fn test_head_location_in_methods() {
        let source = r#"
FUNCTION_BLOCK FB1
VAR_INPUT
    in1 : INT;
END_VAR

METHOD MyMethod
    // method body
END_METHOD
END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();

        // Cursor inside method body
        let offset = source.find("// method body").unwrap();
        let results =
            HeadResult::query_var_decls(root_node, source, root_node.range().into(), offset);

        assert_eq!(results.head_location, HeadLocation::InMethods);
    }

    #[test]
    fn test_head_location_in_stmts() {
        let source = r#"
FUNCTION_BLOCK FB1
VAR_INPUT
    in1 : INT;
END_VAR

METHOD MyMethod
END_METHOD

    // statement area
END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();

        // Cursor after methods
        let offset = source.find("// statement area").unwrap();
        let results =
            HeadResult::query_var_decls(root_node, source, root_node.range().into(), offset);

        assert_eq!(results.head_location, HeadLocation::InBody);
    }

    #[test]
    fn test_head_location_empty_pou_is_in_body() {
        let source = r#"
FUNCTION_BLOCK FB1
END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();

        // Cursor in empty function block — no vars, no methods, no body.
        // Should be InBodyAfterVars so both VAR snippets and body completions are offered.
        let offset = source.find("FUNCTION_BLOCK FB1").unwrap() + "FUNCTION_BLOCK FB1".len();
        let results =
            HeadResult::query_var_decls(root_node, source, root_node.range().into(), offset);

        assert_eq!(results.head_location, HeadLocation::InBodyAfterVars);
    }

    #[test]
    fn test_head_location_between_methods() {
        let source = r#"
FUNCTION_BLOCK FB1
VAR_INPUT
    in1 : INT;
END_VAR

METHOD Method1
    // first method
END_METHOD

METHOD Method2
    // second method
END_METHOD
END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();

        // Cursor between two method declarations
        let first_end = source.find("END_METHOD").unwrap() + "END_METHOD".len();
        let offset = first_end + 2; // Position between the two methods

        let results =
            HeadResult::query_var_decls(root_node, source, root_node.range().into(), offset);

        // Between methods should be InMethods (to allow adding more methods)
        assert_eq!(results.head_location, HeadLocation::InMethods);
    }

    #[test]
    fn test_head_location_in_body_after_vars_with_v_char() {
        let source = r#"
FUNCTION_BLOCK FB1
VAR_INPUT
    in1 : INT;
END_VAR
V
END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();

        // Cursor inside the body which starts with 'V'
        let offset = source.find("V\n").unwrap();
        let results =
            HeadResult::query_var_decls(root_node, source, root_node.range().into(), offset);

        // Body starts with 'V', so it should be InBodyAfterVars
        assert_eq!(results.head_location, HeadLocation::InBodyAfterVars);
    }

    #[test]
    fn test_head_location_in_body_after_vars_with_m_char() {
        let source = r#"
FUNCTION_BLOCK FB1
VAR_INPUT
    in1 : INT;
END_VAR
M
END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();

        // Cursor at the start, body begins with 'M'
        let offset = source.find("M\n").unwrap();
        let results =
            HeadResult::query_var_decls(root_node, source, root_node.range().into(), offset);

        // Body starts with 'M', so it should be InBodyAfterVars
        assert_eq!(results.head_location, HeadLocation::InBodyAfterVars);
    }

    #[test]
    fn test_head_location_in_body_after_methods_with_v_char() {
        let source = r#"
FUNCTION_BLOCK FB1
METHOD MyMethod
END_METHOD
V
END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();

        // Cursor at body which starts with 'V' after method
        let offset = source.find("V\n").unwrap();
        let results =
            HeadResult::query_var_decls(root_node, source, root_node.range().into(), offset);

        // Body starts with 'V' after methods, so it should be InBodyAfterMethods
        assert_eq!(results.head_location, HeadLocation::InBodyAfterMethods);
    }

    #[test]
    fn test_head_location_in_body_after_methods_regular_statement() {
        let source = r#"
FUNCTION_BLOCK FB1
METHOD MyMethod
END_METHOD
x := 5;
END_FUNCTION_BLOCK
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rk::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();

        // Cursor at regular statement after method
        let offset = source.find("x := 5").unwrap();
        let results =
            HeadResult::query_var_decls(root_node, source, root_node.range().into(), offset);

        // Body does NOT start with V/E/M, so it should be regular InBody
        assert_eq!(results.head_location, HeadLocation::InBody);
    }
}
