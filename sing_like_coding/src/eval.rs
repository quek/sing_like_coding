use anyhow::Result;

use crate::app_state::AppState;

pub struct Eval {}

#[derive(Debug)]
enum Word<'a> {
    Word(&'a str),
    Integer(i64),
    Float(f64),
}

impl<'a> std::fmt::Display for Word<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Word::Word(word) => word.fmt(f),
            Word::Integer(value) => value.fmt(f),
            Word::Float(value) => value.fmt(f),
        }
    }
}

impl Eval {
    pub fn eval(buffer: &str, state: &mut AppState) -> Result<()> {
        let mut stack = vec![];
        for word in buffer.split_whitespace() {
            if let Ok(num) = word.parse::<i64>() {
                stack.push(Word::Integer(num));
            } else if let Ok(num) = word.parse::<f64>() {
                stack.push(Word::Float(num));
            } else {
                stack.push(Word::Word(word));
            }
        }
        if let Some(Word::Word(word)) = stack.pop() {
            match word {
                "bpm" => match stack.pop() {
                    Some(Word::Integer(value)) => state.bpm_set(value as f64)?,
                    Some(Word::Float(value)) => state.bpm_set(value)?,
                    _ => (),
                },
                "call" | "c" => {
                    if let Some(word) = stack.pop() {
                        state.eval_call(word.to_string())?;
                    }
                }
                "label" | "l" => {
                    if let Some(word) = stack.pop() {
                        state.eval_label(word.to_string())?;
                    }
                }
                "ret" | "r" => {
                    state.eval_ret()?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}
