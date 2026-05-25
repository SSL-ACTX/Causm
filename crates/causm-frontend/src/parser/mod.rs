use causm_core::Program;
use pest::Parser;
use pest_derive::Parser;

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
