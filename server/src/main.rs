fn main() {
    let _ = database::run().inspect_err( |err| {
        eprintln!("Error handling stream {err}") });
}
