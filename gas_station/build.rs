pub fn main() {
    #[cfg(feature = "debug")]
    println!("cargo::warning=DEBUG MODE IS ENABLED. Do not deploy on mainnet.");
}
