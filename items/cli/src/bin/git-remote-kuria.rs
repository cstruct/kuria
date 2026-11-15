//! `git-remote-kuria`: git runs it for `kuria::<url>/<number>` remotes
//! (`git.remote`), with the remote's name and URL.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (Some(name), Some(url)) = (args.get(1), args.get(2)) else {
        eprintln!("git-remote-kuria: git runs this with a remote's name and URL");
        return ExitCode::from(2);
    };
    let served = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a Tokio runtime")
        .block_on(async {
            let remote = kuria_cli::open_remote(name, url).await?;
            kuria_cli::remote::serve(&remote).await
        });
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kuria: {}", e.message);
            ExitCode::from(1)
        }
    }
}
