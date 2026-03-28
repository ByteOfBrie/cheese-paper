use std::env;

fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("resources/cheese-paper-icon.ico")
            .set("FileDescription", "cheese-paper text editor")
            .set("LegalCopyright", "License: GPLv3 or later");
        if let Err(err) = res.compile() {
            println!("Could not compile resources: {err}");
            std::process::exit(1);
        }
    }

    // If the project is built without git lfs, the icon and spellcheck files will
    // fail to be set, but we can't detect that at compile time. We explicitly fail
    // here to ensure we don't accidentally produce a release like this
    if Ok("release".to_owned()) == env::var("PROFILE")
        && include_bytes!("resources/cheese-paper-icon.png").len() < 1000
    {
        panic!("Attempted to create a release (seemingly) without git lfs")
    }
}
