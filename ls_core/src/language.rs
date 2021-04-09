//! Support for intelligent parsing / understanding of source code

use tree_sitter;
use tree_sitter_rust;
use tree_sitter_cpp;
use tree_sitter_java;
use tree_sitter_javascript;
use tree_sitter_python;
use tree_sitter_typescript;
use tree_sitter_bash;
use lazy_static::lazy_static;

lazy_static! {
    static ref LANGUAGES: [(&'static str, tree_sitter::Language); 8] = {[
        ("rs", tree_sitter_rust::language()),
        ("cpp", tree_sitter_cpp::language()),
        ("java", tree_sitter_java::language()),
        ("js", tree_sitter_javascript::language()),
        ("py", tree_sitter_python::language()),
        ("ts", tree_sitter_typescript::language_typescript()),
        ("tsx", tree_sitter_typescript::language_tsx()),
        ("sh", tree_sitter_bash::language())
    ]};
}

pub fn get_parser(lang_str: &str) -> Option<tree_sitter::Parser> {
    for (name, lang) in LANGUAGES.iter() {
        if name == &lang_str {
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(*lang).ok()?;
            return Some(parser);
        }
    }

    None
}

fn pp_rec(node: &tree_sitter::Node, out: String, depth: i32) -> String {
    let mut result = out;

    for _ in 0..=depth {
        result += "  ";
    }

    result += node.kind();
    
    let range = node.range();
    result += &format!(" ({}.{} - {}.{})", 
        range.start_point.row, range.start_point.column,
        range.end_point.row, range.end_point.column
    );

    result += "\n";
    for i in 0..node.child_count() {
        result = pp_rec(&node.child(i).unwrap(), result, depth + 1);
    }

    result
}

pub fn pretty_print(node: &tree_sitter::Node) -> String {
    pp_rec(node, String::new(), 0i32)
}