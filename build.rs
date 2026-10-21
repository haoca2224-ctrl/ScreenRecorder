fn main() {
    #[cfg(windows)]
    {
        let mut resource = winres::WindowsResource::new();
        resource.set_icon("assets/screen-recorder.ico");
        resource.set("ProductName", "Screen Recorder Studio");
        resource.set(
            "FileDescription",
            "Screen recording and clip editing studio",
        );
        resource.set("LegalCopyright", "MIT License");
        resource
            .compile()
            .expect("compile Windows application resources");
    }
}
