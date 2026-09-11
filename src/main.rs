fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.as_slice() {
        [] => systemd_gtk::ui::run(),
        [arg] if arg == "--version" || arg == "-V" => {
            println!("systemd-gtk {}", env!("CARGO_PKG_VERSION"))
        }
        [arg] if arg == "--help" || arg == "-h" => println!(
            "systemd-gtk — browse and manage systemd units\n\nUsage: systemd-gtk [OPTION]\n\n  -V, --version      Print version\n  -h, --help         Print help\n\nWith no options, open the application. Run as your normal user, not root."
        ),
        _ => {
            eprintln!("Unknown arguments. Run systemd-gtk --help for usage.");
            return std::process::ExitCode::FAILURE;
        }
    }
    std::process::ExitCode::SUCCESS
}
