use super::*;
use crate::tenancy::WorkspaceId;

const BOX: ResourceType = ResourceType {
    owner: Owner::Member,
    owner_actions: &[Action::Read, Action::Write, Action::Share],
    container: Some("t.box"),
    roles: &["reader", "writer"],
    grants: &[
        Grant::new(Grantee::ContainerRole("reader"), &[Action::Read]),
        Grant::new(
            Grantee::ContainerRole("writer"),
            &[Action::Read, Action::Write],
        ),
        Grant::new(Grantee::Clients, &[Action::Read]),
    ],
    ..ResourceType::new("t.box", "A container.", Tenancy::Workspace)
};

const ITEM: ResourceType = ResourceType {
    container: Some("t.box"),
    grants: &[
        Grant::new(Grantee::ContainerRole("reader"), &[Action::Read]),
        Grant::new(Grantee::ContainerRole("writer"), &[Action::Write]),
    ],
    ..ResourceType::new("t.item", "Inside a box.", Tenancy::Workspace)
};

const NOTE: ResourceType = ResourceType {
    owner: Owner::Member,
    owner_actions: &[Action::Read, Action::Delete],
    container: Some("t.box"),
    grants: &[Grant::new(
        Grantee::OwnerWithContainerRole("writer"),
        &[Action::Write, Action::Send],
    )],
    ..ResourceType::new("t.note", "Private to its author.", Tenancy::Workspace)
};

const SETTING: ResourceType = ResourceType {
    owner: Owner::WorkspaceOwners,
    owner_actions: &[Action::Read, Action::Write, Action::Delete],
    grants: &[
        Grant::new(
            Grantee::WorkspaceRole(Role::Admin),
            &[Action::Read, Action::Write],
        ),
        Grant::new(Grantee::WorkspaceRole(Role::Member), &[Action::Read]),
    ],
    ..ResourceType::new("t.setting", "Workspace-wide.", Tenancy::Workspace)
};

const BAD: ResourceType = ResourceType {
    container: Some("t.box"),
    grants: &[Grant::new(Grantee::ContainerRole("owner"), &[Action::Read])],
    ..ResourceType::new("t.bad", "Names a role the box lacks.", Tenancy::Workspace)
};

static TYPES: [ResourceType; 4] = [BOX, ITEM, NOTE, SETTING];

fn catalogue() -> Catalogue {
    Catalogue::new([&Resources::declared(&TYPES)]).unwrap()
}

fn member(user: UserId, role: Role) -> Access {
    Access {
        workspace: WorkspaceId::new(),
        user: Some(user),
        role: Some(role),
    }
}

fn client() -> Access {
    Access {
        workspace: WorkspaceId::new(),
        user: None,
        role: None,
    }
}

fn in_box(owner: OwnerFact, role: Option<&str>) -> Facts {
    Facts::owned_by(owner).within(RecordRef::new("t.box", "b"), role.map(ToOwned::to_owned))
}

#[test]
fn unknown_types_and_missing_resources_are_not_found() {
    let c = catalogue();
    let ana = member(UserId::new(), Role::Owner);
    let facts = Facts::owned_by(OwnerFact::Nobody);
    assert_eq!(
        decide(&c, &ana, Action::Read, "t.unknown", Some(&facts)),
        Decision::NotFound
    );
    assert_eq!(
        decide(&c, &ana, Action::Read, "t.item", None),
        Decision::NotFound
    );
}

#[test]
fn container_roles_grant_exactly_their_actions() {
    let c = catalogue();
    let user = UserId::new();
    let caller = member(user, Role::Member);
    let reader = in_box(OwnerFact::Nobody, Some("reader"));
    let writer = in_box(OwnerFact::Nobody, Some("writer"));
    let outsider = in_box(OwnerFact::Nobody, None);
    assert_eq!(
        decide(&c, &caller, Action::Read, "t.item", Some(&reader)),
        Decision::Allow
    );
    assert_eq!(
        decide(&c, &caller, Action::Write, "t.item", Some(&reader)),
        Decision::Forbidden
    );
    assert_eq!(
        decide(&c, &caller, Action::Write, "t.item", Some(&writer)),
        Decision::Allow
    );
    // Higher roles hold the lower ones' grants.
    assert_eq!(
        decide(&c, &caller, Action::Read, "t.item", Some(&writer)),
        Decision::Allow
    );
    for action in Action::ALL {
        assert_eq!(
            decide(&c, &caller, action, "t.item", Some(&outsider)),
            Decision::NotFound,
            "{action}"
        );
    }
}

#[test]
fn workspace_admins_get_nothing_inside_containers() {
    let c = catalogue();
    let admin = member(UserId::new(), Role::Owner);
    let facts = in_box(OwnerFact::User(UserId::new()), None);
    assert_eq!(
        decide(&c, &admin, Action::Read, "t.box", Some(&facts)),
        Decision::NotFound
    );
    assert_eq!(
        decide(&c, &admin, Action::Read, "t.item", Some(&facts)),
        Decision::NotFound
    );
}

#[test]
fn owners_get_their_actions_and_others_do_not_see_private_records() {
    let c = catalogue();
    let author = UserId::new();
    let facts = in_box(OwnerFact::User(author), Some("writer"));
    let own = member(author, Role::Member);
    assert_eq!(
        decide(&c, &own, Action::Send, "t.note", Some(&facts)),
        Decision::Allow
    );
    assert_eq!(
        decide(&c, &own, Action::Share, "t.note", Some(&facts)),
        Decision::Forbidden
    );
    // Another writer of the same box, even the workspace's owner.
    let other = member(UserId::new(), Role::Owner);
    assert_eq!(
        decide(&c, &other, Action::Read, "t.note", Some(&facts)),
        Decision::NotFound
    );
    // The author demoted to reader keeps reading and deleting, not sending.
    let demoted = in_box(OwnerFact::User(author), Some("reader"));
    assert_eq!(
        decide(&c, &own, Action::Delete, "t.note", Some(&demoted)),
        Decision::Allow
    );
    assert_eq!(
        decide(&c, &own, Action::Send, "t.note", Some(&demoted)),
        Decision::Forbidden
    );
}

#[test]
fn clients_get_their_explicit_grant_only() {
    let c = catalogue();
    let facts = in_box(OwnerFact::User(UserId::new()), None);
    assert_eq!(
        decide(&c, &client(), Action::Read, "t.box", Some(&facts)),
        Decision::Allow
    );
    assert_eq!(
        decide(&c, &client(), Action::Write, "t.box", Some(&facts)),
        Decision::Forbidden
    );
    assert_eq!(
        decide(&c, &client(), Action::Read, "t.item", Some(&facts)),
        Decision::NotFound
    );
    // A record a client wrote belongs to the clients.
    let written = in_box(OwnerFact::Clients, None);
    assert_eq!(
        decide(&c, &client(), Action::Delete, "t.note", Some(&written)),
        Decision::Allow
    );
    let user = member(UserId::new(), Role::Owner);
    assert_eq!(
        decide(&c, &user, Action::Read, "t.note", Some(&written)),
        Decision::NotFound
    );
}

#[test]
fn workspace_roles_rank() {
    let c = catalogue();
    let facts = Facts::owned_by(OwnerFact::Nobody);
    let read = |role| {
        decide(
            &c,
            &member(UserId::new(), role),
            Action::Read,
            "t.setting",
            Some(&facts),
        )
    };
    let write = |role| {
        decide(
            &c,
            &member(UserId::new(), role),
            Action::Write,
            "t.setting",
            Some(&facts),
        )
    };
    let delete = |role| {
        decide(
            &c,
            &member(UserId::new(), role),
            Action::Delete,
            "t.setting",
            Some(&facts),
        )
    };
    assert_eq!(read(Role::Member), Decision::Allow);
    assert_eq!(write(Role::Member), Decision::Forbidden);
    assert_eq!(write(Role::Admin), Decision::Allow);
    assert_eq!(delete(Role::Admin), Decision::Forbidden);
    assert_eq!(delete(Role::Owner), Decision::Allow);
    assert_eq!(
        decide(&c, &client(), Action::Read, "t.setting", Some(&facts)),
        Decision::NotFound
    );
}

#[test]
fn frozen_resources_are_read_only() {
    let c = catalogue();
    let owner = UserId::new();
    let facts = in_box(OwnerFact::User(owner), Some("writer")).frozen(true);
    let caller = member(owner, Role::Member);
    assert_eq!(
        decide(&c, &caller, Action::Read, "t.box", Some(&facts)),
        Decision::Allow
    );
    assert_eq!(
        decide(&c, &caller, Action::Write, "t.box", Some(&facts)),
        Decision::Forbidden
    );
}

#[test]
fn type_level_checks_follow_the_grants() {
    let c = catalogue();
    let user = member(UserId::new(), Role::Member);
    assert_eq!(
        authorize_type(&c, &user, Action::Read, "t.item"),
        Decision::Allow
    );
    assert_eq!(
        authorize_type(&c, &client(), Action::Read, "t.box"),
        Decision::Allow
    );
    assert_eq!(
        authorize_type(&c, &client(), Action::Read, "t.item"),
        Decision::Forbidden
    );
    assert_eq!(
        authorize_type(&c, &user, Action::Write, "t.setting"),
        Decision::Forbidden
    );
    assert_eq!(
        authorize_type(&c, &user, Action::Read, "t.nope"),
        Decision::NotFound
    );
}

#[test]
fn the_catalogue_rejects_duplicates_and_unknown_container_roles() {
    let types: &'static [ResourceType] = Box::leak(Box::new([BOX, BOX]));
    assert!(Catalogue::new([&Resources::declared(types)]).is_err());
    let types: &'static [ResourceType] = Box::leak(Box::new([BOX, BAD]));
    assert!(Catalogue::new([&Resources::declared(types)]).is_err());
}

#[test]
fn actions_round_trip() {
    for action in Action::ALL {
        assert_eq!(Action::parse(action.as_str()), Some(action));
    }
    assert_eq!(Action::parse("own"), None);
}
