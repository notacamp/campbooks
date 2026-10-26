//! `campbooks-server admin grant|revoke|list|log` (ADR 0015): the only way
//! to make someone an instance administrator, change what they may do, or
//! stop it. It runs with shell access to the server, on the **migration
//! login** (`CAMPBOOKS_DATABASE_MIGRATION_URL`, else the database URL): the
//! runtime role every request runs as cannot write a profile.
//!
//! `campbooks-server admin apps add|list|rotate-secret|remove`
//! (campbooks-cloud#369) does the same for **instance apps**: confidential
//! sign-in clients of the whole instance, for the operator's own other apps
//! (first-party single sign-on). No HTTP endpoint registers, rotates or
//! removes one, and the runtime role cannot change one.
//!
//! A change of a profile or an instance app is tracked (ADR 0020) with the
//! tracker `CAMPBOOKS_TRACKING__*` configures, as `system` `admin` (the
//! command's name, outside any request or job) with the role `cli`.

use std::{
    io::{BufRead as _, Write as _},
    process::ExitCode,
};

use campbooks_admin::cli::{self, Profile, Revoked};
use campbooks_identity::domain::instance_apps::{self, InstanceApp, NewInstanceApp};
use campbooks_kernel::{Config, admin::Permission, config::Environment, db, tracking::Tracker};
use clap::{Args, Subcommand};
use sqlx::{Connection as _, PgConnection};
use uuid::Uuid;

/// What `campbooks-server admin` does.
#[derive(Debug, Subcommand)]
pub enum AdminCommand {
    /// Make an account an administrator, or give an administrator more
    /// permissions. The account must have signed in once.
    Grant {
        #[command(flatten)]
        who: Who,
        /// A permission to give (repeatable): `super` (everything), or
        /// `system.read`, `signups.review`, `users.read`, `users.suspend`,
        /// `workspaces.read`, `jobs.read`, `jobs.retry`.
        #[arg(long = "permission", required = true, value_parser = parse_permission)]
        permissions: Vec<Permission>,
        /// Why, for the next operator (`list` shows it).
        #[arg(long)]
        note: Option<String>,
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
    /// Take permissions away; without `--permission`, the whole profile:
    /// the account is no longer an administrator, from its next request.
    Revoke {
        #[command(flatten)]
        who: Who,
        /// A permission to take away (repeatable). Default: all of them.
        #[arg(long = "permission", value_parser = parse_permission)]
        permissions: Vec<Permission>,
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
    /// List the administrators.
    List,
    /// Show the newest entries of the admin audit trail.
    Log {
        /// How many entries.
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    /// Manage instance apps: the operator's own other apps that sign people
    /// in with Campbooks (OpenID Connect, a confidential client). The only
    /// way to register, rotate or remove one.
    Apps {
        #[command(subcommand)]
        command: AppsCommand,
    },
}

/// What `campbooks-server admin apps` does.
#[derive(Debug, Subcommand)]
pub enum AppsCommand {
    /// Register an instance app; prints its client id and its secret, the
    /// only time the secret is shown.
    Add {
        /// The name people see on the consent screen.
        #[arg(long)]
        name: String,
        /// Where codes may go (repeatable): https, matched exactly (loopback
        /// http only when CAMPBOOKS_ENV=development).
        #[arg(long = "redirect-uri", required = true)]
        redirect_uris: Vec<String>,
        /// Where the browser may return after signing out (repeatable).
        #[arg(long = "post-logout-redirect-uri")]
        post_logout_redirect_uris: Vec<String>,
        /// The scope ceiling: every scope it may ever be granted. Any of
        /// `openid profile email offline_access` and the API scopes a
        /// third party may hold (`campbooks-server scopes` lists them).
        #[arg(long = "scopes", required = true, num_args = 1..)]
        scopes: Vec<String>,
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
    /// List the instance apps.
    List,
    /// Give an instance app a new secret, shown once; the old one stops
    /// working at once.
    RotateSecret {
        /// Its client id (`app_…`).
        client_id: String,
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
    /// Remove an instance app: every consent people gave it and every
    /// refresh token it holds end at once.
    Remove {
        /// Its client id (`app_…`).
        client_id: String,
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
}

/// Whose profile: by address, or by account id when an address is shared.
#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub struct Who {
    /// A verified address the person signed in with. Refused unless exactly
    /// one account has it.
    #[arg(long)]
    email: Option<String>,
    /// The account id: the `sub` of their tokens (`list` shows them; a
    /// signed-in browser sees its own at `/bff/session`).
    #[arg(long)]
    account: Option<String>,
}

fn parse_permission(name: &str) -> Result<Permission, String> {
    Permission::parse(name).ok_or_else(|| {
        let known: Vec<&str> = Permission::ALL.iter().map(|p| p.as_str()).collect();
        format!("unknown permission `{name}` (known: {})", known.join(", "))
    })
}

/// Run `command`; the exit code says whether it worked.
pub fn run(command: AdminCommand) -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => return fail(&e.to_string()),
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => return fail(&format!("could not start async runtime: {e}")),
    };
    match runtime.block_on(execute(&config, command)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&e),
    }
}

async fn execute(config: &Config, command: AdminCommand) -> Result<(), String> {
    let mut conn = connect(config).await?;
    let tracker = Tracker::new(&config.tracking).with_role("cli");
    let result = match command {
        AdminCommand::Grant {
            who,
            permissions,
            note,
            yes,
        } => {
            grant(
                &mut conn,
                &tracker,
                &who,
                &permissions,
                note.as_deref(),
                yes,
            )
            .await
        }
        AdminCommand::Revoke {
            who,
            permissions,
            yes,
        } => revoke(&mut conn, &tracker, &who, &permissions, yes).await,
        AdminCommand::List => list(&mut conn).await,
        AdminCommand::Log { limit } => log(&mut conn, limit).await,
        AdminCommand::Apps { command } => apps(config, &mut conn, &tracker, command).await,
    };
    let _ = conn.close().await;
    result
}

async fn grant(
    conn: &mut PgConnection,
    tracker: &Tracker,
    who: &Who,
    permissions: &[Permission],
    note: Option<&str>,
    yes: bool,
) -> Result<(), String> {
    let account = resolve(conn, who).await?;
    let names: Vec<&str> = permissions.iter().map(|p| p.as_str()).collect();
    confirm(
        yes,
        &format!(
            "Give {} the admin permission(s) {}?",
            account.label,
            names.join(", ")
        ),
    )?;
    let profile = cli::grant(
        conn,
        tracker,
        &account.id,
        permissions,
        note,
        chrono::Utc::now(),
    )
    .await
    .map_err(|e| format!("could not grant: {e}"))?;
    say(&format!(
        "granted: {} is an administrator with {}",
        account.label,
        profile.permissions.join(", ")
    ));
    Ok(())
}

async fn revoke(
    conn: &mut PgConnection,
    tracker: &Tracker,
    who: &Who,
    permissions: &[Permission],
    yes: bool,
) -> Result<(), String> {
    let account = resolve(conn, who).await?;
    let what = if permissions.is_empty() {
        "every admin permission".to_owned()
    } else {
        permissions
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    confirm(yes, &format!("Take {what} away from {}?", account.label))?;
    let outcome = cli::revoke(conn, tracker, &account.id, permissions, chrono::Utc::now())
        .await
        .map_err(|e| format!("could not revoke: {e}"))?;
    say(&match outcome {
        Revoked::NotAnAdmin => format!("unchanged: {} is not an administrator", account.label),
        Revoked::Removed => format!("revoked: {} is no longer an administrator", account.label),
        Revoked::Remaining(profile) => format!(
            "revoked: {} keeps {}",
            account.label,
            profile.permissions.join(", ")
        ),
    });
    Ok(())
}

async fn list(conn: &mut PgConnection) -> Result<(), String> {
    let profiles = cli::list(conn)
        .await
        .map_err(|e| format!("could not list: {e}"))?;
    if profiles.is_empty() {
        say("no administrators yet: grant one with `campbooks-server admin grant`");
    }
    for profile in profiles {
        let label = describe(conn, &profile).await;
        say(&format!(
            "{}  {}  since {}{}",
            label,
            profile.permissions.join(","),
            profile.granted_at.format("%Y-%m-%d"),
            profile
                .note
                .map(|note| format!("  ({note})"))
                .unwrap_or_default()
        ));
    }
    Ok(())
}

async fn log(conn: &mut PgConnection, limit: i64) -> Result<(), String> {
    for action in cli::log(conn, limit)
        .await
        .map_err(|e| format!("could not read the trail: {e}"))?
    {
        say(&format!(
            "{}  {:<3}  {}  {}  {}{}",
            action.at.format("%Y-%m-%d %H:%M:%S"),
            action.via,
            action.actor,
            action.operation,
            action.target,
            action
                .status
                .map(|status| format!("  -> {status}"))
                .unwrap_or_default()
        ));
    }
    Ok(())
}

async fn apps(
    config: &Config,
    conn: &mut PgConnection,
    tracker: &Tracker,
    command: AppsCommand,
) -> Result<(), String> {
    match command {
        AppsCommand::Add {
            name,
            redirect_uris,
            post_logout_redirect_uris,
            scopes,
            yes,
        } => {
            let asked = NewInstanceApp {
                name,
                redirect_uris,
                post_logout_redirect_uris,
                scopes: split_scopes(&scopes),
            };
            add_app(config, conn, tracker, &asked, yes).await
        }
        AppsCommand::List => list_apps(conn).await,
        AppsCommand::RotateSecret { client_id, yes } => {
            rotate_app(conn, tracker, client_id.trim(), yes).await
        }
        AppsCommand::Remove { client_id, yes } => {
            remove_app(conn, tracker, client_id.trim(), yes).await
        }
    }
}

async fn add_app(
    config: &Config,
    conn: &mut PgConnection,
    tracker: &Tracker,
    asked: &NewInstanceApp,
    yes: bool,
) -> Result<(), String> {
    let app = instance_apps::validate(
        asked,
        &campbooks_server::scope_catalogue(),
        config.env == Environment::Development,
    )
    .map_err(|refusal| format!("refused: {refusal}"))?;
    confirm(
        yes,
        &format!(
            "Register the instance app {:?} with the scopes {} and the redirect URIs {}?",
            app.name,
            app.scopes.join(" "),
            app.redirect_uris.join(" ")
        ),
    )?;
    let (stored, secret) = instance_apps::register(conn, tracker, &app, chrono::Utc::now())
        .await
        .map_err(|e| format!("could not register: {e}"))?;
    say(&format!("registered: {}", stored.name));
    say(&format!(
        "issuer: {}",
        config.public_url.trim_end_matches('/')
    ));
    say(&format!("client_id: {}", stored.client_id));
    say(&format!("client_secret: {secret}"));
    warn("the secret is shown this once: store it now (rotate-secret makes a new one)");
    Ok(())
}

async fn list_apps(conn: &mut PgConnection) -> Result<(), String> {
    let apps = instance_apps::list(conn)
        .await
        .map_err(|e| format!("could not list: {e}"))?;
    if apps.is_empty() {
        say("no instance apps yet: register one with `campbooks-server admin apps add`");
    }
    for app in &apps {
        say(&describe_app(app));
    }
    Ok(())
}

async fn rotate_app(
    conn: &mut PgConnection,
    tracker: &Tracker,
    client_id: &str,
    yes: bool,
) -> Result<(), String> {
    confirm(
        yes,
        &format!("Give {client_id} a new secret? The current one stops working at once."),
    )?;
    let Some((app, secret)) =
        instance_apps::rotate_secret(conn, tracker, client_id, chrono::Utc::now())
            .await
            .map_err(|e| format!("could not rotate: {e}"))?
    else {
        return Err(format!("no instance app has the client id {client_id}"));
    };
    say(&format!("rotated: {} ({})", app.client_id, app.name));
    say(&format!("client_secret: {secret}"));
    warn("the secret is shown this once: store it now");
    Ok(())
}

async fn remove_app(
    conn: &mut PgConnection,
    tracker: &Tracker,
    client_id: &str,
    yes: bool,
) -> Result<(), String> {
    confirm(
        yes,
        &format!("Remove {client_id}? Everyone signed in to it is signed out of it, at once."),
    )?;
    let Some(removed) = instance_apps::remove(conn, tracker, client_id, chrono::Utc::now())
        .await
        .map_err(|e| format!("could not remove: {e}"))?
    else {
        return Err(format!("no instance app has the client id {client_id}"));
    };
    say(&format!(
        "removed: {client_id}; {} consent(s) and {} sign-in(s) revoked",
        removed.consents_revoked, removed.refresh_tokens_revoked
    ));
    Ok(())
}

/// `--scopes` as given: repeated, or several in one quoted value.
fn split_scopes(given: &[String]) -> Vec<String> {
    given
        .iter()
        .flat_map(|value| value.split([' ', ',']))
        .filter(|scope| !scope.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn describe_app(app: &InstanceApp) -> String {
    let date = |at: &chrono::DateTime<chrono::Utc>| at.format("%Y-%m-%d").to_string();
    let mut parts = vec![
        app.client_id.clone(),
        app.name.clone(),
        format!("since {}", date(&app.created_at)),
        format!("scopes: {}", app.allowed_scopes.join(" ")),
        format!("redirect: {}", app.redirect_uris.join(" ")),
    ];
    if !app.post_logout_redirect_uris.is_empty() {
        parts.push(format!(
            "after logout: {}",
            app.post_logout_redirect_uris.join(" ")
        ));
    }
    if let Some(at) = &app.secret_rotated_at {
        parts.push(format!("secret rotated {}", date(at)));
    }
    parts.push(match &app.last_used_at {
        Some(at) => format!("last used {}", date(at)),
        None => "never used".to_owned(),
    });
    parts.join("  ")
}

/// A connection on the migration login, as `migrate` opens it.
async fn connect(config: &Config) -> Result<PgConnection, String> {
    let options = if config.split_database_logins() {
        db::connect_options(config.migration_url(), None)?
    } else {
        db::connect_options(config.migration_url(), config.database_password_file())?
    };
    PgConnection::connect_with(&options)
        .await
        .map_err(|e| format!("could not connect to the database with the migration login: {e}"))
}

/// The account a command is about, and how to name it on screen.
struct Account {
    id: String,
    label: String,
}

async fn resolve(conn: &mut PgConnection, who: &Who) -> Result<Account, String> {
    let id = match (&who.account, &who.email) {
        (Some(id), _) => id.trim().to_owned(),
        (None, Some(email)) => {
            let found = campbooks_identity::domain::accounts::operator_lookup(conn, email)
                .await
                .map_err(|e| format!("could not look the address up: {e}"))?;
            match found.as_slice() {
                [id] => id.to_string(),
                [] => {
                    return Err(format!(
                        "no account has the verified address {email}; the person must sign in \
                         once first (with another identity provider, use --account with the \
                         `sub` of their tokens)"
                    ));
                }
                several => {
                    let ids: Vec<String> = several.iter().map(ToString::to_string).collect();
                    return Err(format!(
                        "{} accounts have the address {email}; choose one with --account: {}",
                        several.len(),
                        ids.join(", ")
                    ));
                }
            }
        }
        (None, None) => return Err("say who: --email or --account".to_owned()),
    };
    if id.is_empty() {
        return Err("--account is empty".to_owned());
    }
    let known = match id.parse::<Uuid>() {
        Ok(uuid) => campbooks_identity::domain::accounts::describe(conn, uuid)
            .await
            .map_err(|e| format!("could not read the account: {e}"))?,
        Err(_) => None,
    };
    let label = if let Some((name, email)) = known {
        label(&id, name.as_deref(), email.as_deref())
    } else {
        warn(&format!(
            "no Campbooks account has the id {id}: fine when another identity provider \
             (CAMPBOOKS_AUTH__ISSUER) issues the tokens, otherwise check the id"
        ));
        id.clone()
    };
    Ok(Account { id, label })
}

async fn describe(conn: &mut PgConnection, profile: &Profile) -> String {
    let id = &profile.account_id;
    let Ok(uuid) = id.parse::<Uuid>() else {
        return id.clone();
    };
    match campbooks_identity::domain::accounts::describe(conn, uuid).await {
        Ok(Some((name, email))) => label(id, name.as_deref(), email.as_deref()),
        _ => id.clone(),
    }
}

fn label(id: &str, name: Option<&str>, email: Option<&str>) -> String {
    match (name, email) {
        (Some(name), Some(email)) => format!("{id} ({name} <{email}>)"),
        (None, Some(email)) => format!("{id} (<{email}>)"),
        (Some(name), None) => format!("{id} ({name})"),
        (None, None) => id.to_string(),
    }
}

/// Ask on the terminal unless `yes`.
fn confirm(yes: bool, question: &str) -> Result<(), String> {
    if yes {
        return Ok(());
    }
    let mut err = std::io::stderr();
    let _ = write!(err, "{question} [y/N] ");
    let _ = err.flush();
    let mut answer = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut answer)
        .map_err(|e| format!("could not read the answer: {e}"))?;
    if matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err("cancelled; nothing changed".to_owned())
    }
}

fn warn(line: &str) {
    let _ = writeln!(std::io::stderr(), "warning: {line}");
}

fn say(line: &str) {
    let _ = writeln!(std::io::stdout(), "{line}");
}

fn fail(message: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "campbooks-server admin: {message}");
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_parse_by_wire_name() {
        assert_eq!(parse_permission("super"), Ok(Permission::Super));
        assert_eq!(parse_permission("jobs.retry"), Ok(Permission::JobsRetry));
        let err = parse_permission("root").unwrap_err();
        assert!(err.contains("system.read"), "{err}");
    }

    #[test]
    fn scopes_may_come_repeated_or_in_one_value() {
        assert_eq!(
            split_scopes(&[
                "openid profile".to_owned(),
                "email".to_owned(),
                "conversations:read,schedule:read".to_owned(),
            ]),
            [
                "openid",
                "profile",
                "email",
                "conversations:read",
                "schedule:read"
            ]
        );
    }

    #[test]
    fn the_apps_command_parses() {
        use clap::Parser as _;
        #[derive(Debug, clap::Parser)]
        struct Cli {
            #[command(subcommand)]
            command: AdminCommand,
        }
        let cli = Cli::try_parse_from([
            "admin",
            "apps",
            "add",
            "--name",
            "Applicamp",
            "--redirect-uri",
            "https://applicamp.example.com/auth/callback",
            "--post-logout-redirect-uri",
            "https://applicamp.example.com/",
            "--scopes",
            "openid",
            "profile",
            "email",
            "offline_access",
            "conversations:read",
            "--yes",
        ])
        .unwrap();
        let AdminCommand::Apps {
            command:
                AppsCommand::Add {
                    name,
                    redirect_uris,
                    post_logout_redirect_uris,
                    scopes,
                    yes,
                },
        } = cli.command
        else {
            panic!("not `apps add`");
        };
        assert_eq!(name, "Applicamp");
        assert_eq!(
            redirect_uris,
            ["https://applicamp.example.com/auth/callback"]
        );
        assert_eq!(
            post_logout_redirect_uris,
            ["https://applicamp.example.com/"]
        );
        assert_eq!(scopes.len(), 5);
        assert!(yes);
        for args in [
            ["admin", "apps", "rotate-secret", "app_x"],
            ["admin", "apps", "remove", "app_x"],
        ] {
            assert!(Cli::try_parse_from(args).is_ok(), "{args:?}");
        }
        assert!(Cli::try_parse_from(["admin", "apps", "list"]).is_ok());
        // A redirect URI is required.
        assert!(
            Cli::try_parse_from(["admin", "apps", "add", "--name", "X", "--scopes", "openid"])
                .is_err()
        );
    }

    #[test]
    fn labels_name_the_person_when_known() {
        assert_eq!(
            label("a-1", Some("Ana"), Some("ana@example.com")),
            "a-1 (Ana <ana@example.com>)"
        );
        assert_eq!(label("a-1", None, None), "a-1");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn an_address_must_name_exactly_one_account(db: sqlx::PgPool) {
        let mut conn = db.acquire().await.unwrap();
        let by_email = Who {
            email: Some("nobody@example.com".to_owned()),
            account: None,
        };
        let err = resolve(&mut conn, &by_email).await.err().unwrap();
        assert!(err.contains("sign in once first"), "{err}");
        // An id no Campbooks account has (another identity provider's
        // `sub`) is taken as it is, with a warning.
        let by_id = Who {
            email: None,
            account: Some(" outside-user-7 ".to_owned()),
        };
        let account = resolve(&mut conn, &by_id).await.unwrap();
        assert_eq!(
            (account.id.as_str(), account.label.as_str()),
            ("outside-user-7", "outside-user-7")
        );
    }
}
