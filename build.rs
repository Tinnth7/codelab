// Windows only: put the app icon into Codelab.exe (shows in Explorer, taskbar, installer shortcuts).
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().expect("failed to embed the Windows icon");
    }
}
