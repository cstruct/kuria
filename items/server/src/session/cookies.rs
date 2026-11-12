//! The session and login cookies.

use axum::http::{HeaderMap, HeaderValue, header};

/// How long a login attempt, and its cookie, lasts.
pub const LOGIN_MAX_AGE_SECONDS: i64 = 600;

/// The names and attributes of Kuria's cookies.
#[derive(Clone, Debug)]
pub struct Cookies {
    secure: bool,
}

impl Cookies {
    /// Secure cookies, with the `__Host-` prefix. Only local development over
    /// plain HTTP should pass `false`.
    #[must_use]
    pub fn new(secure: bool) -> Self {
        Self { secure }
    }

    fn name(&self, base: &str) -> String {
        if self.secure {
            format!("__Host-{base}")
        } else {
            base.to_owned()
        }
    }

    fn attributes(&self, same_site: &str) -> String {
        let secure = if self.secure { "; Secure" } else { "" };
        format!("Path=/; HttpOnly; SameSite={same_site}{secure}")
    }

    /// The name of the session cookie.
    #[must_use]
    pub fn session_name(&self) -> String {
        self.name("kuria_session")
    }

    /// The name of the login cookie.
    #[must_use]
    pub fn login_name(&self) -> String {
        self.name("kuria_login")
    }

    /// The request's session cookie.
    #[must_use]
    pub fn session(&self, headers: &HeaderMap) -> Option<String> {
        read(headers, &self.session_name())
    }

    /// The request's login cookie.
    #[must_use]
    pub fn login(&self, headers: &HeaderMap) -> Option<String> {
        read(headers, &self.login_name())
    }

    /// Sets the session cookie.
    // r[impl session.cookie]
    #[must_use]
    pub fn set_session(&self, value: &str) -> HeaderValue {
        header_value(&format!(
            "{}={value}; {}",
            self.session_name(),
            self.attributes("Strict")
        ))
    }

    /// Sets the login cookie, which must come back with the provider's
    /// redirect, a cross-site navigation, so it is `SameSite=Lax`.
    #[must_use]
    pub fn set_login(&self, state: &str) -> HeaderValue {
        header_value(&format!(
            "{}={state}; {}; Max-Age={LOGIN_MAX_AGE_SECONDS}",
            self.login_name(),
            self.attributes("Lax")
        ))
    }

    /// Clears the session cookie.
    #[must_use]
    pub fn clear_session(&self) -> HeaderValue {
        header_value(&format!(
            "{}=; {}; Max-Age=0",
            self.session_name(),
            self.attributes("Strict")
        ))
    }

    /// Clears the login cookie.
    #[must_use]
    pub fn clear_login(&self) -> HeaderValue {
        header_value(&format!(
            "{}=; {}; Max-Age=0",
            self.login_name(),
            self.attributes("Lax")
        ))
    }
}

fn header_value(value: &str) -> HeaderValue {
    HeaderValue::from_str(value).expect("cookies hold only header-safe characters")
}

/// The value of the cookie `name`, from every `Cookie` header.
fn read(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(cookie: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_str(cookie).unwrap());
        headers
    }

    // r[verify session.cookie]
    #[test]
    fn sets_a_host_only_strict_http_only_cookie() {
        let set = Cookies::new(true).set_session("abc");
        assert_eq!(
            set,
            "__Host-kuria_session=abc; Path=/; HttpOnly; SameSite=Strict; Secure"
        );
        assert!(!set.to_str().unwrap().contains("Domain"));
    }

    // r[verify session.cookie]
    #[test]
    fn drops_the_prefix_and_secure_only_when_insecure() {
        let cookies = Cookies::new(false);
        assert_eq!(
            cookies.set_session("abc"),
            "kuria_session=abc; Path=/; HttpOnly; SameSite=Strict"
        );
        assert_eq!(
            cookies.set_login("s"),
            "kuria_login=s; Path=/; HttpOnly; SameSite=Lax; Max-Age=600"
        );
    }

    #[test]
    fn reads_a_cookie_among_others() {
        let cookies = Cookies::new(true);
        let headers = headers("a=1; __Host-kuria_session=xyz; __Host-kuria_login=st");
        assert_eq!(cookies.session(&headers).as_deref(), Some("xyz"));
        assert_eq!(cookies.login(&headers).as_deref(), Some("st"));
        assert_eq!(Cookies::new(false).session(&headers), None);
    }
}
