//! What a member can do with a page: the highest role any of these gives
//! them, walking from the page up through its ancestors.
//!
//! - **Workspace place**: every member writes; the page's creator and the
//!   workspace's owners also share and delete.
//! - **Private place**: its creator, and the creator of any page above it,
//!   hold everything; nobody else holds anything without a grant.
//! - **Grants**: a grant for the member on the page, or on the nearest page
//!   above it that has one for them, gives that role. Grants only add.
//!
//! Workspace owners and admins get nothing on a private page. Seeing is the
//! lowest role: no role means the page does not exist for the caller.

use std::{collections::HashMap, hash::BuildHasher};

use campbooks_kernel::{
    Result,
    tenancy::{Access, Role as WorkspaceRole, UserId},
};
use sqlx::PgConnection;

use super::{PageId, Place, Role};

/// One page of a chain, from the page itself up to its top-level page.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: PageId,
    pub parent: Option<PageId>,
    pub place: Place,
    pub created_by: UserId,
}

/// Who asks.
#[derive(Debug, Clone, Copy)]
pub struct Caller {
    /// The member (a client token acting for one is that member).
    pub user: UserId,
    /// Whether they own the workspace.
    pub owner: bool,
}

impl Caller {
    /// The caller of `access`; none for a token acting for nobody.
    pub fn of(access: &Access) -> Option<Self> {
        access.user.map(|user| Self {
            user,
            owner: access.role == Some(WorkspaceRole::Owner),
        })
    }
}

fn raise(best: Option<Role>, role: Role) -> Role {
    best.map_or(role, |held| held.max(role))
}

/// The role `caller` holds on `chain[0]`, whose ancestors follow it, given
/// the caller's `grants`.
pub fn role_in_chain<S: BuildHasher>(
    chain: &[Node],
    grants: &HashMap<PageId, Role, S>,
    caller: Caller,
) -> Option<Role> {
    let page = chain.first()?;
    let mut best: Option<Role> = None;
    match page.place {
        Place::Workspace => {
            best = Some(raise(best, Role::Writer));
            if caller.owner || page.created_by == caller.user {
                best = Some(raise(best, Role::Manager));
            }
        }
        Place::Private => {
            if chain.iter().any(|node| node.created_by == caller.user) {
                best = Some(raise(best, Role::Manager));
            }
        }
    }
    if let Some(granted) = chain.iter().find_map(|node| grants.get(&node.id)) {
        best = Some(raise(best, *granted));
    }
    best
}

const CHAIN: &str = "WITH RECURSIVE chain AS (
        SELECT id, parent_id, place, created_by, 1 AS depth FROM pages.pages WHERE id = $1
        UNION ALL
        SELECT p.id, p.parent_id, p.place, p.created_by, c.depth + 1
          FROM pages.pages p JOIN chain c ON p.id = c.parent_id
         WHERE c.depth < 12
    )
    SELECT id, parent_id, place, created_by FROM chain ORDER BY depth";

type NodeRow = (PageId, Option<PageId>, String, UserId);

fn node_of(row: NodeRow) -> Result<Node> {
    let (id, parent, place, created_by) = row;
    Ok(Node {
        id,
        parent,
        place: place.parse()?,
        created_by,
    })
}

/// Page `id` and the pages above it, nearest first (at most 12). Empty when
/// there is no such page in the workspace.
pub async fn chain(conn: &mut PgConnection, id: PageId) -> Result<Vec<Node>> {
    let rows: Vec<NodeRow> = sqlx::query_as(CHAIN).bind(id).fetch_all(conn).await?;
    rows.into_iter().map(node_of).collect()
}

/// `user`'s grants among the pages `ids`.
pub async fn grants_among(
    conn: &mut PgConnection,
    user: UserId,
    ids: &[PageId],
) -> Result<HashMap<PageId, Role>> {
    let rows: Vec<(PageId, String)> = sqlx::query_as(
        "SELECT page_id, role FROM pages.grants WHERE member_id = $1 AND page_id = ANY($2)",
    )
    .bind(user)
    .bind(ids)
    .fetch_all(conn)
    .await?;
    rows.into_iter()
        .map(|(page, role)| Ok((page, role.parse()?)))
        .collect()
}

/// What `access` holds on page `id`: `None` when the page does not exist,
/// `Some(None)` when it exists and the caller may not see it.
pub async fn standing(
    conn: &mut PgConnection,
    access: &Access,
    id: PageId,
) -> Result<Option<Option<Role>>> {
    let nodes = chain(&mut *conn, id).await?;
    if nodes.is_empty() {
        return Ok(None);
    }
    let Some(caller) = Caller::of(access) else {
        return Ok(Some(None));
    };
    let ids: Vec<PageId> = nodes.iter().map(|node| node.id).collect();
    let grants = grants_among(conn, caller.user, &ids).await?;
    Ok(Some(role_in_chain(&nodes, &grants, caller)))
}

/// The role `access` holds on every page of the workspace it may see. The
/// rows of tables are left out: they follow their table, can hold no
/// sub-pages, and there can be 20,000 to a table.
pub async fn readable(conn: &mut PgConnection, access: &Access) -> Result<HashMap<PageId, Role>> {
    let Some(caller) = Caller::of(access) else {
        return Ok(HashMap::new());
    };
    let rows: Vec<NodeRow> =
        sqlx::query_as(
            "SELECT id, parent_id, place, created_by FROM pages.pages WHERE kind <> 'row'",
        )
            .fetch_all(&mut *conn)
            .await?;
    let nodes: Vec<Node> = rows.into_iter().map(node_of).collect::<Result<_>>()?;
    let rows: Vec<(PageId, String)> =
        sqlx::query_as("SELECT page_id, role FROM pages.grants WHERE member_id = $1")
            .bind(caller.user)
            .fetch_all(conn)
            .await?;
    let grants: HashMap<PageId, Role> = rows
        .into_iter()
        .map(|(page, role)| Ok((page, role.parse()?)))
        .collect::<Result<_>>()?;
    let by_id: HashMap<PageId, &Node> = nodes.iter().map(|node| (node.id, node)).collect();
    let mut roles = HashMap::new();
    for node in &nodes {
        let mut chain: Vec<Node> = Vec::new();
        let mut next = Some(node);
        while let Some(current) = next {
            chain.push(current.clone());
            if chain.len() >= super::MAX_DEPTH {
                break;
            }
            next = current
                .parent
                .and_then(|parent| by_id.get(&parent).copied());
        }
        if let Some(role) = role_in_chain(&chain, &grants, caller) {
            roles.insert(node.id, role);
        }
    }
    Ok(roles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(place: Place, created_by: UserId) -> Node {
        Node {
            id: PageId::new(),
            parent: None,
            place,
            created_by,
        }
    }

    const fn caller(user: UserId, owner: bool) -> Caller {
        Caller { user, owner }
    }

    #[test]
    fn a_workspace_page_is_every_members_and_its_creators_and_owners() {
        let (ana, rui, wes) = (UserId::new(), UserId::new(), UserId::new());
        let chain = [node(Place::Workspace, ana)];
        let none = HashMap::new();
        assert_eq!(
            role_in_chain(&chain, &none, caller(rui, false)),
            Some(Role::Writer)
        );
        assert_eq!(
            role_in_chain(&chain, &none, caller(ana, false)),
            Some(Role::Manager)
        );
        assert_eq!(
            role_in_chain(&chain, &none, caller(wes, true)),
            Some(Role::Manager)
        );
    }

    #[test]
    fn a_private_page_is_its_creators_alone() {
        let (ana, rui, wes) = (UserId::new(), UserId::new(), UserId::new());
        let chain = [node(Place::Private, ana)];
        let none = HashMap::new();
        assert_eq!(
            role_in_chain(&chain, &none, caller(ana, false)),
            Some(Role::Manager)
        );
        assert_eq!(role_in_chain(&chain, &none, caller(rui, false)), None);
        // Not even the workspace's owner.
        assert_eq!(role_in_chain(&chain, &none, caller(wes, true)), None);
    }

    #[test]
    fn a_grant_reaches_sub_pages_and_the_nearest_wins() {
        let (ana, rui) = (UserId::new(), UserId::new());
        let top = node(Place::Private, ana);
        let mid = node(Place::Private, ana);
        let leaf = node(Place::Private, ana);
        let chain = [leaf.clone(), mid.clone(), top.clone()];
        let mut grants = HashMap::new();
        grants.insert(top.id, Role::Reader);
        assert_eq!(
            role_in_chain(&chain, &grants, caller(rui, false)),
            Some(Role::Reader)
        );
        // A nearer grant of the same member replaces the farther one.
        grants.insert(mid.id, Role::Writer);
        assert_eq!(
            role_in_chain(&chain, &grants, caller(rui, false)),
            Some(Role::Writer)
        );
        // And the creator of a page above holds everything below it.
        assert_eq!(
            role_in_chain(&chain, &grants, caller(ana, false)),
            Some(Role::Manager)
        );
    }

    #[test]
    fn a_grant_never_lowers_what_the_workspace_gives() {
        let (ana, rui) = (UserId::new(), UserId::new());
        let page = node(Place::Workspace, ana);
        let mut grants = HashMap::new();
        grants.insert(page.id, Role::Reader);
        assert_eq!(
            role_in_chain(&[page], &grants, caller(rui, false)),
            Some(Role::Writer)
        );
    }
}
