use std::process::Command;

///This function allows the user to open a browser window given an url from the command line.
pub fn open_url(url: &str) {

    #[cfg(target_os = "macos")]
    Command::new("open")
        .arg(url)
        .spawn()
        .unwrap();

    #[cfg(target_os = "linux")]
    Command::new("xdg-open")
        .arg(url)
        .spawn()
        .unwrap();

    #[cfg(target_os = "windows")]
    Command::new("cmd")
        .args(&["/C", "start", url])
        .spawn()
        .unwrap();
}