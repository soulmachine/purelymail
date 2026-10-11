mod api;
mod config;
mod dispatch;
mod output;
mod schema;

use std::process::ExitCode;

fn main() -> ExitCode {
    let schema = schema::load();
    let matches = match dispatch::command(&schema).try_get_matches() {
        Ok(m) => m,
        Err(e) => {
            let _ = e.print();
            // Help and version go to stdout and exit 0; anything else is a usage error.
            return ExitCode::from(u8::from(e.use_stderr()));
        }
    };
    match dispatch::run(&schema, &matches) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}
