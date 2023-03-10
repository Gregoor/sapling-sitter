/*
Intuition: The more resource constrained worried approach would be to pause every branch once an unresolved symbol is hit,
build up a dependency graph and then resolve things in order. This could also be run sync.

The async solution has that kind of implicitly where I just run of and try to resolve all the rules where rules halt and await
when they hit a symbol that can't be resolved. They would then continue whenever that given symbol has been resolved.
*/

use std::{
    collections::{hash_map::Entry, HashMap},
    fs,
    sync::Arc,
};

use async_recursion::async_recursion;
use tokio::{sync::RwLock, task::JoinSet};

use crate::{
    eventually::Eventually,
    tree_sitter_cli::{parse_grammar::parse_grammar, rules::Rule},
};

mod eventually;
mod tree_sitter_cli;

type SymbolResolutions = RwLock<HashMap<String, Eventually<Option<String>>>>;

fn quote(str: &str) -> String {
    format!("\"{str}\"")
}

#[async_recursion]
async fn resolve_rule(
    pattern_matches: HashMap<String, String>,
    symbol_resolutions: Arc<SymbolResolutions>,
    rule: Rule,
) -> Option<String> {
    match rule {
        Rule::Blank => Some(String::new()),
        Rule::String(s) => Some(s),
        Rule::Pattern(s) => {
            if let Some(str) = pattern_matches.get(&s.clone()) {
                return Some(str.clone());
            }
            println!("⛔️️ Missing pattern {}", quote(&s));
            None
        }
        Rule::Metadata {
            params: _params,
            rule,
        } => resolve_rule(pattern_matches, symbol_resolutions, *rule).await,

        Rule::Repeat(rule) => resolve_rule(pattern_matches, symbol_resolutions, *rule).await,
        Rule::Seq(rules) => {
            let mut strings = vec![];
            for rule in rules {
                if let Some(resolution) =
                    resolve_rule(pattern_matches.clone(), symbol_resolutions.clone(), rule).await
                {
                    strings.push(resolution);
                } else {
                    return None;
                }
            }

            Some(strings.join(""))
        }
        Rule::Choice(rules) => {
            let mut join_set = JoinSet::new();

            for rule in rules {
                let symbol_resolutions = symbol_resolutions.clone();
                let pattern_matches = pattern_matches.clone();
                join_set.spawn(async move {
                    resolve_rule(pattern_matches, symbol_resolutions, rule).await
                });
            }

            while let Some(Ok(Some(s))) = join_set.join_next().await {
                return Some(s);
            }
            None
        }

        Rule::NamedSymbol(name) => {
            println!("⏳ Waiting for {} to resolve", quote(&name));
            {
                let mut symbol_resolution = symbol_resolutions.write().await;
                symbol_resolution
                    .entry(name.clone())
                    .or_insert(Eventually::new());
                // drop the write lock
            }
            let entry = symbol_resolutions.read().await;
            let str = entry.get(&name).unwrap().read().await;
            println!("🎉 Resolved {}", quote(&name));
            return str;
        }
        Rule::Symbol(sym) => {
            // TODO
            dbg!(sym);
            None
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // "../tree-sitter-typescript/typescript/src/grammar.json"
    let grammar_str = fs::read_to_string("../tree-sitter-json/src/grammar.json")?;
    let grammar = parse_grammar(&grammar_str)?;

    let pattern_matches: HashMap<String, String> = HashMap::from([
        (r#"[^\\"\n]+"#.into(), "".into()),
        (r#"(\"|\\|\/|b|f|n|r|t|u)"#.into(), "\"".into()),
        (r#"[^*]*\*+([^/*][^*]*\*+)*"#.into(), "*".into()),
        (".*".into(), "".into()),
        ("[0-7]+".into(), "0".into()),
        (r#"\d+"#.into(), "0".into()),
        (r#"[\da-fA-F]+"#.into(), "0".into()),
    ]);

    let symbol_resolutions: Arc<SymbolResolutions> = Default::default();

    let mut join_set = JoinSet::new();
    println!(
        "ℹ️ Resolving {} vars: {}",
        grammar.variables.len(),
        grammar
            .variables
            .iter()
            .map(|v| v.name.clone())
            .collect::<Vec<String>>()
            .join(", ")
    );
    for var in grammar.variables {
        let sr = symbol_resolutions.clone();
        let pattern_matches = pattern_matches.clone();
        join_set.spawn(async move {
            println!("🧮 Resolving {}", quote(&var.name));
            let resolution = resolve_rule(pattern_matches, sr.clone(), var.rule).await;
            if let Some(s) = resolution.clone() {
                println!("✅ Resolved {} to {:?}", quote(&var.name), s);
            } else {
                println!("🛑 Could not resolve {}", quote(&var.name));
            }

            match sr.write().await.entry(var.name) {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().write(resolution).await;
                }
                Entry::Vacant(entry) => {
                    let eventually = Eventually::new();
                    eventually.write(resolution).await;
                    entry.insert(eventually);
                }
            };
        });
    }

    loop {
        dbg!(join_set.join_next().await);
    }

    dbg!(symbol_resolutions);

    Ok(())
}
