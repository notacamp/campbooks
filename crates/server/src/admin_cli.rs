//! `campbooks-server admin grant|revoke|list|log` (ADR 0015): the only way
//! to make someone an instance administrator, change what they may do, or
//! stop it. It runs with shell access to the server, on the **migration
//! login** (`CAMPBOOKS_DATABASE_MIGRATION_URL`, else the database URL): the
//! runtime role every request runs as cannot write a profile.

use std::{
    io::{BufRead as _, Write as _},
    process::ExitCode,
};

use campbooks_admin::cli::{self, Profile, Revoked};
use campbooks_kernel::{Config, admin::Permission, db};
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
}

/// Whose profile: by address, or by account id when an address is shared.
#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub struct Who {
    /// A verified address the person signed in with. Refused unless exactly
    /// one account has it.
    #[arg(long)]
    email: Option<String>,
    /// The account id (the `sub` of their tokens; `list` shows them).
    #[arg(long)]
    account: Option<Uuid>,
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
    let result = match command {
        AdminCommand::Grant {
            who,
            permissions,
            note,
            yes,
        } => grant(&mut conn, &who, &permissions, note.as_deref(), yes).await,
        AdminCommand::Revoke {
            who,
            permissions,
            yes,
        } => revoke(&mut conn, &who, &permissions, yes).await,
        AdminCommand::List => list(&mut conn).await,
        AdminCommand::Log { limit } => log(&mut conn, limit).await,
    };
    let _ = conn.close().await;
    result
}

async fn grant(
    conn: &mut PgConnection,
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
        &account.id.to_string(),
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
    let outcome = cli::revoke(
        conn,
        &account.id.to_string(),
        permissions,
        chrono::Utc::now(),
    )
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
    id: Uuid,
    label: String,
}

async fn resolve(conn: &mut PgConnection, who: &Who) -> Result<Account, String> {
    let id = match (&who.account, &who.email) {
        (Some(id), _) => *id,
        (None, Some(email)) => {
            let found = campbooks_identity::domain::accounts::operator_lookup(conn, email)
                .await
                .map_err(|e| format!("could not look the address up: {e}"))?;
            match found.as_slice() {
                [id] => *id,
                [] => {
                    return Err(format!(
                        "no account has the verified address {email}; the person must sign in \
                         once first"
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
    match campbooks_identity::domain::accounts::describe(conn, id)
        .await
        .map_err(|e| format!("could not read the account: {e}"))?
    {
        Some((name, email)) => Ok(Account {
            id,
            label: label(id, name.as_deref(), email.as_deref()),
        }),
        None => Err(format!("no account has the id {id}")),
    }
}

async fn describe(conn: &mut PgConnection, profile: &Profile) -> String {
    let Ok(id) = profile.account_id.parse::<Uuid>() else {
        return profile.account_id.clone();
    };
    match campbooks_identity::domain::accounts::describe(conn, id).await {
        Ok(Some((name, email))) => label(id, name.as_deref(), email.as_deref()),
        _ => format!("{id} (no such account)"),
    }
}

fn label(id: Uuid, name: Option<&str>, email: Option<&str>) -> String {
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
    fn labels_name_the_person_when_known() {
        let id = Uuid::nil();
        assert_eq!(
            label(id, Some("Ana"), Some("ana@example.com")),
            format!("{id} (Ana <ana@example.com>)")
        );
        assert_eq!(label(id, None, None), id.to_string());
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
        let by_id = Who {
            email: None,
            account: Some(Uuid::now_v7()),
        };
        let err = resolve(&mut conn, &by_id).await.err().unwrap();
        assert!(err.contains("no account has the id"), "{err}");
    }
}
