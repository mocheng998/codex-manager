fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set("FileDescription", "Codex Manager Launcher");
        res.set("ProductName", "Codex Manager");
        let _ = res.compile();
    }
}
