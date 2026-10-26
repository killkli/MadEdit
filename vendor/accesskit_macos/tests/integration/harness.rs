// Copyright 2026 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file) or the MIT license (found in
// the LICENSE-MIT file), at your option.

use std::process::ExitCode;

pub struct Test {
    pub name: &'static str,
    pub run: fn(),
}

macro_rules! tests {
    ($($name:ident),* $(,)?) => {
        &[$($crate::harness::Test { name: stringify!($name), run: $name }),*]
    };
}
pub(crate) use tests;

// 回補版直接依序執行上游測試，確保 AppKit 操作都在主執行緒，且不增加測試框架相依。
pub fn run(suites: &[(&str, &[Test])]) -> ExitCode {
    for (suite, tests) in suites {
        for test in *tests {
            print!("{suite}::{} ... ", test.name);
            (test.run)();
            println!("ok");
        }
    }
    ExitCode::SUCCESS
}
