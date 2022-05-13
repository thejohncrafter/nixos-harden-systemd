
use std::fs;
use std::error::Error;

use serde::Serialize;

use serde_json;

use rnix::types::*;
use rnix::SyntaxNode;

use crate::walkers::*;

fn find_test_file(n: SyntaxNode) -> Result<String, Box<dyn Error>> {
    let app_ext = Apply::cast(n).ok_or("unexpected file structure")?;
    let app = Apply::cast(app_ext.lambda().ok_or("parse error")?).ok_or("unexpected file structure")?;

    if let Ok(f) = parse_ident_select(app.lambda().ok_or("parse error")?) {
        if f == [ "handleTest" ] {
            expect_relative_path(app.value().ok_or("parse error")?)
        } else {
            Err("unexpected file structure")?
        }
    } else {
        let app_inner = Apply::cast(app.lambda().ok_or("parse error")?).ok_or("unexpected file structure")?;
        let f = parse_ident_select(app_inner.lambda().ok_or("parse error")?)?;
        if f == [ "handleTestOn" ] {
            expect_relative_path(app.value().ok_or("parse error")?)
        } else {
            Err("unexpected file structure")?
        }
    }
}

#[derive(Serialize)]
struct Output {
    paths: Vec<(String, String)>,
    failures: Vec<String>,
}

pub fn find_all_tests(all_tests: &str) -> Result<(), Box<dyn Error>> {
    let content = fs::read_to_string(&all_tests)?;
    let ast = rnix::parse(&content).as_result()?;

    let root_fn = ast.root().inner().and_then(Lambda::cast).ok_or("root isn't a function")?;
    let val = go_right_value(root_fn.body().ok_or("parse error")?)?;
    let attrs = AttrSet::cast(val).ok_or("unexpected file structure")?;

    let nodes = attrs.entries()
        .map(|entry| -> Result<_, Box<dyn Error>> {
            Ok((entry.key().ok_or("parse error")?,
            entry.value().ok_or("parse error")?)) } )
        .collect::<Result<Vec<(_, SyntaxNode)>, Box<dyn Error>>>()?;

    let mut paths: Vec<(String, String)> = vec!();
    let mut failures: Vec<String> = vec!();

    for (k, v) in nodes.into_iter() {
        if let Ok(f) = find_test_file(v) {
            paths.push((format!("{}", k.node()), f));
        } else {
            failures.push(format!("{}", k.node()));
        }
    }

    println!("{}", serde_json::to_string(&Output { paths, failures })?);

    Ok(())
}

