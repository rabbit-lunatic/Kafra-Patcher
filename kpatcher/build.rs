#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    // The icon path is relative to the Cargo.toml directory
    res.set_icon("resources/kpatcher.ico");
    // Embed DPI-awareness manifest (Per-Monitor V2) to prevent
    // Windows bitmap scaling on notebooks with display scaling (110%, 125%, etc.)
    res.set_manifest(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">
        PerMonitorV2, PerMonitor
      </dpiAwareness>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">
        True/PM
      </dpiAware>
    </windowsSettings>
  </application>
</assembly>"#);

    // Note: winres will automatically use the [package.metadata.winres] info from Cargo.toml
    // for version info, etc.

    if let Err(e) = res.compile() {
        println!("cargo:warning=Failed to compile resources: {}", e);
    }
}

#[cfg(not(windows))]
fn main() {}
