use anyhow::Result;

use crate::app_state::AppState;

pub struct Eval {}

enum Word<'a> {
    Word(&'a str),
    Number(i64),
}

impl<'a> std::fmt::Display for Word<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Word::Word(word) => word.fmt(f),
            Word::Number(number) => number.fmt(f),
        }
    }
}

impl Eval {
    pub fn eval(buffer: &str, state: &mut AppState) -> Result<()> {
        let mut stack = vec![];
        for word in buffer.split_whitespace() {
            if let Ok(num) = word.parse::<i64>() {
                stack.push(Word::Number(num));
            } else {
                stack.push(Word::Word(word));
            }
        }
        if let Some(Word::Word(word)) = stack.pop() {
            match word {
                "label" => {
                    if let Some(word) = stack.pop() {
                        state.label_set(word.to_string())?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
