pub mod scanner;

#[derive(Debug, Clone, PartialEq)]
pub struct BofaConfig {
    pub scanner: scanner::ScannerConfig,
}
