use crate::app_state::AppState;

use anyhow::Result;

pub struct Eval {}

impl Eval {
    pub fn eval(buffer: &str, state: &mut AppState) -> Result<()> {
        let mut stack = buffer.split_whitespace().collect::<Vec<_>>();
        if let Some(word) = stack.pop() {
            match word {
                "bpm" => {
                    if let Some(Ok(value)) = stack.pop().map(|x| x.parse::<f64>()) {
                        state.bpm_set(value)?;
                    }
                }
                "call" | "c" => {
                    if let Some(label) = stack.pop() {
                        state.eval_call(label.to_string())?;
                    }
                }
                "label" | "l" => {
                    if let Some(label) = stack.pop() {
                        state.eval_label(label.to_string())?;
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
