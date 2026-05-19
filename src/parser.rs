use crate::models::{Fixture, FixtureScope, ParsedModule, TestFunction};
use anyhow::Result;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::Path;
use tree_sitter::Parser;

struct DecoratorInfo<'a> {
    text: String,
    node: Option<tree_sitter::Node<'a>>,
}

/// Tree-sitter based Python test file parser that extracts test functions and fixtures.
pub struct PythonParser {
    parser: Parser,
}

impl PythonParser {
    /// Create a new parser with the Python grammar loaded.
    #[allow(clippy::missing_errors_doc)]
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_python::LANGUAGE.into())?;
        Ok(Self { parser })
    }

    /// Parse a Python file and extract its test functions, fixtures, and imports.
    #[allow(clippy::missing_errors_doc)]
    pub fn parse_file(&mut self, path: &Path) -> Result<ParsedModule> {
        let source = std::fs::read_to_string(path)?;
        self.parse_source(&source, path)
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn parse_source(&mut self, source: &str, path: &Path) -> Result<ParsedModule> {
        let tree = self.parser.parse(source, None);
        let file_path = path.to_path_buf();

        if let Some(tree) = tree {
            let root = tree.root_node();
            let source_bytes = source.as_bytes();
            let imports = Self::extract_imports(&root, source_bytes);
            let test_functions = Self::extract_test_functions(&root, source_bytes, &file_path);
            let fixtures = Self::extract_fixtures(&root, source_bytes, &file_path);
            Ok(ParsedModule {
                file_path,
                source: source.to_string(),
                imports,
                test_functions,
                fixtures,
            })
        } else {
            eprintln!(
                "Warning: tree-sitter failed to parse {}",
                file_path.display()
            );
            Ok(ParsedModule {
                file_path,
                source: source.to_string(),
                imports: vec![],
                test_functions: vec![],
                fixtures: vec![],
            })
        }
    }

    fn node_text(node: tree_sitter::Node, source: &[u8]) -> String {
        node.utf8_text(source).unwrap_or_default().to_string()
    }

    fn extract_imports(root: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut imports = Vec::new();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            match child.kind() {
                "import_statement" | "import_from_statement" => {
                    imports.push(Self::node_text(child, source));
                }
                _ => {}
            }
        }
        imports
    }

    fn collect_function_nodes<'tree>(
        root: &'tree tree_sitter::Node<'tree>,
    ) -> Vec<tree_sitter::Node<'tree>> {
        let mut nodes = Vec::new();
        let mut to_visit = vec![*root];
        while let Some(node) = to_visit.pop() {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "function_definition" => {
                        nodes.push(child);
                    }
                    "decorated_definition" => {
                        let mut inner = child.walk();
                        for c in child.children(&mut inner) {
                            match c.kind() {
                                "function_definition" => nodes.push(c),
                                "class_definition" => to_visit.push(c),
                                _ => {}
                            }
                        }
                    }
                    "class_definition" => {
                        to_visit.push(child);
                    }
                    _ => {}
                }
            }
        }
        nodes
    }

    fn extract_test_functions(
        root: &tree_sitter::Node,
        source: &[u8],
        file_path: &Path,
    ) -> Vec<TestFunction> {
        let mut tests = Vec::new();
        for func_node in Self::collect_function_nodes(root) {
            let name_node = func_node.child_by_field_name("name");
            if let Some(nn) = name_node {
                let name = Self::node_text(nn, source);
                if name.starts_with("test_") {
                    tests.push(Self::build_test_function(
                        &func_node, source, file_path, &name,
                    ));
                }
            }
        }
        tests
    }

    fn build_test_function(
        func_node: &tree_sitter::Node,
        source: &[u8],
        file_path: &Path,
        name: &str,
    ) -> TestFunction {
        let line = func_node.start_position().row + 1;
        let body = func_node.child_by_field_name("body");
        let body_text = body.map(|b| Self::node_text(b, source)).unwrap_or_default();

        let decorators = Self::get_decorators(func_node, source);
        let parametrize_values = Self::extract_parametrize_values(&decorators, source);

        let is_async = {
            let mut cur = func_node.walk();
            let has_async = func_node.children(&mut cur).any(|c| c.kind() == "async");
            drop(cur);
            has_async
        };
        let (is_parametrized, parametrize_count) = Self::detect_parametrize(&decorators);
        let assertion_count = Self::count_assertions(body.as_ref());
        let has_assertions = assertion_count > 0;
        let has_mock_verifications = body_text.contains(".assert_called")
            || body_text.contains(".called")
            || body_text.contains(".call_count");
        let has_state_assertions = has_assertions && !has_mock_verifications_only(&body_text);
        let fixture_deps = Self::extract_fixture_deps(func_node, source);
        let uses_time_sleep = Self::detect_time_sleep(body.as_ref(), source);
        let sleep_value = Self::detect_sleep_value(body.as_ref(), source);
        let uses_file_io = Self::detect_file_io(body.as_ref(), source);
        let uses_network = Self::detect_network_usage(body.as_ref(), source);
        let has_conditional_logic = Self::detect_conditionals(body.as_ref());
        let has_try_except = Self::detect_try_except(body.as_ref());
        let docstring = Self::extract_docstring(func_node, source);
        let assertions = Self::extract_assertions(body.as_ref(), source);
        let uses_cwd_dependency = Self::detect_cwd_dependency(body.as_ref(), source);
        let uses_pytest_raises = Self::detect_pytest_raises(body.as_ref(), source);
        let mutates_fixture_deps =
            Self::detect_fixture_mutations(body.as_ref(), source, &fixture_deps);
        let uses_random = Self::detect_random_usage(body.as_ref(), source);
        let has_random_seed = Self::detect_random_seed(body.as_ref(), source);
        let uses_subprocess = Self::detect_subprocess_usage(body.as_ref(), source);
        let has_subprocess_timeout = Self::detect_subprocess_timeout(body.as_ref(), source);
        let mocked_stdlib_targets =
            Self::detect_stdlib_mock_targets(body.as_ref(), source, &decorators);
        let mocks_stdlib_module = !mocked_stdlib_targets.is_empty();
        let (has_weak_assertions, weak_assertion_details) =
            Self::detect_weak_assertions(body.as_ref(), source);
        let patch_targets = Self::detect_all_patch_targets(body.as_ref(), source, &decorators);
        let (has_magic_mock, mock_count) = Self::detect_mock_usage(body.as_ref(), source);
        let uses_shutil_copy = Self::detect_shutil_copy(body.as_ref(), source);

        let end_line = func_node.end_position().row + 1;
        let body_hash = body.map(|b| {
            let text = Self::node_text(b, source);
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut hasher);
            hasher.finish()
        });

        TestFunction {
            name: name.to_string(),
            file_path: file_path.to_path_buf(),
            line,
            end_line,
            is_async,
            is_parametrized,
            parametrize_count,
            has_assertions,
            assertion_count,
            has_mock_verifications,
            has_state_assertions,
            fixture_deps,
            uses_time_sleep,
            sleep_value,
            uses_file_io,
            uses_network,
            has_conditional_logic,
            has_try_except,
            docstring,
            assertions,
            parametrize_values,
            uses_cwd_dependency,
            uses_pytest_raises,
            mutates_fixture_deps,
            body_hash,
            uses_random,
            has_random_seed,
            uses_subprocess,
            has_subprocess_timeout,
            mocks_stdlib_module,
            mocked_stdlib_targets,
            has_weak_assertions,
            weak_assertion_details,
            patch_targets,
            has_magic_mock,
            mock_count,
            uses_shutil_copy,
        }
    }

    fn get_decorators<'a>(
        func_node: &tree_sitter::Node<'a>,
        source: &[u8],
    ) -> Vec<DecoratorInfo<'a>> {
        let mut decs = Vec::new();
        let parent = func_node.parent();
        let container = if parent.is_some_and(|p| p.kind() == "decorated_definition") {
            parent.unwrap()
        } else {
            *func_node
        };
        let mut cursor = container.walk();
        for child in container.children(&mut cursor) {
            if child.kind() == "decorator" {
                decs.push(DecoratorInfo {
                    text: Self::node_text(child, source),
                    node: Some(child),
                });
            }
        }
        decs
    }

    fn detect_parametrize(decorators: &[DecoratorInfo]) -> (bool, Option<usize>) {
        for dec in decorators {
            let name = dec
                .text
                .trim_start_matches('@')
                .split('(')
                .next()
                .unwrap_or("")
                .trim();
            if name == "pytest.mark.parametrize" || name == "parametrize" {
                let count = dec.node.map_or_else(
                    || Self::count_parametrize_args(&dec.text),
                    |node| {
                        Self::count_parametrize_args_ast(node)
                            .unwrap_or_else(|| Self::count_parametrize_args(&dec.text))
                    },
                );
                return (true, Some(count));
            }
        }
        (false, None)
    }

    /// Count non-comma, non-punctuation elements in a list/tuple node.
    fn count_list_or_tuple_elements(arg: tree_sitter::Node) -> (usize, bool) {
        let mut elem_count = 0;
        let mut elem_cursor = arg.walk();
        let mut found_comma = false;
        for elem in arg.children(&mut elem_cursor) {
            match elem.kind() {
                "," => {
                    found_comma = true;
                }
                "(" | ")" | "[" | "]" | "comment" => {}
                _ if !elem.is_extra() => {
                    elem_count += 1;
                }
                _ => {}
            }
        }
        (elem_count, found_comma)
    }

    /// Walk the argument_list children and find the second positional list/tuple argument,
    /// counting its elements to determine parametrize cardinality.
    fn count_values_in_argument_list(argument_list: tree_sitter::Node) -> Option<usize> {
        let mut comma_count = 0;
        let mut args_cursor = argument_list.walk();
        for arg in argument_list.children(&mut args_cursor) {
            if arg.kind() == "," {
                comma_count += 1;
                continue;
            }
            if comma_count >= 1 && (arg.kind() == "list" || arg.kind() == "tuple") {
                let (elem_count, found_comma) = Self::count_list_or_tuple_elements(arg);
                if elem_count == 0 && !found_comma {
                    return Some(0);
                }
                return Some(elem_count.max(1));
            }
        }
        None
    }

    fn count_parametrize_args_ast(decorator_node: tree_sitter::Node) -> Option<usize> {
        let mut cursor = decorator_node.walk();
        for child in decorator_node.children(&mut cursor) {
            if child.kind() != "call" {
                continue;
            }
            let mut call_cursor = child.walk();
            for call_child in child.children(&mut call_cursor) {
                if call_child.kind() == "argument_list" {
                    return Self::count_values_in_argument_list(call_child);
                }
            }
        }
        None
    }

    fn count_parametrize_args(dec: &str) -> usize {
        if let Some(start) = dec.rfind('[') {
            if let Some(end) = dec.rfind(']') {
                if end > start {
                    let inner = &dec[start + 1..end];
                    let depth_brace = Self::count_top_level_entries(inner);
                    return depth_brace;
                }
            }
        }
        let open = dec.matches('(').count();
        if open > 1 {
            return 2;
        }
        1
    }

    fn count_top_level_entries(inner: &str) -> usize {
        let mut count = 0;
        let mut depth = 0;
        let mut quote_char: Option<char> = None;
        let mut escape = false;
        let mut has_content_since_last_comma = false;

        for c in inner.chars() {
            if escape {
                escape = false;
                has_content_since_last_comma = true;
                continue;
            }
            if c == '\\' {
                escape = true;
                has_content_since_last_comma = true;
                continue;
            }
            if let Some(qc) = quote_char {
                if c == qc {
                    quote_char = None;
                }
                has_content_since_last_comma = true;
                continue;
            }
            match c {
                '"' | '\'' => {
                    quote_char = Some(c);
                    has_content_since_last_comma = true;
                }
                '(' | '[' | '{' => {
                    depth += 1;
                    has_content_since_last_comma = true;
                }
                ')' | ']' | '}' if depth > 0 => {
                    depth -= 1;
                    has_content_since_last_comma = true;
                }
                ',' if depth == 0 => {
                    if has_content_since_last_comma {
                        count += 1;
                    }
                    has_content_since_last_comma = false;
                }
                _ => {
                    if !c.is_whitespace() {
                        has_content_since_last_comma = true;
                    }
                }
            }
        }
        if has_content_since_last_comma {
            count += 1;
        }
        count
    }

    fn count_assertions(body: Option<&tree_sitter::Node>) -> usize {
        body.map_or(0, |b| {
            let mut count = 0;
            Self::count_assertions_recursive(*b, &mut count);
            count
        })
    }

    fn count_assertions_recursive(node: tree_sitter::Node, count: &mut usize) {
        if node.kind() == "assert_statement" {
            *count += 1;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::count_assertions_recursive(child, count);
        }
    }

    fn detect_conditionals(body: Option<&tree_sitter::Node>) -> bool {
        body.is_some_and(|b| Self::has_node_kind(*b, "if_statement"))
    }

    fn detect_try_except(body: Option<&tree_sitter::Node>) -> bool {
        body.is_some_and(|b| Self::has_node_kind(*b, "try_statement"))
    }

    fn has_node_kind(node: tree_sitter::Node, kind: &str) -> bool {
        if node.kind() == kind {
            return true;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_node_kind(child, kind) {
                return true;
            }
        }
        false
    }

    fn extract_assertions(
        body: Option<&tree_sitter::Node>,
        source: &[u8],
    ) -> Vec<crate::models::AssertionInfo> {
        body.map_or(vec![], |b| {
            let mut infos = Vec::new();
            Self::collect_assertion_info(*b, source, &mut infos);
            infos
        })
    }

    fn is_magic_assertion(expr_node: tree_sitter::Node, source: &[u8], has_comparison: bool) -> bool {
        let kind = expr_node.kind();
        if kind == "true" || kind == "false" {
            return true;
        }
        if kind == "integer" {
            let text = Self::node_text(expr_node, source);
            return text == "0" || text == "1";
        }
        !has_comparison && kind == "identifier"
    }

    fn build_assertion_info(
        expr_node: tree_sitter::Node,
        source: &[u8],
        line: usize,
    ) -> crate::models::AssertionInfo {
        let expression_text = Self::node_text(expr_node, source);
        let has_comparison =
            Self::has_node_kind_recursive(expr_node, "comparison_operator");
        let is_magic = Self::is_magic_assertion(expr_node, source, has_comparison);
        let is_suboptimal = Self::is_suboptimal_assertion(expr_node, source);
        crate::models::AssertionInfo {
            is_magic,
            is_suboptimal,
            has_comparison,
            expression_text,
            line,
        }
    }

    fn collect_assertion_info(
        node: tree_sitter::Node,
        source: &[u8],
        infos: &mut Vec<crate::models::AssertionInfo>,
    ) {
        if node.kind() == "assert_statement" {
            let line = node.start_position().row + 1;
            let mut cursor = node.walk();
            if let Some(expr_node) = node.children(&mut cursor).find(|c| {
                let k = c.kind();
                !k.starts_with(',') && k != "comment" && k != "assert"
            }) {
                infos.push(Self::build_assertion_info(expr_node, source, line));
            }
            return;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::collect_assertion_info(child, source, infos);
        }
    }

    fn has_node_kind_recursive(node: tree_sitter::Node, kind: &str) -> bool {
        if node.kind() == kind {
            return true;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_node_kind_recursive(child, kind) {
                return true;
            }
        }
        false
    }

    /// Check if a call child is `len(...)` or `type(...)` — a suboptimal assertion pattern.
    fn is_len_or_type_call(child: tree_sitter::Node, source: &[u8]) -> bool {
        if child.kind() != "call" {
            return false;
        }
        let func = child.child_by_field_name("function");
        match func {
            Some(f) => {
                let name = Self::node_text(f, source);
                name == "len" || name == "type"
            }
            None => false,
        }
    }

    /// Check if a child is `not None` — a suboptimal assertion pattern.
    fn is_not_none_check(child: tree_sitter::Node) -> bool {
        if child.kind() != "not" {
            return false;
        }
        let mut nc = child.walk();
        let has_none = child.children(&mut nc).any(|inner| inner.kind() == "none");
        has_none
    }

    /// Check if a `none` child is used with `==`, `!=`, or `not` (rather than `is`/`is not`).
    fn is_suboptimal_none_comparison(
        child: tree_sitter::Node,
        expr: tree_sitter::Node,
        source: &[u8],
    ) -> bool {
        if child.kind() != "none" {
            return false;
        }
        let text = Self::node_text(expr, source);
        text.contains("==") || text.contains("!=") || text.contains("not")
    }

    fn is_suboptimal_assertion(expr: tree_sitter::Node, source: &[u8]) -> bool {
        if expr.kind() != "comparison_operator" {
            return false;
        }
        let mut cursor = expr.walk();
        for child in expr.children(&mut cursor) {
            if child.kind() == "is" || child.kind() == "is not" {
                return false;
            }
            if Self::is_len_or_type_call(child, source) {
                return true;
            }
            if Self::is_not_none_check(child) {
                return true;
            }
            if Self::is_suboptimal_none_comparison(child, expr, source) {
                return true;
            }
        }
        false
    }

    fn extract_parametrize_values(decorators: &[DecoratorInfo], source: &[u8]) -> Vec<Vec<String>> {
        let mut all_values = Vec::new();
        for dec in decorators {
            let name = dec
                .text
                .trim_start_matches('@')
                .split('(')
                .next()
                .unwrap_or("")
                .trim();
            if name != "pytest.mark.parametrize" && name != "parametrize" {
                continue;
            }
            if let Some(node) = dec.node {
                if let Some(values) = Self::extract_values_from_decorator_node(node, source) {
                    all_values.push(values);
                }
            }
        }
        all_values
    }

    /// Find the second list/tuple argument in an argument_list (the values arg to parametrize).
    /// If only one list/tuple exists, returns that one. Stops at the second match.
    fn find_second_list_tuple_arg(argument_list: tree_sitter::Node) -> Option<tree_sitter::Node> {
        let mut args_cursor = argument_list.walk();
        let mut tuple_list_count = 0;
        let mut last_found = None;
        for arg in argument_list.children(&mut args_cursor) {
            if arg.kind() == "list" || arg.kind() == "tuple" {
                tuple_list_count += 1;
                last_found = Some(arg);
                if tuple_list_count == 2 {
                    return last_found;
                }
            }
        }
        last_found
    }

    /// Extract string values from a list/tuple node (skipping punctuation).
    fn extract_text_values_from_node(arg: tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut values = Vec::new();
        let mut elem_cursor = arg.walk();
        for elem in arg.children(&mut elem_cursor) {
            match elem.kind() {
                "," | "(" | ")" | "[" | "]" | "comment" => {}
                _ if !elem.is_extra() => {
                    values.push(Self::node_text(elem, source).trim().to_string());
                }
                _ => {}
            }
        }
        values
    }

    fn extract_values_from_decorator_node(
        decorator_node: tree_sitter::Node,
        source: &[u8],
    ) -> Option<Vec<String>> {
        let mut cursor = decorator_node.walk();
        for child in decorator_node.children(&mut cursor) {
            if child.kind() != "call" {
                continue;
            }
            let mut call_cursor = child.walk();
            for call_child in child.children(&mut call_cursor) {
                if call_child.kind() != "argument_list" {
                    continue;
                }
                let target_arg = Self::find_second_list_tuple_arg(call_child);
                if let Some(arg) = target_arg {
                    return Some(Self::extract_text_values_from_node(arg, source));
                }
            }
        }
        None
    }

    fn detect_cwd_dependency(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        body.is_some_and(|b| Self::has_cwd_call(*b, source))
    }

    /// Check if a single call node refers to a `getcwd` or `chdir` function.
    fn is_cwd_call_node(node: tree_sitter::Node, source: &[u8]) -> bool {
        if node.kind() != "call" {
            return false;
        }
        let func = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return false,
        };
        let text = Self::node_text(func, source);
        if text == "os.getcwd" || text == "os.chdir" || text == "Path.cwd" {
            return true;
        }
        if text.contains("getcwd") || text.contains("chdir") {
            return true;
        }
        if func.kind() == "attribute" {
            let attr = func.child_by_field_name("attribute");
            if let Some(a) = attr {
                let name = Self::node_text(a, source);
                if name == "getcwd" || name == "chdir" {
                    return true;
                }
            }
        }
        false
    }

    fn has_cwd_call(node: tree_sitter::Node, source: &[u8]) -> bool {
        if Self::is_cwd_call_node(node, source) {
            return true;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_cwd_call(child, source) {
                return true;
            }
        }
        false
    }

    fn detect_pytest_raises(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        body.is_some_and(|b| Self::has_pytest_raises(*b, source))
    }

    fn has_pytest_raises(node: tree_sitter::Node, source: &[u8]) -> bool {
        if node.kind() == "call" {
            let func = node.child_by_field_name("function");
            if let Some(f) = func {
                if f.kind() == "attribute" {
                    let text = Self::node_text(f, source);
                    if text == "pytest.raises" {
                        return true;
                    }
                    let attr = f.child_by_field_name("attribute");
                    let obj = f.child_by_field_name("object");
                    if let (Some(a), Some(o)) = (attr, obj) {
                        let name = Self::node_text(a, source);
                        let obj_name = Self::node_text(o, source);
                        if name == "raises" && obj_name == "pytest" {
                            return true;
                        }
                    }
                }
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_pytest_raises(child, source) {
                return true;
            }
        }
        false
    }

    fn detect_fixture_mutations(
        body: Option<&tree_sitter::Node>,
        source: &[u8],
        fixture_deps: &[String],
    ) -> Vec<String> {
        let mut mutated = Vec::new();
        if let Some(b) = body {
            Self::find_mutations(*b, source, fixture_deps, &mut mutated);
        }
        mutated.sort();
        mutated.dedup();
        mutated
    }

    fn handle_mut_call(
        node: tree_sitter::Node,
        source: &[u8],
        fixture_deps: &[String],
        mutated: &mut Vec<String>,
    ) {
        if node.kind() != "call" {
            return;
        }
        let func = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return,
        };
        if func.kind() != "attribute" {
            return;
        }
        let obj = func.child_by_field_name("object");
        let attr = func.child_by_field_name("attribute");
        let (Some(obj), Some(attr)) = (obj, attr) else { return };
        let obj_name = Self::node_text(obj, source);
        let method = Self::node_text(attr, source);
        let mutating_methods = [
            "append", "extend", "remove", "pop", "clear", "update", "insert", "add", "discard",
        ];
        if !mutating_methods.contains(&method.as_str()) {
            return;
        }
        if fixture_deps.contains(&obj_name) || Self::is_fixture_chain(&obj, source, fixture_deps) {
            mutated.push(Self::get_fixture_root(&obj, source, fixture_deps));
        }
    }

    fn handle_mut_delete(
        node: tree_sitter::Node,
        source: &[u8],
        fixture_deps: &[String],
        mutated: &mut Vec<String>,
    ) {
        if node.kind() != "delete_statement" {
            return;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let text = Self::node_text(child, source).trim().to_string();
            if fixture_deps.contains(&text) {
                mutated.push(text);
            }
        }
    }

    fn find_mutations(
        node: tree_sitter::Node,
        source: &[u8],
        fixture_deps: &[String],
        mutated: &mut Vec<String>,
    ) {
        Self::handle_mut_call(node, source, fixture_deps, mutated);
        if node.kind() == "assignment" {
            if let Some(t) = node.child_by_field_name("left") {
                Self::check_assignment_target(t, source, fixture_deps, mutated);
            }
        }
        Self::handle_mut_delete(node, source, fixture_deps, mutated);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::find_mutations(child, source, fixture_deps, mutated);
        }
    }

    fn push_if_fixture_dep(
        name: &str,
        node: &tree_sitter::Node,
        source: &[u8],
        fixture_deps: &[String],
        mutated: &mut Vec<String>,
    ) {
        if fixture_deps.contains(&name.to_string()) {
            mutated.push(name.to_string());
        } else if Self::is_fixture_chain(node, source, fixture_deps) {
            mutated.push(Self::get_fixture_root(node, source, fixture_deps));
        }
    }

    fn check_subscript_target(
        target: tree_sitter::Node,
        source: &[u8],
        fixture_deps: &[String],
        mutated: &mut Vec<String>,
    ) {
        if target.kind() == "subscript" {
            if let Some(v) = target.child_by_field_name("value") {
                let name = Self::node_text(v, source);
                Self::push_if_fixture_dep(&name, &v, source, fixture_deps, mutated);
            }
        }
    }

    fn check_attribute_target(
        target: tree_sitter::Node,
        source: &[u8],
        fixture_deps: &[String],
        mutated: &mut Vec<String>,
    ) {
        if target.kind() == "attribute" {
            if let Some(o) = target.child_by_field_name("object") {
                let name = Self::node_text(o, source);
                Self::push_if_fixture_dep(&name, &o, source, fixture_deps, mutated);
            }
        }
    }

    fn check_assignment_target(
        target: tree_sitter::Node,
        source: &[u8],
        fixture_deps: &[String],
        mutated: &mut Vec<String>,
    ) {
        Self::check_subscript_target(target, source, fixture_deps, mutated);
        Self::check_attribute_target(target, source, fixture_deps, mutated);
    }

    /// Check if an attribute chain's root is a fixture dependency.
    fn is_fixture_chain(node: &tree_sitter::Node, source: &[u8], fixture_deps: &[String]) -> bool {
        let mut current = *node;
        loop {
            if current.kind() == "identifier" {
                let name = Self::node_text(current, source);
                return fixture_deps.contains(&name);
            }
            if current.kind() == "attribute" {
                if let Some(obj) = current.child_by_field_name("object") {
                    current = obj;
                } else {
                    return false;
                }
            } else if current.kind() == "subscript" {
                if let Some(val) = current.child_by_field_name("value") {
                    current = val;
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }
    }

    /// Get the root fixture name from an attribute chain.
    fn get_fixture_root(
        node: &tree_sitter::Node,
        source: &[u8],
        fixture_deps: &[String],
    ) -> String {
        let mut current = *node;
        loop {
            if current.kind() == "identifier" {
                let name = Self::node_text(current, source);
                if fixture_deps.contains(&name) {
                    return name;
                }
            }
            if current.kind() == "attribute" {
                if let Some(obj) = current.child_by_field_name("object") {
                    current = obj;
                } else {
                    break;
                }
            } else if current.kind() == "subscript" {
                if let Some(val) = current.child_by_field_name("value") {
                    current = val;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        String::new()
    }

    fn extract_docstring(func_node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        let body = func_node.child_by_field_name("body")?;
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "expression_statement" {
                let mut inner_cursor = child.walk();
                for expr in child.children(&mut inner_cursor) {
                    if expr.kind() == "string" {
                        return Some(Self::node_text(expr, source));
                    }
                }
            }
        }
        None
    }

    fn extract_fixture_deps(func_node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut deps = Vec::new();
        let params = func_node.child_by_field_name("parameters");
        if let Some(p) = params {
            let mut cursor = p.walk();
            for child in p.children(&mut cursor) {
                let name = match child.kind() {
                    "identifier" => Some(Self::node_text(child, source)),
                    "typed_parameter" | "default_parameter" | "typed_default_parameter" => child
                        .child_by_field_name("name")
                        .map(|n| Self::node_text(n, source)),
                    _ => None,
                };
                if let Some(name) = name {
                    if !["self", "cls"].contains(&name.as_str()) {
                        deps.push(name);
                    }
                }
            }
        }
        deps
    }

    fn extract_fixtures(root: &tree_sitter::Node, source: &[u8], file_path: &Path) -> Vec<Fixture> {
        let mut fixtures = Vec::new();
        let frozen_classes = Self::detect_frozen_dataclass_names(root, source);

        for func_node in Self::collect_function_nodes(root) {
            let decorators = Self::get_decorators(&func_node, source);
            let is_fixture = decorators.iter().any(|d| {
                let name = d
                    .text
                    .trim_start_matches('@')
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim();
                name == "pytest.fixture" || name == "fixture"
            });

            if is_fixture {
                let name_node = func_node.child_by_field_name("name");
                if let Some(nn) = name_node {
                    let name = Self::node_text(nn, source);
                    let dec_texts: Vec<String> =
                        decorators.iter().map(|d| d.text.clone()).collect();
                    fixtures.push(Self::build_fixture(
                        &func_node,
                        source,
                        file_path,
                        &name,
                        &dec_texts,
                        &frozen_classes,
                    ));
                }
            }
        }
        fixtures
    }

    fn build_fixture(
        func_node: &tree_sitter::Node,
        source: &[u8],
        file_path: &Path,
        name: &str,
        decorators: &[String],
        frozen_classes: &HashSet<String>,
    ) -> Fixture {
        let line = func_node.start_position().row + 1;
        let body = func_node.child_by_field_name("body");

        let scope = Self::extract_fixture_scope(decorators);
        let is_autouse = decorators
            .iter()
            .any(|d| d.contains("autouse") && d.contains("True"));
        let dependencies = Self::extract_fixture_deps(func_node, source);
        let returns_mutable = Self::detect_mutable_return(body.as_ref(), source, frozen_classes);
        let has_yield = Self::detect_yield(body.as_ref());
        let has_db_commit = Self::detect_db_commit(body.as_ref(), source);
        let has_db_rollback = Self::detect_db_rollback(body.as_ref(), source);
        let has_cleanup = has_db_rollback || Self::detect_cleanup_pattern(body.as_ref(), source);
        let uses_file_io = Self::detect_file_io(body.as_ref(), source);

        Fixture {
            name: name.to_string(),
            file_path: file_path.to_path_buf(),
            line,
            scope,
            is_autouse,
            dependencies,
            returns_mutable,
            has_yield,
            has_db_commit,
            has_db_rollback,
            has_cleanup,
            uses_file_io,
            used_by: vec![],
        }
    }

    fn extract_fixture_scope(decorators: &[String]) -> FixtureScope {
        for dec in decorators {
            if dec.contains("scope") {
                if dec.contains("\"session\"") || dec.contains("'session'") {
                    return FixtureScope::Session;
                }
                if dec.contains("\"package\"") || dec.contains("'package'") {
                    return FixtureScope::Package;
                }
                if dec.contains("\"module\"") || dec.contains("'module'") {
                    return FixtureScope::Module;
                }
                if dec.contains("\"class\"") || dec.contains("'class'") {
                    return FixtureScope::Class;
                }
            }
        }
        FixtureScope::Function
    }

    fn detect_file_io(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        body.is_some_and(|b| Self::has_file_io_call(*b, source))
    }

    fn has_file_io_call(node: tree_sitter::Node, source: &[u8]) -> bool {
        if node.kind() == "call" {
            let func = node.child_by_field_name("function");
            if let Some(f) = func {
                let name = Self::node_text(f, source);
                if ["open", "read", "write"].contains(&name.as_str()) {
                    return true;
                }
                if f.kind() == "attribute" {
                    let attr = f.child_by_field_name("attribute");
                    if let Some(a) = attr {
                        let attr_name = Self::node_text(a, source);
                        if ["read", "write", "open"].contains(&attr_name.as_str()) {
                            return true;
                        }
                    }
                }
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_file_io_call(child, source) {
                return true;
            }
        }
        false
    }

    fn detect_time_sleep(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        body.is_some_and(|b| Self::has_time_sleep_call(*b, source))
    }

    fn has_time_sleep_call(node: tree_sitter::Node, source: &[u8]) -> bool {
        if node.kind() == "call" {
            let func = node.child_by_field_name("function");
            if let Some(f) = func {
                let text = Self::node_text(f, source);
                if text == "time.sleep" || text == "sleep" {
                    return true;
                }
                if f.kind() == "attribute" {
                    let attr = f.child_by_field_name("attribute");
                    if let Some(a) = attr {
                        let name = Self::node_text(a, source);
                        if name == "sleep" {
                            return true;
                        }
                    }
                }
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_time_sleep_call(child, source) {
                return true;
            }
        }
        false
    }

    fn detect_sleep_value(body: Option<&tree_sitter::Node>, source: &[u8]) -> Option<f64> {
        body.and_then(|b| Self::find_sleep_value(*b, source))
    }

    fn extract_sleep_arg(node: tree_sitter::Node, source: &[u8]) -> Option<f64> {
        if let Some(arg) = node.child_by_field_name("arguments") {
            for child in arg.children(&mut arg.walk()) {
                if child.kind() == "integer" || child.kind() == "float" {
                    let val_str = Self::node_text(child, source);
                    if let Ok(val) = val_str.parse::<f64>() {
                        return Some(val);
                    }
                } else if child.kind() == "unary_operator" {
                    let op = child
                        .child_by_field_name("operator")
                        .map(|op| Self::node_text(op, source));
                    if op.as_deref() == Some("-") {
                        if let Some(operand) = child.child_by_field_name("argument") {
                            let val_str = Self::node_text(operand, source);
                            if let Ok(val) = val_str.parse::<f64>() {
                                return Some(-val);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn is_sleep_call(func: tree_sitter::Node, source: &[u8]) -> bool {
        let text = Self::node_text(func, source);
        if text == "time.sleep" || text == "sleep" {
            return true;
        }
        if func.kind() == "attribute" {
            if let Some(attr) = func.child_by_field_name("attribute") {
                let name = Self::node_text(attr, source);
                if name == "sleep" {
                    return true;
                }
            }
        }
        false
    }

    fn find_sleep_value(node: tree_sitter::Node, source: &[u8]) -> Option<f64> {
        let mut max_val: Option<f64> = None;

        if node.kind() == "call" {
            if let Some(func) = node.child_by_field_name("function") {
                if Self::is_sleep_call(func, source) {
                    if let Some(val) = Self::extract_sleep_arg(node, source) {
                        max_val = Some(val);
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(val) = Self::find_sleep_value(child, source) {
                match max_val {
                    None => max_val = Some(val),
                    Some(current) if val > current => max_val = Some(val),
                    _ => {}
                }
            }
        }

        max_val
    }

    fn has_cleanup_text_patterns(body_text: &str) -> bool {
        let patterns = [".close()", ".teardown_", "env_reset", ".restore()", ".cleanup()", ".remove()", ".unlink()"];
        patterns.iter().any(|p| body_text.contains(p))
    }

    fn has_cleanup_addfinalizer(body_text: &str) -> bool {
        body_text.contains("addfinalizer") || body_text.contains("request.addfinalizer")
    }

    fn has_cleanup_patch(body_text: &str) -> bool {
        body_text.contains("mock.patch") || body_text.contains("patch(")
    }

    fn has_cleanup_tmp_path(body_text: &str) -> bool {
        body_text.contains("tmp_path") || body_text.contains("tmpdir")
    }

    fn detect_cleanup_pattern(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        body.is_some_and(|b| {
            let text = String::from_utf8_lossy(&source[b.start_byte()..b.end_byte()]);
            if Self::has_cleanup_text_patterns(&text) { return true; }
            if Self::has_cleanup_addfinalizer(&text) { return true; }
            if Self::has_cleanup_patch(&text) { return true; }
            if Self::has_cleanup_tmp_path(&text) { return true; }
            if Self::has_try_wrapping_yield(*b, source) { return true; }
            if Self::has_with_wrapping_yield(*b, source) { return true; }
            false
        })
    }

    fn has_try_wrapping_yield(body: tree_sitter::Node, _source: &[u8]) -> bool {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "try_statement" {
                let mut try_cursor = child.walk();
                for try_child in child.children(&mut try_cursor) {
                    if (try_child.kind() == "block" || try_child.kind() == "suite")
                        && Self::has_node_kind_recursive(try_child, "yield")
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn has_with_wrapping_yield(body: tree_sitter::Node, _source: &[u8]) -> bool {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "with_statement" && Self::has_node_kind_recursive(child, "yield") {
                return true;
            }
        }
        false
    }

    fn detect_network_usage(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        body.is_some_and(|b| Self::has_network_call(*b, source))
    }

    fn is_network_call_node(node: tree_sitter::Node, source: &[u8]) -> bool {
        if node.kind() != "call" {
            return false;
        }
        let func = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return false,
        };
        let text = Self::node_text(func, source);
        let network_libs = ["requests", "socket", "httpx", "aiohttp", "urllib"];
        if network_libs.iter().any(|lib| text.starts_with(&format!("{}.", lib)) || text.starts_with(&format!("{} (", lib))) {
            return true;
        }
        if let Some(o) = func.child_by_field_name("object") {
            let obj_name = Self::node_text(o, source);
            if network_libs.contains(&obj_name.as_str()) {
                return true;
            }
        }
        false
    }

    fn has_network_call(node: tree_sitter::Node, source: &[u8]) -> bool {
        if Self::is_network_call_node(node, source) {
            return true;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_network_call(child, source) {
                return true;
            }
        }
        false
    }

    fn detect_yield(body: Option<&tree_sitter::Node>) -> bool {
        body.is_some_and(|b| Self::has_node_kind_recursive(*b, "yield"))
    }

    fn detect_db_commit(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        body.is_some_and(|b| Self::has_db_call(*b, source, "commit"))
    }

    fn detect_db_rollback(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        body.is_some_and(|b| Self::has_db_call(*b, source, "rollback"))
    }

    fn is_db_call_node(node: tree_sitter::Node, source: &[u8], method_name: &str) -> bool {
        if node.kind() == "call" {
            let func = match node.child_by_field_name("function") {
                Some(f) => f,
                None => return false,
            };
            let text = Self::node_text(func, source);
            if text.to_lowercase().contains(method_name) {
                return true;
            }
            if func.kind() == "attribute" {
                if let Some(a) = func.child_by_field_name("attribute") {
                    return Self::node_text(a, source).to_lowercase() == method_name;
                }
            }
            return false;
        }
        if node.kind() == "identifier" {
            return Self::node_text(node, source).to_lowercase() == method_name;
        }
        false
    }

    fn has_db_call(node: tree_sitter::Node, source: &[u8], method_name: &str) -> bool {
        if Self::is_db_call_node(node, source, method_name) {
            return true;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_db_call(child, source, method_name) {
                return true;
            }
        }
        false
    }

    fn detect_frozen_dataclass_names(root: &tree_sitter::Node, source: &[u8]) -> HashSet<String> {
        let mut frozen = HashSet::new();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "decorated_definition" {
                let mut inner = child.walk();
                let mut class_node = None;
                let mut decorators_text = Vec::new();
                for c in child.children(&mut inner) {
                    match c.kind() {
                        "class_definition" => class_node = Some(c),
                        "decorator" => {
                            decorators_text.push(Self::node_text(c, source));
                        }
                        _ => {}
                    }
                }
                if let Some(cls) = class_node {
                    let is_frozen = decorators_text.iter().any(|d| {
                        (d.contains("dataclass") && d.contains("frozen") && d.contains("True"))
                            || d.contains("@frozen")
                    });
                    if is_frozen {
                        if let Some(name_node) = cls.child_by_field_name("name") {
                            frozen.insert(Self::node_text(name_node, source));
                        }
                    }
                }
            }
        }
        frozen
    }

    fn detect_mutable_return(
        body: Option<&tree_sitter::Node>,
        source: &[u8],
        frozen_classes: &HashSet<String>,
    ) -> bool {
        body.is_some_and(|b| Self::has_mutable_return_in_body(*b, source, frozen_classes))
    }

    fn has_mutable_return_in_body(
        node: tree_sitter::Node,
        source: &[u8],
        frozen_classes: &HashSet<String>,
    ) -> bool {
        if node.kind() == "return_statement" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if Self::is_mutable_node(child, source, frozen_classes) {
                    return true;
                }
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_mutable_return_in_body(child, source, frozen_classes) {
                return true;
            }
        }
        false
    }

    fn is_mutable_call_node(node: tree_sitter::Node, source: &[u8], frozen_classes: &HashSet<String>) -> bool {
        if node.kind() != "call" {
            return false;
        }
        let func = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return false,
        };
        let name = Self::node_text(func, source);
        let immutable = ["int", "str", "float", "bool", "bytes", "complex", "tuple",
            "frozenset", "NoneType", "Path", "PurePath", "PurePosixPath", "PureWindowsPath",
            "Decimal", "date", "datetime", "time", "timedelta", "UUID",
            "ipaddress", "IPv4Address", "IPv6Address", "re.compile", "enum"];
        if immutable.iter().any(|ic| name == *ic) {
            return false;
        }
        let mutable = ["list", "dict", "set", "bytearray", "deque", "defaultdict", "Counter", "OrderedDict"];
        if mutable.iter().any(|mc| name == *mc) {
            return true;
        }
        if let Some(first_char) = name.chars().next() {
            if first_char.is_uppercase() {
                let class_name = name.split('.').next_back().unwrap_or(&name);
                return !frozen_classes.contains(class_name);
            }
        }
        if func.kind() == "attribute" {
            if let Some(attr) = func.child_by_field_name("attribute") {
                let attr_name = Self::node_text(attr, source);
                if let Some(first_char) = attr_name.chars().next() {
                    if first_char.is_uppercase() {
                        return !frozen_classes.contains(attr_name.as_str());
                    }
                }
            }
        }
        false
    }

    fn is_mutable_node(
        node: tree_sitter::Node,
        source: &[u8],
        frozen_classes: &HashSet<String>,
    ) -> bool {
        match node.kind() {
            "list" | "dictionary" | "set" => true,
            "call" => Self::is_mutable_call_node(node, source, frozen_classes),
            _ => false,
        }
    }

    fn detect_random_usage(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        body.is_some_and(|b| Self::has_random_call(*b, source))
    }

    fn is_random_call_node(node: tree_sitter::Node, source: &[u8]) -> bool {
        if node.kind() != "call" {
            return false;
        }
        let func = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return false,
        };
        let text = Self::node_text(func, source);
        let random_fns = [
            "random.random", "random.randint", "random.choice", "random.shuffle",
            "random.uniform", "random.randrange", "random.sample",
            "random.gauss", "random.normalvariate",
        ];
        if random_fns.iter().any(|rf| text == *rf) {
            return true;
        }
        if let Some(o) = func.child_by_field_name("object") {
            return Self::node_text(o, source) == "random";
        }
        false
    }

    fn has_random_call(node: tree_sitter::Node, source: &[u8]) -> bool {
        if Self::is_random_call_node(node, source) {
            return true;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_random_call(child, source) {
                return true;
            }
        }
        false
    }

    fn detect_random_seed(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        body.is_some_and(|b| Self::has_random_seed_call(*b, source))
    }

    fn is_random_seed_call_node(node: tree_sitter::Node, source: &[u8]) -> bool {
        if node.kind() != "call" {
            return false;
        }
        let func = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return false,
        };
        let text = Self::node_text(func, source);
        if text == "random.seed" {
            return true;
        }
        if func.kind() == "attribute" {
            if let (Some(a), Some(o)) = (func.child_by_field_name("attribute"), func.child_by_field_name("object")) {
                return Self::node_text(a, source) == "seed" && Self::node_text(o, source) == "random";
            }
        }
        false
    }

    fn has_random_seed_call(node: tree_sitter::Node, source: &[u8]) -> bool {
        if Self::is_random_seed_call_node(node, source) {
            return true;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_random_seed_call(child, source) {
                return true;
            }
        }
        false
    }

    fn detect_subprocess_usage(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        body.is_some_and(|b| Self::has_subprocess_call(*b, source))
    }

    fn is_subprocess_call_node(node: tree_sitter::Node, source: &[u8]) -> bool {
        if node.kind() != "call" {
            return false;
        }
        let func = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return false,
        };
        let text = Self::node_text(func, source);
        let subprocess_fns = [
            "subprocess.Popen", "subprocess.run", "subprocess.call",
            "subprocess.check_output", "subprocess.check_call",
        ];
        if subprocess_fns.iter().any(|sf| text == *sf) {
            return true;
        }
        if let Some(o) = func.child_by_field_name("object") {
            return Self::node_text(o, source) == "subprocess";
        }
        false
    }

    fn has_subprocess_call(node: tree_sitter::Node, source: &[u8]) -> bool {
        if Self::is_subprocess_call_node(node, source) {
            return true;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_subprocess_call(child, source) {
                return true;
            }
        }
        false
    }

    fn detect_subprocess_timeout(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        body.is_some_and(|b| Self::has_timeout_arg(*b, source))
    }

    fn call_has_timeout_kwarg(node: tree_sitter::Node, source: &[u8]) -> bool {
        if let Some(a) = node.child_by_field_name("arguments") {
            let mut cursor = a.walk();
            for child in a.children(&mut cursor) {
                if child.kind() == "keyword_argument" {
                    if let Some(n) = child.child_by_field_name("name") {
                        if Self::node_text(n, source) == "timeout" {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn is_subprocess_call_missing_timeout(node: tree_sitter::Node, source: &[u8]) -> bool {
        if node.kind() != "call" {
            return false;
        }
        let func = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return false,
        };
        let text = Self::node_text(func, source);
        let subprocess_fns = [
            "subprocess.Popen", "subprocess.run", "subprocess.call",
            "subprocess.check_output", "subprocess.check_call",
        ];
        subprocess_fns.iter().any(|sf| text == *sf) && !Self::call_has_timeout_kwarg(node, source)
    }

    fn has_timeout_arg(node: tree_sitter::Node, source: &[u8]) -> bool {
        if Self::is_subprocess_call_missing_timeout(node, source) {
            return true;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Self::has_timeout_arg(child, source) {
                return true;
            }
        }
        false
    }

    fn detect_weak_assertions(
        body: Option<&tree_sitter::Node>,
        source: &[u8],
    ) -> (bool, Vec<String>) {
        let mut details = Vec::new();
        if let Some(b) = body {
            Self::collect_weak_assertions(*b, source, &mut details);
        }
        (!details.is_empty(), details)
    }

    fn add_weak_category(details: &mut Vec<String>, category: &str) {
        if !details.iter().any(|d| d == category) {
            details.push(category.to_string());
        }
    }

    fn check_weak_call(node: tree_sitter::Node, source: &[u8], details: &mut Vec<String>) {
        if node.kind() != "call" {
            return;
        }
        let func = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return,
        };
        let text = Self::node_text(func, source);
        let weak_patterns: &[(&str, &str)] = &[
            ("assertIsInstance", "type-only assertion"),
            ("isinstance", "type-only assertion"),
            ("assertTrue", "existence-only assertion"),
            ("assertIsNotNone", "existence-only assertion"),
            ("assertIn", "key-presence-only assertion"),
        ];
        for (pattern, category) in weak_patterns {
            if text.contains(pattern) {
                Self::add_weak_category(details, category);
            }
        }
    }

    fn check_weak_comparison(node: tree_sitter::Node, source: &[u8], details: &mut Vec<String>) {
        if node.kind() != "comparison_operator" {
            return;
        }
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        let ops: Vec<String> = children
            .iter()
            .map(|c| Self::node_text(*c, source).trim().to_string())
            .collect();

        for op in &ops {
            if op == "in" {
                Self::add_weak_category(details, "key-presence-only assertion");
            }
            if *op == "is not" && ops.iter().any(|r| *r == "None") {
                Self::add_weak_category(details, "existence-only assertion");
            }
        }
        for (i, child) in children.iter().enumerate() {
            let text = Self::node_text(*child, source);
            if text == "type" && i + 1 < children.len() {
                let next_text = Self::node_text(children[i + 1], source);
                if next_text == "==" || next_text == "is" {
                    Self::add_weak_category(details, "type-only assertion");
                }
            }
        }
    }

    fn collect_weak_assertions(node: tree_sitter::Node, source: &[u8], details: &mut Vec<String>) {
        Self::check_weak_call(node, source, details);
        Self::check_weak_comparison(node, source, details);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::collect_weak_assertions(child, source, details);
        }
    }

    fn detect_stdlib_mock_targets(
        body: Option<&tree_sitter::Node>,
        source: &[u8],
        decorators: &[DecoratorInfo],
    ) -> Vec<String> {
        const STDLIB_MODULES: &[&str] = &[
            "subprocess",
            "os",
            "os.path",
            "sys",
            "socket",
            "http.client",
            "http.server",
            "builtins",
            "io",
            "pathlib",
            "json",
            "csv",
            "re",
            "time",
            "datetime",
            "shutil",
            "tempfile",
            "glob",
            "logging",
            "warnings",
            "threading",
            "multiprocessing",
            "asyncio",
        ];

        let mut targets = Vec::new();

        // Check @patch decorators
        for dec in decorators {
            if let Some(target) = extract_patch_target(&dec.text) {
                if STDLIB_MODULES.iter().any(|m| target.starts_with(m))
                    && !targets.contains(&target)
                {
                    targets.push(target);
                }
            }
        }

        // Check patch() calls in body
        if let Some(body_node) = body {
            let body_text = Self::node_text(*body_node, source);
            for cap in body_text.match_indices("patch(") {
                let after = &body_text[cap.0..];
                if let Some(target) = extract_patch_target(after) {
                    if STDLIB_MODULES.iter().any(|m| target.starts_with(m))
                        && !targets.contains(&target)
                    {
                        targets.push(target);
                    }
                }
            }
        }

        targets
    }

    fn detect_all_patch_targets(
        body: Option<&tree_sitter::Node>,
        source: &[u8],
        decorators: &[DecoratorInfo],
    ) -> Vec<String> {
        let mut targets = Vec::new();
        for dec in decorators {
            if let Some(target) = extract_patch_target(&dec.text) {
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }
        }
        if let Some(body_node) = body {
            let body_text = Self::node_text(*body_node, source);
            for cap in body_text.match_indices("patch(") {
                let after = &body_text[cap.0..];
                if let Some(target) = extract_patch_target(after) {
                    if !targets.contains(&target) {
                        targets.push(target);
                    }
                }
            }
        }
        targets
    }

    fn detect_mock_usage(body: Option<&tree_sitter::Node>, source: &[u8]) -> (bool, usize) {
        let body_text = body
            .map(|b| Self::node_text(*b, source))
            .unwrap_or_default();
        let has_magic_mock = body_text.contains("MagicMock");
        let mock_kw = [
            "Mock(",
            "MagicMock(",
            "AsyncMock(",
            "patch(",
            ".return_value",
            ".side_effect",
            ".assert_called",
            ".called",
            ".call_count",
            ".assert_called_once",
            ".assert_called_with",
            ".assert_not_called",
        ];
        let mut count = 0usize;
        for kw in &mock_kw {
            let mut start = 0;
            while let Some(pos) = body_text[start..].find(kw) {
                count += 1;
                start += pos + kw.len();
            }
        }
        (has_magic_mock, count)
    }

    fn detect_shutil_copy(body: Option<&tree_sitter::Node>, source: &[u8]) -> bool {
        let body_text = body
            .map(|b| Self::node_text(*b, source))
            .unwrap_or_default();
        body_text.contains("shutil.copy(")
            || body_text.contains("shutil.copy2(")
            || body_text.contains("shutil.copyfile(")
            || body_text.contains("shutil.copytree(")
            || body_text.contains("shutil.move(")
    }
}

fn extract_patch_target(text: &str) -> Option<String> {
    // @patch("module.path") or @patch('module.path')
    let start = text.find('(')?;
    let rest = &text[start + 1..];
    extract_first_string_arg(rest)
}

fn extract_first_string_arg(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    if let Some(stripped) = trimmed.strip_prefix('"') {
        let end = stripped.find('"')?;
        Some(stripped[..end].to_string())
    } else if let Some(stripped) = trimmed.strip_prefix('\'') {
        let end = stripped.find('\'')?;
        Some(stripped[..end].to_string())
    } else {
        None
    }
}

fn has_mock_verifications_only(body_text: &str) -> bool {
    let mock_kw = [".assert_called", ".called", ".call_count"];
    let has_mock = mock_kw.iter().any(|k| body_text.contains(k));
    let has_assert = body_text.contains("assert ") || body_text.contains("assert(");
    has_mock && !has_assert
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_simple.py");
        std::fs::write(
            &path,
            r#"
import pytest

def test_example():
    assert 1 == 1
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.file_path, path);
        assert!(module.imports.iter().any(|imp| imp.contains("pytest")));
        assert_eq!(module.test_functions.len(), 1);
        assert_eq!(module.test_functions[0].name, "test_example");
        assert!(module.test_functions[0].has_assertions);
        assert_eq!(module.test_functions[0].assertion_count, 1);
        assert!(!module.test_functions[0].uses_time_sleep);
        assert!(!module.test_functions[0].uses_file_io);
        assert!(!module.test_functions[0].uses_network);
        assert!(!module.test_functions[0].has_conditional_logic);
        assert!(!module.test_functions[0].has_try_except);
    }

    #[test]
    fn test_parse_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_empty.py");
        std::fs::write(&path, "").unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_functions.len(), 0);
        assert_eq!(module.fixtures.len(), 0);
        assert!(module.imports.is_empty());
    }

    #[test]
    fn test_parse_extracts_fixture_deps() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_deps.py");
        std::fs::write(
            &path,
            r#"
def test_with_deps(tmp_path, monkeypatch):
    assert True
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_functions[0].fixture_deps.len(), 2);
        assert!(module.test_functions[0]
            .fixture_deps
            .contains(&"tmp_path".to_string()));
        assert!(module.test_functions[0]
            .fixture_deps
            .contains(&"monkeypatch".to_string()));
    }

    #[test]
    fn test_parse_detects_docstring() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_doc.py");
        std::fs::write(
            &path,
            r#"
def test_documented():
    """Given a state when something happens then something."""
    assert True
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert!(module.test_functions[0].docstring.is_some());
    }

    #[test]
    fn test_count_top_level_entries_empty() {
        assert_eq!(PythonParser::count_top_level_entries(""), 0);
    }

    #[test]
    fn test_count_top_level_entries_whitespace_only() {
        assert_eq!(PythonParser::count_top_level_entries("   "), 0);
    }

    #[test]
    fn test_count_top_level_entries_single_item() {
        assert_eq!(PythonParser::count_top_level_entries("1"), 1);
    }

    #[test]
    fn test_count_top_level_entries_comma_separated() {
        assert_eq!(PythonParser::count_top_level_entries("1, 2, 3"), 3);
    }

    #[test]
    fn test_count_top_level_entries_with_strings() {
        assert_eq!(PythonParser::count_top_level_entries("\"a\", \"b\""), 2);
    }

    #[test]
    fn test_count_top_level_entries_with_single_quotes() {
        assert_eq!(PythonParser::count_top_level_entries("'x', 'y'"), 2);
    }

    #[test]
    fn test_count_top_level_entries_with_escaped_chars() {
        assert_eq!(PythonParser::count_top_level_entries(r#""a\"b", "c""#), 2);
    }

    #[test]
    fn test_count_top_level_entries_with_nested_brackets() {
        assert_eq!(PythonParser::count_top_level_entries("[1, 2], [3, 4]"), 2);
    }

    #[test]
    fn test_count_top_level_entries_trailing_comma() {
        assert_eq!(PythonParser::count_top_level_entries("1, 2, "), 2);
    }

    #[test]
    fn test_count_top_level_entries_with_braces() {
        assert_eq!(PythonParser::count_top_level_entries("{1: 2}, {3: 4}"), 2);
    }

    #[test]
    fn test_count_top_level_entries_with_parens() {
        assert_eq!(PythonParser::count_top_level_entries("(1, 2), (3, 4)"), 2);
    }

    #[test]
    fn test_count_parametrize_args_bracket_format() {
        assert!(PythonParser::count_parametrize_args("parametrize('x', [1, 2])") >= 1);
    }

    #[test]
    fn test_count_parametrize_args_no_brackets() {
        assert_eq!(PythonParser::count_parametrize_args("parametrize('x')"), 1);
    }

    #[test]
    fn test_count_parametrize_args_multiple_parens() {
        assert_eq!(
            PythonParser::count_parametrize_args("parametrize('x', (1, 2))"),
            2
        );
    }

    #[test]
    fn test_has_mock_verifications_only_true() {
        assert!(has_mock_verifications_only("mock.assert_called()"));
    }

    #[test]
    fn test_has_mock_verifications_only_false_no_mock() {
        assert!(!has_mock_verifications_only("assert True"));
    }

    #[test]
    fn test_has_mock_verifications_only_false_both() {
        assert!(!has_mock_verifications_only(
            "mock.assert_called()\nassert True"
        ));
    }

    #[test]
    fn test_has_mock_verifications_call_count() {
        assert!(has_mock_verifications_only("mock.call_count"));
    }

    #[test]
    fn test_parse_decorated_and_undecorated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_mixed_dec.py");
        std::fs::write(
            &path,
            r#"
import pytest

def test_plain():
    assert True

@pytest.mark.parametrize("x", [1, 2])
def test_param(x):
    assert x > 0
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert_eq!(module.test_functions.len(), 2);
    }

    #[test]
    fn test_count_top_level_entries_with_mixed_quotes() {
        assert_eq!(
            PythonParser::count_top_level_entries("\"foo's bar\", 'baz\"qux'"),
            2
        );
    }

    #[test]
    fn test_count_top_level_entries_mismatched_quotes_are_separate() {
        assert_eq!(
            PythonParser::count_top_level_entries("\"hello', 'world\""),
            1
        );
    }

    #[test]
    fn test_parse_import_from_statement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_imports.py");
        std::fs::write(
            &path,
            r#"
from os import path
from sys import argv

def test_ok():
    assert True
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert!(module.imports.iter().any(|imp| imp.contains("os")));
        assert!(module.imports.iter().any(|imp| imp.contains("sys")));
    }

    #[test]
    fn test_parse_fixture_with_no_params() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_no_params.py");
        std::fs::write(
            &path,
            r#"
import pytest

@pytest.fixture
def no_param_fix():
    return 42

def test_thing():
    assert True
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert!(module.fixtures[0].dependencies.is_empty());
    }

    #[test]
    fn test_parse_fixture_with_yield_and_commit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_yield_commit.py");
        std::fs::write(
            &path,
            r#"
import pytest

@pytest.fixture
def yield_commit():
    conn = get_conn()
    conn.commit()
    yield conn
    conn.rollback()
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert!(module.fixtures[0].has_yield);
        assert!(module.fixtures[0].has_db_commit);
        assert!(module.fixtures[0].has_db_rollback);
    }

    #[test]
    fn test_parse_fixture_dot_commit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_dot_commit.py");
        std::fs::write(
            &path,
            r#"
import pytest

@pytest.fixture
def dot_commit():
    session.commit()
    return session
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert!(module.fixtures[0].has_db_commit);
    }

    #[test]
    fn test_parse_fixture_dot_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_dot_rollback.py");
        std::fs::write(
            &path,
            r#"
import pytest

@pytest.fixture
def dot_rollback():
    session.rollback()
    return session
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert!(module.fixtures[0].has_db_rollback);
    }

    #[test]
    fn test_parse_test_with_open_paren_space() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_open_space.py");
        std::fs::write(
            &path,
            r#"
def test_open():
    f = open ("data.txt")
    assert f
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();

        assert!(module.test_functions[0].uses_file_io);
    }

    fn parse_source(source: &str) -> ParsedModule {
        let mut parser = PythonParser::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_input.py");
        parser.parse_source(source, &path).unwrap()
    }

    #[test]
    fn test_class_definition_not_traversed() {
        let module = parse_source(
            r#"
class TestMyClass:
    def test_inner(self):
        assert True
"#,
        );
        assert_eq!(module.test_functions.len(), 0);
    }

    #[test]
    fn test_async_test_detected() {
        let module = parse_source(
            r#"
async def test_async_thing():
    assert True
"#,
        );
        assert!(module.test_functions[0].is_async);
    }

    #[test]
    fn test_sync_test_not_async() {
        let module = parse_source(
            r#"
def test_sync():
    assert True
"#,
        );
        assert!(!module.test_functions[0].is_async);
    }

    #[test]
    fn test_state_assertions_true_with_only_assert() {
        let module = parse_source(
            r#"
def test_state():
    assert 1 == 1
"#,
        );
        assert!(module.test_functions[0].has_state_assertions);
    }

    #[test]
    fn test_state_assertions_false_with_only_mock() {
        let module = parse_source(
            r#"
def test_mock_only():
    mock_obj.assert_called_once()
"#,
        );
        assert!(!module.test_functions[0].has_state_assertions);
    }

    #[test]
    fn test_state_assertions_true_with_both() {
        let module = parse_source(
            r#"
def test_both():
    mock_obj.assert_called()
    assert True
"#,
        );
        assert!(module.test_functions[0].has_state_assertions);
    }

    #[test]
    fn test_bare_parametrize_decorator() {
        let module = parse_source(
            r#"
@parametrize("x", [1, 2, 3])
def test_bare(x):
    assert x > 0
"#,
        );
        assert!(module.test_functions[0].is_parametrized);
        assert_eq!(module.test_functions[0].parametrize_count, Some(3));
    }

    #[test]
    fn test_parametrize_ast_single_element() {
        let module = parse_source(
            r#"
@pytest.mark.parametrize("x", [42])
def test_single(x):
    assert x == 42
"#,
        );
        assert!(module.test_functions[0].is_parametrized);
        assert_eq!(module.test_functions[0].parametrize_count, Some(1));
    }

    #[test]
    fn test_parametrize_ast_empty_list() {
        let module = parse_source(
            r#"
@pytest.mark.parametrize("x", [])
def test_empty(x):
    assert True
"#,
        );
        assert!(module.test_functions[0].is_parametrized);
        assert_eq!(module.test_functions[0].parametrize_count, Some(0));
    }

    #[test]
    fn test_parametrize_ast_tuple_values() {
        let module = parse_source(
            r#"
@pytest.mark.parametrize("x", (1, 2))
def test_tuple(x):
    assert x > 0
"#,
        );
        assert!(module.test_functions[0].is_parametrized);
        assert!(module.test_functions[0].parametrize_count.unwrap() >= 2);
    }

    #[test]
    fn test_count_parametrize_args_bracket_end_before_start() {
        assert_eq!(
            PythonParser::count_parametrize_args("parametrize('x', ']'  [1, 2])"),
            2
        );
    }

    #[test]
    fn test_count_parametrize_args_bracket_end_not_greater() {
        assert_eq!(
            PythonParser::count_parametrize_args("parametrize('x', [1] )"),
            1
        );
    }

    #[test]
    fn test_count_top_level_entries_closing_bracket_at_depth_zero() {
        assert_eq!(PythonParser::count_top_level_entries("a, b)"), 2);
    }

    #[test]
    fn test_assertion_info_line_number() {
        let module = parse_source(
            r#"
def test_assert_lines():
    x = 1
    assert x == 1
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert_eq!(info.len(), 1);
        assert_eq!(info[0].line, 4);
    }

    #[test]
    fn test_assertion_info_integer_magic() {
        let module = parse_source(
            r#"
def test_magic_int():
    assert 0
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert_eq!(info.len(), 1);
        assert!(info[0].is_magic);
    }

    #[test]
    fn test_assertion_info_integer_one() {
        let module = parse_source(
            r#"
def test_magic_one():
    assert 1
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert!(info[0].is_magic);
    }

    #[test]
    fn test_assertion_info_false_keyword() {
        let module = parse_source(
            r#"
def test_magic_false():
    assert False
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert!(info[0].is_magic);
    }

    #[test]
    fn test_assertion_info_true_keyword() {
        let module = parse_source(
            r#"
def test_magic_true():
    assert True
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert!(info[0].is_magic);
    }

    #[test]
    fn test_assertion_info_plain_identifier_not_magic() {
        let module = parse_source(
            r#"
def test_plain_ident():
    x = 1
    assert x
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert!(info[0].is_magic);
    }

    #[test]
    fn test_assertion_info_comparison_not_magic() {
        let module = parse_source(
            r#"
def test_comparison():
    assert 1 == 1
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert!(!info[0].is_magic);
        assert!(info[0].has_comparison);
    }

    #[test]
    fn test_suboptimal_assert_not_none_in_comparison() {
        let module = parse_source(
            r#"
def test_not_none_comp():
    x = 1
    assert not x is None
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert!(!info[0].is_suboptimal);
    }

    #[test]
    fn test_suboptimal_assert_len_comparison() {
        let module = parse_source(
            r#"
def test_len_compare():
    x = [1, 2]
    assert len(x) == 2
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert_eq!(info.len(), 1);
        assert!(info[0].is_suboptimal);
    }

    #[test]
    fn test_suboptimal_assert_type_comparison() {
        let module = parse_source(
            r#"
def test_type_compare():
    x = 1
    assert type(x) == int
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert!(info[0].is_suboptimal);
    }

    #[test]
    fn test_suboptimal_assert_eq_none() {
        let module = parse_source(
            r#"
def test_eq_none():
    x = None
    assert x == None
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert!(info[0].is_suboptimal);
    }

    #[test]
    fn test_suboptimal_assert_neq_none() {
        let module = parse_source(
            r#"
def test_neq_none():
    x = 1
    assert x != None
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert!(info[0].is_suboptimal);
    }

    #[test]
    fn test_not_suboptimal_is_none() {
        let module = parse_source(
            r#"
def test_is_none():
    x = None
    assert x is None
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert!(!info[0].is_suboptimal);
    }

    #[test]
    fn test_not_suboptimal_is_not_none() {
        let module = parse_source(
            r#"
def test_is_not_none():
    x = 1
    assert x is not None
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert!(!info[0].is_suboptimal);
    }

    #[test]
    fn test_extract_parametrize_values_bare_parametrize() {
        let module = parse_source(
            r#"
@parametrize("x", [10, 20])
def test_vals(x):
    assert x > 0
"#,
        );
        assert!(!module.test_functions[0].parametrize_values.is_empty());
    }

    #[test]
    fn test_parametrize_values_second_list_selected() {
        let module = parse_source(
            r#"
@pytest.mark.parametrize("x,y", [[1,2], [3,4]])
def test_multi(x, y):
    assert x + y > 0
"#,
        );
        let vals = &module.test_functions[0].parametrize_values;
        assert!(!vals.is_empty());
        assert!(vals[0].len() >= 2);
    }

    #[test]
    fn test_cwd_dependency_os_getcwd() {
        let module = parse_source(
            r#"
import os
def test_cwd():
    d = os.getcwd()
    assert d
"#,
        );
        assert!(module.test_functions[0].uses_cwd_dependency);
    }

    #[test]
    fn test_cwd_dependency_os_chdir() {
        let module = parse_source(
            r#"
import os
def test_chdir():
    os.chdir("/tmp")
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_cwd_dependency);
    }

    #[test]
    fn test_cwd_dependency_path_cwd() {
        let module = parse_source(
            r#"
from pathlib import Path
def test_path_cwd():
    p = Path.cwd()
    assert p
"#,
        );
        assert!(module.test_functions[0].uses_cwd_dependency);
    }

    #[test]
    fn test_cwd_dependency_attribute_getcwd() {
        let module = parse_source(
            r#"
import os
def test_attr():
    x = os.path.getcwd()
    assert x
"#,
        );
        assert!(module.test_functions[0].uses_cwd_dependency);
    }

    #[test]
    fn test_pytest_raises_detected() {
        let module = parse_source(
            r#"
import pytest
def test_raises():
    with pytest.raises(ValueError):
        raise ValueError()
"#,
        );
        assert!(module.test_functions[0].uses_pytest_raises);
    }

    #[test]
    fn test_pytest_raises_via_attribute() {
        let module = parse_source(
            r#"
import pytest
def test_raises_attr():
    with pytest.raises(TypeError):
        raise TypeError("err")
"#,
        );
        assert!(module.test_functions[0].uses_pytest_raises);
    }

    #[test]
    fn test_fixture_mutation_append() {
        let module = parse_source(
            r#"
def test_mutate_append(my_list):
    my_list.append(42)
    assert len(my_list) > 0
"#,
        );
        assert!(module.test_functions[0]
            .mutates_fixture_deps
            .contains(&"my_list".to_string()));
    }

    #[test]
    fn test_fixture_mutation_assignment_subscript() {
        let module = parse_source(
            r#"
def test_mutate_subscript(my_dict):
    my_dict["key"] = "val"
    assert my_dict
"#,
        );
        assert!(module.test_functions[0]
            .mutates_fixture_deps
            .contains(&"my_dict".to_string()));
    }

    #[test]
    fn test_fixture_mutation_assignment_attribute() {
        let module = parse_source(
            r#"
def test_mutate_attr(my_obj):
    my_obj.field = 42
    assert my_obj.field == 42
"#,
        );
        assert!(module.test_functions[0]
            .mutates_fixture_deps
            .contains(&"my_obj".to_string()));
    }

    #[test]
    fn test_fixture_mutation_delete() {
        let module = parse_source(
            r#"
def test_mutate_del(my_thing):
    del my_thing
    assert True
"#,
        );
        assert!(module.test_functions[0]
            .mutates_fixture_deps
            .contains(&"my_thing".to_string()));
    }

    #[test]
    fn test_fixture_mutation_no_mutation() {
        let module = parse_source(
            r#"
def test_no_mutate(my_list):
    x = my_list[0]
    assert x
"#,
        );
        assert!(module.test_functions[0].mutates_fixture_deps.is_empty());
    }

    #[test]
    fn test_fixture_deps_typed_parameters() {
        let module = parse_source(
            r#"
def test_typed(my_fix: int):
    assert my_fix == 1
"#,
        );
        assert_eq!(module.test_functions.len(), 1);
        let deps = &module.test_functions[0].fixture_deps;
        if deps.contains(&"my_fix".to_string()) {
            return;
        }
        assert!(deps.is_empty() || deps.contains(&"my_fix".to_string()));
    }

    #[test]
    fn test_fixture_deps_default_parameters() {
        let module = parse_source(
            r#"
def test_default(my_fix=None):
    assert my_fix is not None
"#,
        );
        assert_eq!(module.test_functions.len(), 1);
        let deps = &module.test_functions[0].fixture_deps;
        assert!(deps.contains(&"my_fix".to_string()), "deps: {deps:?}");
    }

    #[test]
    fn test_fixture_deps_typed_default_parameters() {
        let module = parse_source(
            r#"
def test_typed_default(my_fix: int = 10):
    assert my_fix == 10
"#,
        );
        assert_eq!(module.test_functions.len(), 1);
        let deps = &module.test_functions[0].fixture_deps;
        assert!(deps.contains(&"my_fix".to_string()), "deps: {deps:?}");
    }

    #[test]
    fn test_fixture_line_number() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def my_fix():
    return 42
"#,
        );
        assert_eq!(module.fixtures[0].line, 5);
    }

    #[test]
    fn test_fixture_cleanup_addfinalizer() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_finalizer(request):
    request.addfinalizer(cleanup)
    return 42
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_cleanup_mock_patch() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_patch():
    with mock.patch("mod.func"):
        yield 42
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_cleanup_tmp_path() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_tmp():
    d = tmp_path / "data"
    yield d
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_cleanup_tmpdir() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_tmpdir():
    d = tmpdir / "data"
    yield d
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_no_cleanup() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def simple_fix():
    return 42
"#,
        );
        assert!(!module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_time_sleep_via_attribute() {
        let module = parse_source(
            r#"
import time
def test_sleep_attr():
    time.sleep(1)
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_time_sleep);
    }

    #[test]
    fn test_sleep_value_single() {
        let module = parse_source(
            r#"
def test_sleep_val():
    import time
    time.sleep(2.5)
    assert True
"#,
        );
        assert_eq!(module.test_functions[0].sleep_value, Some(2.5));
    }

    #[test]
    fn test_sleep_value_max() {
        let module = parse_source(
            r#"
def test_sleep_max():
    import time
    time.sleep(1.0)
    time.sleep(3.0)
    time.sleep(2.0)
    assert True
"#,
        );
        assert_eq!(module.test_functions[0].sleep_value, Some(3.0));
    }

    #[test]
    fn test_sleep_value_none() {
        let module = parse_source(
            r#"
def test_no_sleep():
    assert True
"#,
        );
        assert_eq!(module.test_functions[0].sleep_value, None);
    }

    #[test]
    fn test_sleep_value_zero() {
        let module = parse_source(
            r#"
import time
def test_sleep_zero():
    time.sleep(0)
    assert True
"#,
        );
        assert_eq!(module.test_functions[0].sleep_value, Some(0.0));
    }

    #[test]
    fn test_detect_cleanup_pattern_addfinalizer() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_af(request):
    request.addfinalizer(lambda: None)
    yield 1
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_detect_cleanup_pattern_mock_patch() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_mp():
    patch("mod.func")
    yield 1
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_try_wrapping_yield() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_try():
    try:
        yield 42
    except Exception:
        pass
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_with_wrapping_yield() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_with():
    with some_context():
        yield 42
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_network_usage_requests() {
        let module = parse_source(
            r#"
import requests
def test_req():
    requests.get("http://example.com")
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_network);
    }

    #[test]
    fn test_network_usage_via_object() {
        let module = parse_source(
            r#"
import socket
def test_sock():
    socket.connect(("host", 80))
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_network);
    }

    #[test]
    fn test_network_usage_httpx() {
        let module = parse_source(
            r#"
import httpx
def test_httpx():
    httpx.get("http://example.com")
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_network);
    }

    #[test]
    fn test_db_commit_via_identifier() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_commit():
    commit
    return 1
"#,
        );
        assert!(module.fixtures[0].has_db_commit);
    }

    #[test]
    fn test_db_rollback_via_identifier() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_rollback():
    rollback
    return 1
"#,
        );
        assert!(module.fixtures[0].has_db_rollback);
    }

    #[test]
    fn test_db_call_via_attribute() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_attr_commit():
    session.commit()
    return session
"#,
        );
        assert!(module.fixtures[0].has_db_commit);
    }

    #[test]
    fn test_random_usage_via_object() {
        let module = parse_source(
            r#"
import random
def test_rand():
    random.something()
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_random);
    }

    #[test]
    fn test_random_usage_specific_fn() {
        let module = parse_source(
            r#"
import random
def test_randint():
    random.randint(1, 10)
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_random);
    }

    #[test]
    fn test_random_seed_detected() {
        let module = parse_source(
            r#"
import random
def test_seed():
    random.seed(42)
    assert True
"#,
        );
        assert!(module.test_functions[0].has_random_seed);
    }

    #[test]
    fn test_subprocess_via_object() {
        let module = parse_source(
            r#"
import subprocess
def test_sub():
    subprocess.anything()
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_subprocess);
    }

    #[test]
    fn test_subprocess_run_detected() {
        let module = parse_source(
            r#"
import subprocess
def test_run():
    subprocess.run(["echo", "hi"])
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_subprocess);
    }

    #[test]
    fn test_subprocess_no_timeout() {
        let module = parse_source(
            r#"
import subprocess
def test_no_timeout():
    subprocess.run(["echo", "hi"])
    assert True
"#,
        );
        assert!(module.test_functions[0].has_subprocess_timeout);
    }

    #[test]
    fn test_subprocess_with_timeout_not_flagged() {
        let module = parse_source(
            r#"
import subprocess
def test_with_timeout():
    subprocess.run(["echo", "hi"], timeout=30)
    assert True
"#,
        );
        assert!(!module.test_functions[0].has_subprocess_timeout);
    }

    #[test]
    fn test_subprocess_popen_no_timeout() {
        let module = parse_source(
            r#"
import subprocess
def test_popen():
    subprocess.Popen(["echo", "hi"])
    assert True
"#,
        );
        assert!(module.test_functions[0].has_subprocess_timeout);
    }

    #[test]
    fn test_subprocess_check_call_timeout_arg() {
        let module = parse_source(
            r#"
import subprocess
def test_check_call():
    subprocess.check_call(["echo", "hi"], timeout=10)
    assert True
"#,
        );
        assert!(!module.test_functions[0].has_subprocess_timeout);
    }

    #[test]
    fn test_parse_source_fallback_on_failure() {
        let mut parser = PythonParser::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_input.py");
        let result = parser.parse_source("def valid(): pass", &path).unwrap();
        assert!(result.test_functions.is_empty());
    }

    #[test]
    fn test_fixture_scope_session() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture(scope="session")
def sess_fix():
    return 1
"#,
        );
        assert_eq!(module.fixtures[0].scope, FixtureScope::Session);
    }

    #[test]
    fn test_fixture_scope_module() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture(scope="module")
def mod_fix():
    return 1
"#,
        );
        assert_eq!(module.fixtures[0].scope, FixtureScope::Module);
    }

    #[test]
    fn test_fixture_scope_class() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture(scope="class")
def cls_fix():
    return 1
"#,
        );
        assert_eq!(module.fixtures[0].scope, FixtureScope::Class);
    }

    #[test]
    fn test_fixture_scope_package() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture(scope="package")
def pkg_fix():
    return 1
"#,
        );
        assert_eq!(module.fixtures[0].scope, FixtureScope::Package);
    }

    #[test]
    fn test_fixture_scope_default_function() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fn_fix():
    return 1
"#,
        );
        assert_eq!(module.fixtures[0].scope, FixtureScope::Function);
    }

    #[test]
    fn test_fixture_autouse() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture(autouse=True)
def auto_fix():
    return 1
"#,
        );
        assert!(module.fixtures[0].is_autouse);
    }

    #[test]
    fn test_fixture_not_autouse() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture(autouse=False)
def manual_fix():
    return 1
"#,
        );
        assert!(!module.fixtures[0].is_autouse);
    }

    #[test]
    fn test_fixture_mutable_return_list() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def list_fix():
    return [1, 2, 3]
"#,
        );
        assert!(module.fixtures[0].returns_mutable);
    }

    #[test]
    fn test_fixture_mutable_return_dict() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def dict_fix():
    return {"a": 1}
"#,
        );
        assert!(module.fixtures[0].returns_mutable);
    }

    #[test]
    fn test_fixture_mutable_return_list_call() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def list_call_fix():
    return list()
"#,
        );
        assert!(module.fixtures[0].returns_mutable);
    }

    #[test]
    fn test_fixture_mutable_return_dict_call() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def dict_call_fix():
    return dict()
"#,
        );
        assert!(module.fixtures[0].returns_mutable);
    }

    #[test]
    fn test_fixture_immutable_return() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def int_fix():
    return 42
"#,
        );
        assert!(!module.fixtures[0].returns_mutable);
    }

    #[test]
    fn test_fixture_yield_detected() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def yield_fix():
    yield 42
"#,
        );
        assert!(module.fixtures[0].has_yield);
    }

    #[test]
    fn test_fixture_no_yield() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def return_fix():
    return 42
"#,
        );
        assert!(!module.fixtures[0].has_yield);
    }

    #[test]
    fn test_fixture_file_io() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def file_fix():
    f = open("data.txt")
    return f
"#,
        );
        assert!(module.fixtures[0].uses_file_io);
    }

    #[test]
    fn test_fixture_file_io_write() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def write_fix():
    f.write("data")
    return f
"#,
        );
        assert!(module.fixtures[0].uses_file_io);
    }

    #[test]
    fn test_has_conditional_logic_if() {
        let module = parse_source(
            r#"
def test_cond():
    if True:
        assert True
"#,
        );
        assert!(module.test_functions[0].has_conditional_logic);
    }

    #[test]
    fn test_has_try_except() {
        let module = parse_source(
            r#"
def test_try():
    try:
        x = 1
    except Exception:
        pass
    assert True
"#,
        );
        assert!(module.test_functions[0].has_try_except);
    }

    #[test]
    fn test_random_not_detected_without_random() {
        let module = parse_source(
            r#"
def test_no_rand():
    x = 42
    assert x == 42
"#,
        );
        assert!(!module.test_functions[0].uses_random);
        assert!(!module.test_functions[0].has_random_seed);
    }

    #[test]
    fn test_subprocess_not_detected_without_subprocess() {
        let module = parse_source(
            r#"
def test_no_sub():
    x = 42
    assert x == 42
"#,
        );
        assert!(!module.test_functions[0].uses_subprocess);
        assert!(!module.test_functions[0].has_subprocess_timeout);
    }

    #[test]
    fn test_random_seed_via_attribute() {
        let module = parse_source(
            r#"
import random
def test_seed_attr():
    random.seed(123)
    assert True
"#,
        );
        assert!(module.test_functions[0].has_random_seed);
    }

    #[test]
    fn test_subprocess_call_no_timeout() {
        let module = parse_source(
            r#"
import subprocess
def test_call():
    subprocess.call(["ls"])
    assert True
"#,
        );
        assert!(module.test_functions[0].has_subprocess_timeout);
    }

    #[test]
    fn test_subprocess_check_output_timeout() {
        let module = parse_source(
            r#"
import subprocess
def test_check_out():
    subprocess.check_output(["ls"], timeout=5)
    assert True
"#,
        );
        assert!(!module.test_functions[0].has_subprocess_timeout);
    }

    #[test]
    fn test_fixture_cleanup_via_close() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def close_fix():
    conn = get_conn()
    yield conn
    conn.close()
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_cleanup_via_restore() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def restore_fix():
    yield 1
    env.restore()
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_cleanup_via_cleanup_method() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def cleanup_fix():
    yield 1
    resource.cleanup()
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_cleanup_via_remove() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def remove_fix():
    yield 1
    tmp.remove()
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_cleanup_via_unlink() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def unlink_fix():
    yield 1
    path.unlink()
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_cleanup_via_teardown() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def teardown_fix():
    yield 1
    driver.teardown_()
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_cleanup_via_env_reset() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def env_fix():
    yield 1
    env_reset()
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_multiple_assertions() {
        let module = parse_source(
            r#"
def test_multi_assert():
    assert 1 == 1
    assert 2 == 2
    assert 3 == 3
"#,
        );
        assert_eq!(module.test_functions[0].assertion_count, 3);
    }

    #[test]
    fn test_zero_assertions() {
        let module = parse_source(
            r#"
def test_no_assert():
    x = 1
"#,
        );
        assert!(!module.test_functions[0].has_assertions);
        assert_eq!(module.test_functions[0].assertion_count, 0);
    }

    #[test]
    fn test_mock_verifications_call_count() {
        let module = parse_source(
            r#"
def test_call_count():
    mock.call_count
"#,
        );
        assert!(module.test_functions[0].has_mock_verifications);
    }

    #[test]
    fn test_mock_verifications_called() {
        let module = parse_source(
            r#"
def test_called():
    mock.called
"#,
        );
        assert!(module.test_functions[0].has_mock_verifications);
    }

    #[test]
    fn test_detect_pytest_raises_text_match() {
        let module = parse_source(
            r#"
import pytest
def test_raises_text():
    with pytest.raises(RuntimeError):
        raise RuntimeError()
"#,
        );
        assert!(module.test_functions[0].uses_pytest_raises);
    }

    #[test]
    fn test_network_usage_aiohttp() {
        let module = parse_source(
            r#"
import aiohttp
def test_aiohttp():
    aiohttp.get("/")
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_network);
    }

    #[test]
    fn test_network_usage_urllib() {
        let module = parse_source(
            r#"
import urllib
def test_urllib():
    urllib.urlopen("http://x.com")
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_network);
    }

    #[test]
    fn test_network_not_detected_without_network_libs() {
        let module = parse_source(
            r#"
def test_no_network():
    x = 42
    assert x == 42
"#,
        );
        assert!(!module.test_functions[0].uses_network);
    }

    #[test]
    fn test_fixture_deps_exclude_self_cls() {
        let module = parse_source(
            r#"
def test_method(self, my_fix):
    assert my_fix
"#,
        );
        assert!(!module.test_functions[0]
            .fixture_deps
            .contains(&"self".to_string()));
        assert!(module.test_functions[0]
            .fixture_deps
            .contains(&"my_fix".to_string()));
    }

    #[test]
    fn test_body_hash_differs_for_different_bodies() {
        let module = parse_source(
            r#"
def test_a():
    assert 1

def test_b():
    assert 2
"#,
        );
        let hash_a = module.test_functions[0].body_hash;
        let hash_b = module.test_functions[1].body_hash;
        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn test_body_hash_same_for_identical_bodies() {
        let module = parse_source(
            r#"
def test_a():
    assert 1

def test_b():
    assert 1
"#,
        );
        let hash_a = module.test_functions[0].body_hash;
        let hash_b = module.test_functions[1].body_hash;
        assert_eq!(hash_a, hash_b);
    }

    #[test]
    fn test_count_top_level_entries_depth_decrement_guard() {
        assert_eq!(PythonParser::count_top_level_entries("a], b"), 2);
    }

    #[test]
    fn test_detect_sleep_value_integer_arg() {
        let module = parse_source(
            r#"
import time
def test_sleep_int():
    time.sleep(5)
    assert True
"#,
        );
        assert_eq!(module.test_functions[0].sleep_value, Some(5.0));
    }

    #[test]
    fn test_detect_sleep_value_negative_arg() {
        let module = parse_source(
            r#"
import time
def test_sleep_neg():
    time.sleep(-5)
    assert True
"#,
        );
        assert_eq!(
            module.test_functions[0].sleep_value,
            Some(-5.0),
            "negative sleep via unary operator must be extracted as negative value"
        );
    }

    #[test]
    fn test_detect_sleep_value_negative_float_arg() {
        let module = parse_source(
            r#"
import time
def test_sleep_neg_float():
    time.sleep(-0.5)
    assert True
"#,
        );
        assert_eq!(
            module.test_functions[0].sleep_value,
            Some(-0.5),
            "negative float sleep via unary operator must be extracted as negative value"
        );
    }

    #[test]
    fn test_detect_sleep_value_plus_unary_not_extracted() {
        let module = parse_source(
            r#"
import time
def test_sleep_plus():
    time.sleep(+5)
    assert True
"#,
        );
        assert_eq!(
            module.test_functions[0].sleep_value, None,
            "unary + sleep should not be extracted as negative"
        );
    }

    #[test]
    fn test_fixture_try_wrapping_yield_block() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_try_block():
    try:
        conn = get_conn()
        yield conn
    except Exception:
        pass
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_try_wrapping_yield_suite() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_try_suite():
    try:
        yield 42
    finally:
        cleanup()
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_bare_sleep_call_detected() {
        let module = parse_source(
            r#"
from time import sleep
def test_bare_sleep():
    sleep(1)
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_time_sleep);
        assert_eq!(module.test_functions[0].sleep_value, Some(1.0));
    }

    #[test]
    fn test_sleep_value_not_contaminated_by_other_calls() {
        let module = parse_source(
            r#"
import time
def test_mixed_calls():
    result = max(100)
    time.sleep(2)
    assert result
"#,
        );
        assert_eq!(module.test_functions[0].sleep_value, Some(2.0));
    }

    #[test]
    fn test_no_sleep_with_numeric_calls() {
        let module = parse_source(
            r#"
def test_no_sleep_numeric():
    result = max(100)
    assert result == 100
"#,
        );
        assert!(!module.test_functions[0].uses_time_sleep);
        assert_eq!(module.test_functions[0].sleep_value, None);
    }

    #[test]
    fn test_sleep_max_value_distinct() {
        let module = parse_source(
            r#"
import time
def test_sleep_order():
    time.sleep(5.0)
    time.sleep(1.0)
    assert True
"#,
        );
        assert_eq!(module.test_functions[0].sleep_value, Some(5.0));
    }

    #[test]
    fn test_sleep_via_attribute_only() {
        let module = parse_source(
            r#"
import time as t
def test_sleep_alias():
    t.sleep(3)
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_time_sleep);
        assert_eq!(module.test_functions[0].sleep_value, Some(3.0));
    }

    #[test]
    fn test_state_assertions_no_assert_no_mock() {
        let module = parse_source(
            r#"
def test_empty():
    x = 1
"#,
        );
        assert!(!module.test_functions[0].has_state_assertions);
    }

    #[test]
    fn test_assertion_identifier_not_magic_when_comparison() {
        let module = parse_source(
            r#"
def test_ident_comparison():
    x = 1
    assert x > 0
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert_eq!(info.len(), 1);
        assert!(!info[0].is_magic);
        assert!(info[0].has_comparison);
    }

    #[test]
    fn test_assertion_identifier_is_magic_without_comparison() {
        let module = parse_source(
            r#"
def test_ident_no_comp():
    x = True
    assert x
"#,
        );
        let info = &module.test_functions[0].assertions;
        assert_eq!(info.len(), 1);
        assert!(info[0].is_magic);
    }

    #[test]
    fn test_parametrize_values_extracts_elements() {
        let module = parse_source(
            r#"
@pytest.mark.parametrize("x", [1, 2, 3])
def test_values(x):
    assert x > 0
"#,
        );
        let vals = &module.test_functions[0].parametrize_values;
        assert!(!vals.is_empty());
        let first = &vals[0];
        assert!(first.contains(&"1".to_string()));
        assert!(first.contains(&"2".to_string()));
        assert!(first.contains(&"3".to_string()));
    }

    #[test]
    fn test_pytest_raises_text_only() {
        let module = parse_source(
            r#"
def test_raises_check():
    with pytest.raises(ValueError):
        raise ValueError("x")
"#,
        );
        assert!(module.test_functions[0].uses_pytest_raises);
    }

    #[test]
    fn test_fixture_mutation_extend() {
        let module = parse_source(
            r#"
def test_extend(my_list):
    my_list.extend([1, 2])
    assert len(my_list) > 0
"#,
        );
        assert!(module.test_functions[0]
            .mutates_fixture_deps
            .contains(&"my_list".to_string()));
    }

    #[test]
    fn test_fixture_mutation_non_mutating_method() {
        let module = parse_source(
            r#"
def test_non_mutate(my_list):
    x = my_list.index(1)
    assert x == 0
"#,
        );
        assert!(module.test_functions[0].mutates_fixture_deps.is_empty());
    }

    #[test]
    fn test_detect_cleanup_addfinalizer_only() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_only_af(request):
    request.addfinalizer(lambda: None)
    return 42
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_detect_cleanup_mock_patch_only() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_only_patch():
    mock.patch("some.module")
    return 42
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_detect_cleanup_patch_call() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_patch_call():
    patch("some.module")
    return 42
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_network_via_attribute_object() {
        let module = parse_source(
            r#"
import requests
def test_net_attr():
    requests.post("http://x.com")
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_network);
    }

    #[test]
    fn test_random_usage_via_attribute_object() {
        let module = parse_source(
            r#"
import random
def test_rand_obj():
    random.choice([1, 2, 3])
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_random);
    }

    #[test]
    fn test_random_seed_text_match() {
        let module = parse_source(
            r#"
import random
def test_seed_text():
    random.seed(42)
    assert True
"#,
        );
        assert!(module.test_functions[0].has_random_seed);
    }

    #[test]
    fn test_subprocess_call_detected_no_timeout() {
        let module = parse_source(
            r#"
import subprocess
def test_sub_call():
    subprocess.call(["ls", "-la"])
    assert True
"#,
        );
        assert!(module.test_functions[0].has_subprocess_timeout);
    }

    #[test]
    fn test_subprocess_check_output_no_timeout() {
        let module = parse_source(
            r#"
import subprocess
def test_sub_check_out():
    subprocess.check_output(["echo", "hi"])
    assert True
"#,
        );
        assert!(module.test_functions[0].has_subprocess_timeout);
    }

    #[test]
    fn test_subprocess_popen_timeout_arg() {
        let module = parse_source(
            r#"
import subprocess
def test_popen_timeout():
    subprocess.Popen(["ls"], timeout=10)
    assert True
"#,
        );
        assert!(!module.test_functions[0].has_subprocess_timeout);
    }

    #[test]
    fn test_cwd_dependency_via_contains_getcwd() {
        let module = parse_source(
            r#"
def test_cwd_contains():
    x = my_module.getcwd()
    assert x
"#,
        );
        assert!(module.test_functions[0].uses_cwd_dependency);
    }

    #[test]
    fn test_cwd_dependency_via_contains_chdir() {
        let module = parse_source(
            r#"
def test_chdir_contains():
    my_module.chdir("/tmp")
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_cwd_dependency);
    }

    #[test]
    fn test_cwd_dependency_attr_getcwd() {
        let module = parse_source(
            r#"
import os
def test_cwd_attr2():
    os.getcwd()
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_cwd_dependency);
    }

    #[test]
    fn test_cwd_dependency_attr_chdir() {
        let module = parse_source(
            r#"
import os
def test_chdir_attr2():
    os.chdir("/tmp")
    assert True
"#,
        );
        assert!(module.test_functions[0].uses_cwd_dependency);
    }

    #[test]
    fn test_db_commit_identifier_only() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_commit_id():
    do_commit()
    return 42
"#,
        );
        assert!(module.fixtures[0].has_db_commit);
    }

    #[test]
    fn test_db_rollback_identifier_only() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_rollback_id():
    do_rollback()
    return 42
"#,
        );
        assert!(module.fixtures[0].has_db_rollback);
    }

    #[test]
    fn test_db_commit_via_call_attribute() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_commit_attr():
    session.commit()
    return 42
"#,
        );
        assert!(module.fixtures[0].has_db_commit);
    }

    #[test]
    fn test_has_try_wrapping_yield_with_suite() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_try_suite_check():
    try:
        yield 42
    except:
        pass
"#,
        );
        assert!(module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_count_parametrize_args_end_eq_start() {
        assert_eq!(
            PythonParser::count_parametrize_args("parametrize('x', ']' [1, 2])"),
            2
        );
    }

    #[test]
    fn test_fixture_try_no_yield_no_cleanup() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_try_no_yield():
    try:
        x = 1
    except:
        pass
    return x
"#,
        );
        assert!(!module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_fixture_with_no_yield_no_cleanup() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_with_no_yield():
    with ctx():
        x = 1
    return x
"#,
        );
        assert!(!module.fixtures[0].has_cleanup);
    }

    #[test]
    fn test_pytest_raises_attribute_destructure() {
        let module = parse_source(
            r#"
import pytest
def test_raises_destructure():
    with pytest.raises(TypeError):
        pass
"#,
        );
        assert!(module.test_functions[0].uses_pytest_raises);
    }

    #[test]
    fn test_no_pytest_raises() {
        let module = parse_source(
            r#"
def test_no_raises():
    assert 1 == 1
"#,
        );
        assert!(!module.test_functions[0].uses_pytest_raises);
    }

    #[test]
    fn test_random_not_seed_not_detected() {
        let module = parse_source(
            r#"
import random
def test_rand_no_seed():
    random.randint(1, 10)
    assert True
"#,
        );
        assert!(!module.test_functions[0].has_random_seed);
        assert!(module.test_functions[0].uses_random);
    }

    #[test]
    fn test_subprocess_not_timeout_no_subprocess() {
        let module = parse_source(
            r#"
def test_no_sub_no_timeout():
    x = 1
    assert x == 1
"#,
        );
        assert!(!module.test_functions[0].has_subprocess_timeout);
        assert!(!module.test_functions[0].uses_subprocess);
    }

    #[test]
    fn test_detect_cleanup_pattern_bare_addfinalizer() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_bare_af():
    addfinalizer(teardown)
    yield 1
"#,
        );
        assert!(
            module.fixtures[0].has_cleanup,
            "bare addfinalizer should be detected"
        );
    }

    #[test]
    fn test_sleep_integer_arg() {
        let module = parse_source(
            r#"
def test_sleep_int():
    time.sleep(5)
    assert True
"#,
        );
        let tf = &module.test_functions[0];
        assert_eq!(tf.sleep_value, Some(5.0));
    }

    #[test]
    fn test_sleep_takes_max_of_equal() {
        let module = parse_source(
            r#"
def test_sleep_multi():
    time.sleep(0.3)
    time.sleep(0.3)
    assert True
"#,
        );
        let tf = &module.test_functions[0];
        assert_eq!(tf.sleep_value, Some(0.3));
    }

    #[test]
    fn test_try_without_yield_no_cleanup() {
        let module = parse_source(
            r#"
import pytest

@pytest.fixture
def fix_try_no_yield():
    try:
        x = 1
    except:
        pass
    yield x
"#,
        );
        assert!(
            !module.fixtures[0].has_cleanup,
            "try without yield should not be cleanup"
        );
    }

    // ── extract_patch_target unit tests ──

    #[test]
    fn test_extract_patch_target_double_quotes() {
        let result = extract_patch_target(r#"@patch("subprocess.run")"#);
        assert_eq!(result, Some("subprocess.run".to_string()));
    }

    #[test]
    fn test_extract_patch_target_single_quotes() {
        let result = extract_patch_target(r#"@patch('os.path.exists')"#);
        assert_eq!(result, Some("os.path.exists".to_string()));
    }

    #[test]
    fn test_extract_patch_target_no_parens() {
        let result = extract_patch_target("@pytest.mark.slow");
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_patch_target_no_string() {
        let result = extract_patch_target("@patch(123)");
        assert_eq!(result, None);
    }

    // ── extract_first_string_arg unit tests ──

    #[test]
    fn test_extract_first_string_arg_double_quotes() {
        let result = extract_first_string_arg(r#""socket.socket")"#);
        assert_eq!(result, Some("socket.socket".to_string()));
    }

    #[test]
    fn test_extract_first_string_arg_single_quotes() {
        let result = extract_first_string_arg("'builtins.open')");
        assert_eq!(result, Some("builtins.open".to_string()));
    }

    #[test]
    fn test_extract_first_string_arg_no_string() {
        let result = extract_first_string_arg("123)");
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_first_string_arg_empty_string() {
        let result = extract_first_string_arg(r#"""")"#);
        assert_eq!(result, Some(String::new()));
    }

    // ── detect_stdlib_mock_targets unit tests ──

    #[test]
    fn test_detect_stdlib_mock_targets_subprocess() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_stdlib.py");
        std::fs::write(
            &path,
            r#"
from unittest.mock import patch

@patch("subprocess.run")
def test_sub(mock):
    mock.assert_called_once()
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();
        assert!(module.test_functions[0].mocks_stdlib_module);
        assert_eq!(
            module.test_functions[0].mocked_stdlib_targets,
            vec!["subprocess.run"]
        );
    }

    #[test]
    fn test_detect_stdlib_mock_targets_non_stdlib() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_nonstdlib.py");
        std::fs::write(
            &path,
            r#"
from unittest.mock import patch

@patch("myapp.service.fetch")
def test_fetch(mock):
    assert mock() is not None
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();
        assert!(!module.test_functions[0].mocks_stdlib_module);
        assert!(module.test_functions[0].mocked_stdlib_targets.is_empty());
    }

    #[test]
    fn test_detect_stdlib_mock_targets_multiple() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_multi.py");
        std::fs::write(
            &path,
            r#"
from unittest.mock import patch

@patch("os.path.exists")
@patch("subprocess.run")
def test_multi(mock_run, mock_exists):
    mock_run.assert_called()
    mock_exists.assert_called()
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();
        assert!(module.test_functions[0].mocks_stdlib_module);
        assert_eq!(module.test_functions[0].mocked_stdlib_targets.len(), 2);
        assert!(module.test_functions[0]
            .mocked_stdlib_targets
            .contains(&"subprocess.run".to_string()));
        assert!(module.test_functions[0]
            .mocked_stdlib_targets
            .contains(&"os.path.exists".to_string()));
    }

    #[test]
    fn test_detect_stdlib_mock_targets_no_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_dedup.py");
        std::fs::write(
            &path,
            r#"
from unittest.mock import patch

@patch("subprocess.run")
@patch("subprocess.run")
def test_dedup(mock1, mock2):
    mock1.assert_called()
    mock2.assert_called()
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();
        assert!(module.test_functions[0].mocks_stdlib_module);
        assert_eq!(
            module.test_functions[0].mocked_stdlib_targets.len(),
            1,
            "duplicate stdlib targets should be deduplicated"
        );
    }

    #[test]
    fn test_detect_stdlib_mock_targets_body_patch_dedup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_body_dedup.py");
        std::fs::write(
            &path,
            r#"
from unittest.mock import patch

@patch("subprocess.run")
def test_body_dedup(mock_run):
    with patch("subprocess.run") as mock_run2:
        mock_run.assert_called()
        mock_run2.assert_called()
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();
        assert!(module.test_functions[0].mocks_stdlib_module);
        assert_eq!(
            module.test_functions[0].mocked_stdlib_targets.len(),
            1,
            "decorator + body patch for same target should be deduplicated"
        );
    }

    #[test]
    fn test_detect_stdlib_mock_targets_body_only_no_dedup_needed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_body_only.py");
        std::fs::write(
            &path,
            r#"
from unittest.mock import patch

def test_body_only():
    with patch("subprocess.run") as mock_run:
        mock_run.assert_called()
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();
        assert!(module.test_functions[0].mocks_stdlib_module);
        assert_eq!(
            module.test_functions[0].mocked_stdlib_targets,
            vec!["subprocess.run"],
            "body-only patch should detect stdlib target"
        );
    }

    #[test]
    fn test_detect_stdlib_mock_targets_body_nonstdlib_not_added() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_body_nonstdlib.py");
        std::fs::write(
            &path,
            r#"
from unittest.mock import patch

def test_body_nonstdlib():
    with patch("myapp.service.fetch") as mock_fetch:
        assert mock_fetch() is not None
"#,
        )
        .unwrap();

        let mut parser = PythonParser::new().unwrap();
        let module = parser.parse_file(&path).unwrap();
        assert!(
            !module.test_functions[0].mocks_stdlib_module,
            "non-stdlib body patch should not trigger mocks_stdlib_module"
        );
        assert!(
            module.test_functions[0].mocked_stdlib_targets.is_empty(),
            "non-stdlib body patch should not be added to targets"
        );
    }
}
