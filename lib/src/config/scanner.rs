pub mod sensitive;

#[derive(Debug, Clone, PartialEq)]
pub struct ScannerConfig {
    pub sensitive: sensitive::SensitiveScannerConfig,
}
