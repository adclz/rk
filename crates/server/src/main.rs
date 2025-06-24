use std::error::Error;

use server::boot;

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    stderrlog::new()
        .modules([module_path!(), "server", "db"])
        .quiet(false)
        .verbosity(4)
        .timestamp(stderrlog::Timestamp::Second)
        .init()
        .unwrap();

    //fastrace::set_reporter(ConsoleReporter, Config::default());

    boot()
}