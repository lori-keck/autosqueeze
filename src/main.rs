fn main() {
    eprintln!("Usage:");
    eprintln!("  cargo run --release --bin compress < input > output");
    eprintln!("  cargo run --release --bin compress -- -d < compressed > output");
    eprintln!("  cargo run --release --bin benchmark");
}
