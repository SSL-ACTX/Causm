use causm_core::{Program, SpannedStatement, Statement, TimelineBlock};
use pest::Parser;
use pest_derive::Parser;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub mod expressions;
pub mod statements;

#[derive(Parser)]
#[grammar = "causm.pest"]
pub struct CausmParser;

pub fn parse_causm(source: &str) -> anyhow::Result<Program> {
    let mut pairs = CausmParser::parse(Rule::program, source)?;
    let mut timelines = Vec::new();

    if let Some(program_pair) = pairs.next() {
        for pair in program_pair.into_inner() {
            if pair.as_rule() == Rule::timeline_block {
                timelines.push(statements::parse_timeline_block(pair));
            }
        }
    }

    Ok(Program { timelines })
}

fn expand_spanned_statements(
    stmts: Vec<SpannedStatement>,
    base_dir: Option<&Path>,
    loaded_files: &mut HashSet<String>,
) -> anyhow::Result<Vec<SpannedStatement>> {
    let mut result = Vec::new();
    for spanned in stmts {
        match spanned.stmt {
            Statement::Import { path, .. } | Statement::FromImport { path, .. } => {
                let target_path = if let Some(dir) = base_dir {
                    dir.join(&path)
                } else {
                    PathBuf::from(&path)
                };
                let path_str = target_path.to_string_lossy().to_string();
                if !loaded_files.contains(&path_str) && target_path.exists() {
                    loaded_files.insert(path_str.clone());
                    let imported_source = std::fs::read_to_string(&target_path)?;
                    let imported_prog = parse_causm(&imported_source)?;
                    for imp_tl in imported_prog.timelines {
                        let expanded = expand_spanned_statements(
                            imp_tl.statements,
                            target_path.parent(),
                            loaded_files,
                        )?;
                        for imp_spanned in expanded {
                            if let Statement::Isolate(iso) = imp_spanned.stmt {
                                result.extend(iso.body);
                            } else {
                                result.push(imp_spanned);
                            }
                        }
                    }
                }
            }
            Statement::Isolate(mut iso) => {
                iso.body =
                    expand_spanned_statements(iso.body, base_dir, loaded_files)?;
                result.push(SpannedStatement {
                    stmt: Statement::Isolate(iso),
                    span: spanned.span,
                });
            }
            _ => result.push(spanned),
        }
    }
    Ok(result)
}

pub fn parse_causm_with_imports(
    source: &str,
    base_dir: Option<&Path>,
) -> anyhow::Result<Program> {
    let main_program = parse_causm(source)?;
    let mut loaded_files = HashSet::new();
    let mut new_timelines = Vec::new();

    for timeline in main_program.timelines {
        let expanded_statements = expand_spanned_statements(
            timeline.statements,
            base_dir,
            &mut loaded_files,
        )?;
        new_timelines.push(TimelineBlock {
            time: timeline.time,
            no_z3: timeline.no_z3,
            entropy_mode: timeline.entropy_mode,
            statements: expanded_statements,
        });
    }

    Ok(Program {
        timelines: new_timelines,
    })
}
