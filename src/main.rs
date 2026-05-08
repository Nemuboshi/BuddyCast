fn main() {
    if let Err(error) = buddy_cast::run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
