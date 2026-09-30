fn main() {
    match codegrid_lsp::run_stdio() {
        Ok(true) => {}
        Ok(false) => {
            eprintln!("codegrid-lsp: [lsp.abnormal_exit] exit before shutdown");
            std::process::exit(1)
        }
        Err(error) => {
            eprintln!("codegrid-lsp: [lsp.transport_io] {error}");
            std::process::exit(1);
        }
    }
}
