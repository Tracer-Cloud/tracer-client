use std::process::Command;

///This function allows the user to open a browser window given an url from the command line.
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let mut child = Command::new("open").arg(url).spawn().unwrap();
        child.wait().unwrap();
    }

    #[cfg(target_os = "linux")]
    {
        let mut child = Command::new("xdg-open").arg(url).spawn().unwrap();
        child.wait().unwrap();
    }

    #[cfg(target_os = "windows")]
    {
        let mut child = Command::new("cmd")
            .args(&["/C", "start", url])
            .spawn()
            .unwrap();
        child.wait().unwrap();
    }
}
