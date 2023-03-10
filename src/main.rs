/*
Intuition: The more resource constrained worried approach would be to pause every branch once an unresolved symbol is hit,
build up a dependency graph and then resolve things in order. This could also be run sync.

The async solution has that kind of implicitly where I just run of and try to resolve all the rules where rules halt and await
when they hit a symbol that can't be resolved. They would then continue whenever that given symbol has been resolved.
*/

use std::{collections::HashMap, fs};

use crate::tree_sitter_cli::{parse_grammar::parse_grammar, rules::Rule};

mod tree_sitter_cli;

type SymbolResolutions = HashMap<String, String>;

fn quote(str: &str) -> String {
    format!("\"{str}\"")
}

fn resolve_rule(
    pattern_matches: &HashMap<String, String>,
    symbol_resolutions: &mut SymbolResolutions,
    rule: &Rule,
) -> Option<String> {
    match rule {
        Rule::Blank => Some(String::new()),
        Rule::String(s) => Some(s.clone()),
        Rule::Pattern(s) => {
            if let Some(str) = pattern_matches.get(&s.clone()) {
                return Some(str.clone());
            }
            println!("⛔️️ Missing pattern {}", quote(&s));
            None
        }
        Rule::Metadata {
            params: _params,
            rule: _rule,
        } => None,

        Rule::Repeat(rule) => resolve_rule(pattern_matches, symbol_resolutions, rule),
        Rule::Seq(rules) => {
            let mut strings = vec![];
            for rule in rules {
                if let Some(resolution) = resolve_rule(pattern_matches, symbol_resolutions, rule) {
                    strings.push(resolution);
                } else {
                    return None;
                }
            }

            Some(strings.join(""))
        }
        Rule::Choice(rules) => rules
            .into_iter()
            .map(|rule| resolve_rule(pattern_matches, symbol_resolutions, rule))
            .filter_map(|r| r)
            .min(),

        Rule::NamedSymbol(name) => symbol_resolutions.get(name).map(|s| s.clone()),
        Rule::Symbol(sym) => {
            // TODO
            dbg!(sym);
            None
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let grammar_str = fs::read_to_string("../tree-sitter-typescript/typescript/src/grammar.json")?;
    let grammar = parse_grammar(&grammar_str)?;

    let pattern_matches: HashMap<String, String> = HashMap::from([
        // JSON
        (r#"[^\\"\n]+"#.into(), "".into()),
        (r#"(\"|\\|\/|b|f|n|r|t|u)"#.into(), "\"".into()),
        (r#"[^*]*\*+([^/*][^*]*\*+)*"#.into(), "*".into()),
        (".*".into(), "".into()),
        ("[0-7]+".into(), "0".into()),
        (r#"\d+"#.into(), "0".into()),
        (r#"[\da-fA-F]+"#.into(), "0".into()),
        ("[1-9]".into(), "1".into()),
        ("[0-1]+".into(), "0".into()),
        // TypeScript
        (
            r#"[^\x00-\x1F\s\p{Zs}0-9:;`"'@#.,|^&<=>+\-*/\\%?!~()\[\]{}\uFEFF\u2060\u200B]|\\u[0-9a-fA-F]{4}|\\u\{[0-9a-fA-F]+\}"#.into(),
            "T".into()
        ),
        (r#"[a-zA-Z_$][a-zA-Z\d_$]*-[a-zA-Z\d_$\-]*"#.into(), "A-".into()),
        (".{1,}".into(), "T".into()),
        ("[^{}<>]+".into(), "T".into()),
        ("#!.*".into(), "#!".into())
    ]);

    let mut symbol_resolutions: SymbolResolutions = Default::default();

    for _n in 0..grammar.variables.len() {
        for var in &grammar.variables {
            if let Some(s) = resolve_rule(&pattern_matches, &mut symbol_resolutions, &var.rule) {
                symbol_resolutions.insert(var.name.clone(), s);
            }
        }
    }

    dbg!(symbol_resolutions);

    Ok(())
}
