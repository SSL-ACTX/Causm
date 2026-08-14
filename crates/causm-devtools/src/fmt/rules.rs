// crates/causm-devtools/src/fmt/rules.rs

#[derive(Debug, Clone)]
pub struct FormatConfig {
    pub indent_spaces: usize,
    pub max_width: usize,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            indent_spaces: 4,
            max_width: 100,
        }
    }
}
