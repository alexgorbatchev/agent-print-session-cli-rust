use cli_command_kit::Mode;
use std::io::Write;
fn main() -> std::process::ExitCode {
    let mode = Mode::from_env();
    let mut out = std::io::stdout().lock();
    let result = agent_print_session_cli::cli::run(std::env::args_os(), mode, &mut out)
        .and_then(|()| out.flush().map_err(Into::into));
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            if let Err(write_error) =
                agent_print_session_cli::cli::report(&error, mode, &mut std::io::stderr().lock())
            {
                let _ = writeln!(std::io::stderr(), "error reporting failed: {write_error}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}
