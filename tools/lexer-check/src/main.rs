//! Reads source file paths on stdin (one language per run), parses each with
//! tree-sitter and with the plugin's lexer, and reports how many of
//! tree-sitter's comments the lexer found (recall) and how many of the
//! lexer's were real (precision), by starting line, with sample mismatches.
//! Files tree-sitter itself fails to parse are skipped.

use std::collections::BTreeSet;
use std::io::BufRead;
use tree_sitter::{Language, Node, Parser};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum L {
    Rust,
    C,
    Cpp,
    Go,
    Java,
    Kotlin,
    Scala,
    Swift,
    CSharp,
    Js,
    Ts,
    Tsx,
    Python,
    Ruby,
    Php,
}
use L::*;

fn lang_of(path: &str) -> Option<L> {
    let ext = path.rsplit('.').next()?;
    Some(match ext {
        "rs" => Rust,
        "c" | "h" => C,
        "cc" | "cpp" | "hpp" | "cxx" | "hh" => Cpp,
        "go" => Go,
        "java" => Java,
        "kt" | "kts" => Kotlin,
        "scala" | "sc" => Scala,
        "swift" => Swift,
        "cs" => CSharp,
        "js" | "mjs" | "cjs" | "jsx" => Js,
        "ts" | "mts" | "cts" => Ts,
        "tsx" => Tsx,
        "py" | "pyi" => Python,
        "rb" | "rake" => Ruby,
        "php" => Php,
        _ => return None,
    })
}

fn ts_lang(l: L) -> Language {
    match l {
        Rust => tree_sitter_rust::LANGUAGE.into(),
        C => tree_sitter_c::LANGUAGE.into(),
        Cpp => tree_sitter_cpp::LANGUAGE.into(),
        Go => tree_sitter_go::LANGUAGE.into(),
        Java => tree_sitter_java::LANGUAGE.into(),
        Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
        Scala => tree_sitter_scala::LANGUAGE.into(),
        Swift => tree_sitter_swift::LANGUAGE.into(),
        CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
        Js => tree_sitter_javascript::LANGUAGE.into(),
        Ts => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Python => tree_sitter_python::LANGUAGE.into(),
        Ruby => tree_sitter_ruby::LANGUAGE.into(),
        Php => tree_sitter_php::LANGUAGE_PHP.into(),
    }
}

#[path = "../../../src/lexer.rs"]
#[allow(dead_code)]
mod lexer;

fn lex(src: &[u8], path: &str) -> (Vec<usize>, Vec<usize>) {
    let text = String::from_utf8_lossy(src);
    let (mut c, mut d) = (vec![], vec![]);
    for x in lexer::comments(&text, path) {
        if x.kind == lexer::Kind::DocString {
            d.push(x.start)
        } else {
            c.push(x.start)
        }
    }
    (c, d)
}

// ---------------------------------------------------------------- compare

fn walk(n: Node, l: L, comments: &mut BTreeSet<usize>, docs: &mut BTreeSet<usize>) {
    let k = n.kind();
    if k.contains("comment") && !k.contains("marker") {
        comments.insert(n.start_position().row);
    }
    if l == Python && k == "expression_statement" && n.named_child_count() == 1 {
        let ch = n.named_child(0).unwrap();
        if ch.kind() == "string" || ch.kind() == "concatenated_string" {
            docs.insert(n.start_position().row);
        }
    }
    let mut cur = n.walk();
    for ch in n.children(&mut cur) {
        walk(ch, l, comments, docs);
    }
}

fn rows(src: &[u8], offs: &[usize]) -> BTreeSet<usize> {
    let mut starts = vec![0];
    for (i, &b) in src.iter().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    offs.iter()
        .map(|&o| starts.partition_point(|&s| s <= o) - 1)
        .collect()
}

#[derive(Default)]
struct Stat {
    files: usize,
    skipped: usize,
    ts: usize,
    lx: usize,
    hit: usize,
    dts: usize,
    dlx: usize,
    dhit: usize,
    miss_ex: Vec<String>,
    extra_ex: Vec<String>,
}

fn main() {
    let mut stat = Stat::default();
    let mut lang_name = String::new();
    for line in std::io::stdin().lock().lines() {
        let path = line.unwrap();
        let Some(l) = lang_of(&path) else { continue };
        lang_name = format!("{l:?}");
        let Ok(src) = std::fs::read(&path) else {
            continue;
        };
        if src.len() > 400_000 {
            continue;
        }
        let mut p = Parser::new();
        p.set_language(&ts_lang(l)).unwrap();
        let Some(tree) = p.parse(&src, None) else {
            continue;
        };
        if tree.root_node().has_error() {
            stat.skipped += 1;
            continue;
        }
        stat.files += 1;
        let (mut tc, mut td) = (BTreeSet::new(), BTreeSet::new());
        walk(tree.root_node(), l, &mut tc, &mut td);
        if std::str::from_utf8(&src).is_err() {
            continue;
        }
        let (lc, ld) = lex(&src, &path);
        let (lc, ld) = (rows(&src, &lc), rows(&src, &ld));
        let text: Vec<&[u8]> = src.split(|&b| b == b'\n').collect();
        let show = |r: usize| {
            format!(
                "{path}:{}: {}",
                r + 1,
                String::from_utf8_lossy(text.get(r).copied().unwrap_or(b""))
                    .trim()
                    .chars()
                    .take(90)
                    .collect::<String>()
            )
        };
        stat.ts += tc.len();
        stat.lx += lc.len();
        stat.hit += tc.intersection(&lc).count();
        for r in tc.difference(&lc) {
            if stat.miss_ex.len() < 6 {
                stat.miss_ex.push(show(*r));
            }
        }
        for r in lc.difference(&tc) {
            if stat.extra_ex.len() < 6 {
                stat.extra_ex.push(show(*r));
            }
        }
        stat.dts += td.len();
        stat.dlx += ld.len();
        stat.dhit += td.intersection(&ld).count();
    }
    let pct = |a: usize, b: usize| {
        if b == 0 {
            100.0
        } else {
            100.0 * a as f64 / b as f64
        }
    };
    println!(
        "{:8} files={:4} (skipped {:3} with parse errors)  comments: tree-sitter={:6} lexer={:6}  recall={:6.2}%  precision={:6.2}%",
        lang_name,
        stat.files,
        stat.skipped,
        stat.ts,
        stat.lx,
        pct(stat.hit, stat.ts),
        pct(stat.hit, stat.lx)
    );
    if stat.dts > 0 {
        println!(
            "         docstrings: tree-sitter={} lexer={} recall={:.2}% precision={:.2}%",
            stat.dts,
            stat.dlx,
            pct(stat.dhit, stat.dts),
            pct(stat.dhit, stat.dlx)
        );
    }
    for m in &stat.miss_ex {
        println!("   MISS  {m}");
    }
    for m in &stat.extra_ex {
        println!("   EXTRA {m}");
    }
}
