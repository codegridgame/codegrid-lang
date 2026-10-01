fn main() {
    match codegrid_lsp::run_stdio() {
        Ok(true) => {}
        Ok(false) => {
            eprintln!(
                "codegrid-lsp: [{error_number}] [lsp.abnormal_exit] exit before shutdown",
                error_number = codegrid_model::error_number("lsp", "lsp.abnormal_exit")
                    .expect("Registered process error")
            );
            std::process::exit(1)
        }
        Err(error) => {
            eprintln!(
                "codegrid-lsp: [{error_number}] [lsp.transport_io] {error}",
                error_number = codegrid_model::error_number("lsp", "lsp.transport_io")
                    .expect("Registered process error")
            );
            std::process::exit(1);
        }
    }
}
