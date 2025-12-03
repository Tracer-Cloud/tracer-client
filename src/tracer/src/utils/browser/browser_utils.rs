use colored::Colorize;
use std::process::{Child, Command};

///This function allows the user to open a browser window given an url from the command line.
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let mut child = Command::new("open").arg(url).spawn().unwrap();
        open_browser(&mut child, url);
    }

    #[cfg(target_os = "linux")]
    {
        let mut child = Command::new("xdg-open").arg(url).spawn().unwrap();
        crate::utils::browser::browser_utils::open_browser(&mut child, url);
    }

    #[cfg(target_os = "windows")]
    {
        let mut child = Command::new("cmd")
            .args(&["/C", "start", url])
            .spawn()
            .unwrap();
        crate::utils::browser::browser_utils::open_browser(&mut child, url);
    }
}

fn open_browser(child: &mut Child, url: &str) {
    let wait_object = child.wait();
    if wait_object.is_err() {
        eprintln!("Failed to open browser window, please:");
        eprintln!("1. Login inside {}.", url.cyan());
        eprintln!("2. Select the environment you want to use.");
    }
}
