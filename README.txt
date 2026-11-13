Pyxee release-pipeline fixes (based on main at d38c00e). Nothing was pushed.

1. Unzip over the repository root (paths are repo-relative).
2. Delete what REMOVED_FILES.txt lists. In short:
     git rm -r crates/app ui scripts/perf docs/measuring.md packaging/arch
3. Run: cargo fmt --all -- --check ; cargo test -p xtask --locked ;
        python -m unittest discover -s scripts/tests

Verified here (Linux): xtask builds, 45 tests, fmt and clippy clean; 44 Python tests pass; installer.nsi compiles with makensis;
both workflow files parse. NOT run (no Windows/Flutter/PowerShell here): the workflows, `cargo xtask installer|portable|smoke`,
check-windows.ps1 and test-build.ps1.
