//! Web address of a repository's `origin`, so the project menu can open the hosting page (pull
//! requests, workflows, issues) in a browser tab whatever the provider is.

/// Return the browsable `https://` address of `cwd`'s `origin`, or None when there is no origin or
/// it is not a network remote.
pub fn repo_web_url(cwd: &str) -> Option<String> {
    super::remote_origin_url(cwd).and_then(|origin| web_url(&origin))
}

/// Convert a Git remote URL to the address of its web page. HTTP(S) keeps its scheme and port;
/// SSH and `git://` remotes map to `https://` on the same host, without the SSH port. Credentials
/// are dropped so they never reach an address bar. Local paths and `file://` return None.
pub fn web_url(remote: &str) -> Option<String> {
    let u = remote.trim().trim_end_matches('/');
    let u = u.strip_suffix(".git").unwrap_or(u);
    let (scheme, host, path) = if let Some(i) = u.find("://") {
        let scheme = u[..i].to_ascii_lowercase();
        let rest = &u[i + 3..];
        let (authority, path) = rest.split_once('/')?;
        // Userinfo ends at the last `@` of the authority; it may hold a token.
        let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        match scheme.as_str() {
            "http" | "https" => (scheme.clone(), host_port.to_string(), path),
            "ssh" | "git" | "git+ssh" | "ssh+git" => {
                let host = host_port.split(':').next().unwrap_or_default();
                ("https".to_string(), host.to_string(), path)
            }
            _ => return None,
        }
    } else {
        // SCP-style `[user@]host:path`. A slash before the colon means a local path instead.
        let (authority, path) = u.split_once(':')?;
        if authority.contains('/') || authority.contains('\\') {
            return None;
        }
        let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        ("https".to_string(), host.to_string(), path)
    };
    let host = host.to_ascii_lowercase();
    let path = path.trim_matches('/');
    // A one-letter host is a Windows drive (`C:\repo`), not a server.
    if host.len() < 2 || path.is_empty() {
        return None;
    }
    // Azure DevOps SSH remotes are `ssh.dev.azure.com:v3/org/project/repo`; the web page is
    // `dev.azure.com/org/project/_git/repo`.
    if host == "ssh.dev.azure.com" {
        let parts: Vec<&str> = path.split('/').collect();
        if let ["v3", org, project, repo] = parts.as_slice() {
            return Some(format!("https://dev.azure.com/{org}/{project}/_git/{repo}"));
        }
        return None;
    }
    Some(format!("{scheme}://{host}/{path}"))
}

#[cfg(test)]
mod tests {
    use super::web_url;

    #[test]
    fn https_remotes_keep_scheme_and_port_and_drop_credentials() {
        let cases = [
            ("https://github.com/owner/repo.git", "https://github.com/owner/repo"),
            ("https://github.com/owner/repo/", "https://github.com/owner/repo"),
            ("https://user:token@gitlab.com/group/sub/repo.git", "https://gitlab.com/group/sub/repo"),
            ("http://gitea.local:3000/owner/repo", "http://gitea.local:3000/owner/repo"),
            (
                "https://org@dev.azure.com/org/project/_git/repo",
                "https://dev.azure.com/org/project/_git/repo",
            ),
        ];
        for (remote, expected) in cases {
            assert_eq!(web_url(remote).as_deref(), Some(expected), "{remote}");
        }
    }

    #[test]
    fn ssh_remotes_map_to_https_on_the_same_host() {
        let cases = [
            ("git@github.com:owner/repo.git", "https://github.com/owner/repo"),
            ("git@bitbucket.org:team/repo.git", "https://bitbucket.org/team/repo"),
            ("ssh://git@gitlab.example.com:2222/group/repo.git", "https://gitlab.example.com/group/repo"),
            ("git://github.com/owner/repo.git", "https://github.com/owner/repo"),
            ("git@ssh.dev.azure.com:v3/org/project/repo", "https://dev.azure.com/org/project/_git/repo"),
        ];
        for (remote, expected) in cases {
            assert_eq!(web_url(remote).as_deref(), Some(expected), "{remote}");
        }
    }

    #[test]
    fn local_and_unsupported_remotes_have_no_web_page() {
        for remote in [
            "/srv/git/repo.git",
            "../repo",
            "file:///srv/git/repo.git",
            "C:\\repos\\repo",
            "https://github.com",
            "",
        ] {
            assert_eq!(web_url(remote), None, "{remote}");
        }
    }
}
