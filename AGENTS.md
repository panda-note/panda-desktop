.rules

## Desktop debug build output

For desktop debug builds, always set the following environment variables:

```powershell
$env:CARGO_HOME = 'D:\ch'
$env:CARGO_TARGET_DIR = 'D:\cargo-target\panda-desktop'
```

Build the desktop application with `cargo build -p panda`. The required output is
`D:\cargo-target\panda-desktop\debug\panda.exe`.
