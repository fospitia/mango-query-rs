use serde_json::Value;

pub trait FlavourCompiler<TOutput, TConfig = ()> {
    /// Compile a Mango Query or Selector JSON value into target output format.
    fn compile(&self, query: &Value, config: Option<TConfig>) -> Result<TOutput, String>;
}
