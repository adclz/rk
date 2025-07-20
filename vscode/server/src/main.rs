/*
This file is part of auto-lsp.
Copyright (C) 2025 CLAUZEL Adrien

auto-lsp is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>
*/
use std::error::Error;

use fastrace::collector::{Config, ConsoleReporter};
use server::boot;

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    stderrlog::new()
        .modules([module_path!(), "server"])
        .quiet(false)
        .verbosity(4)
        .timestamp(stderrlog::Timestamp::Second)
        .init()
        .unwrap();

    //fastrace::set_reporter(ConsoleReporter, Config::default());

    boot()
}
