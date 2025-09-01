use crate::utils::browser::browser;

pub fn login() -> Result<String, Box<dyn std::error::Error>> {
    let app_url = "http://localhost:3000";
    let login_url = format!("{}/api/auth/cli-login", app_url);

    // open the browser window to login
    browser::open_url(&login_url);

    Ok("Success".into())
}