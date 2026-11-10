fn main() {
    if let Err(error) = {{crate_name}}_desktop::run() {
        eprintln!("application error: {error}");
        std::process::exit(1);
    }
}
