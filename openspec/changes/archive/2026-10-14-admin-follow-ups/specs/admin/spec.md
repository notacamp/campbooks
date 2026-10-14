## ADDED Requirements

### Requirement: Account standings for the People list
The `identity` module SHALL serve `GET /admin/v1/identity/accounts`
(`identity.admin_list_accounts`, `users.read`) taking up to 200
comma-separated account ids in `ids` and answering, for each of them that is
an account of this instance, its id and its suspension time, and nothing
else. Ids that are not accounts here SHALL be left out; more than 200 ids
SHALL answer 400 `too_many_accounts`; anyone without `users.read` SHALL get
404 whatever the parameters.

#### Scenario: One request for a page of people
- **WHEN** a super admin asks about two accounts, one of them suspended, plus an unknown id and another provider's subject
- **THEN** the answer lists exactly the two accounts, the suspended one with its suspension time, each with no field but its id and suspension time

#### Scenario: Too many at once
- **WHEN** a super admin asks about 201 accounts
- **THEN** the answer is 400 `too_many_accounts`

#### Scenario: Not an administrator
- **WHEN** a member without a profile asks about any accounts
- **THEN** the answer is 404

## MODIFIED Requirements

### Requirement: People and Workspaces panels
The admin section SHALL offer **People** (a searchable list that shows more
on request and marks each person whose account is suspended with a
"Suspended" tag; a person's page with their account, ways in, sessions,
workspaces and a Suspend or Unsuspend action) and **Workspaces** (a
searchable list; a workspace's page with its members and its connections'
health), in all four locales, at 375 px and desktop widths, with loading,
empty and error states. Suspend SHALL ask for confirmation in place, saying
that the person is signed out everywhere and cannot sign in until
unsuspended, and that nothing is deleted. The list SHALL learn which
accounts are suspended with one request per page of people and SHALL show
again without tags if that request fails.

#### Scenario: Suspending from the People page
- **WHEN** a super admin opens a person, chooses Suspend and confirms
- **THEN** the account shows as suspended with the time, and the action becomes Unsuspend

#### Scenario: An administrator's account
- **WHEN** the confirmation is answered `account_is_admin`
- **THEN** the page explains that the admin profile must be revoked with the server's command line first, and nothing changes

#### Scenario: Suspended people stand out in the list
- **WHEN** a super admin opens People and one person on the page is suspended
- **THEN** that person's row carries a "Suspended" tag and no other row does

### Requirement: The Jobs panel
The admin dashboard SHALL have a Jobs panel showing the queue by kind and the
failed jobs with their last error in monospace, wrapped and copyable, and
SHALL let an administrator retry a job, discard it after a confirmation, and
retry every failed job of a kind after a confirmation. Every action SHALL say
what happened, and the panel SHALL work at 375 px, in light and dark, and in
the four shipped locales. The last error shown SHALL be the sanitized,
bounded message the worker stored, never a handler's raw message.

#### Scenario: Discarding asks first
- **WHEN** an administrator chooses Discard on a failed job
- **THEN** the panel asks for confirmation naming the kind and says the job is deleted for good, and nothing is deleted until they confirm

### Requirement: Signup administration
Administrators holding `signups.review` (or `super`) SHALL be able, through
the admin API, to list signup requests (filtered by status, pending first,
cursor-paged), approve or reject a request (a rejection may carry a reason of
at most 500 characters), list signup codes, mint a code and revoke a code.
Approving, rejecting and revoking SHALL be idempotent. Listing codes SHALL
never reveal a code or its digest; minting SHALL return the code in that
response only. Approving and rejecting SHALL be recorded in the security log.
Everyone else SHALL get 404. The admin screen SHALL describe a code as good
for the number of signups set when it was minted, stopping once used up,
expired or revoked, and never as single-use.

#### Scenario: Deciding is idempotent
- **WHEN** an administrator approves the same request twice
- **THEN** both answer 200 with the same decision time and one `signup_approved` entry exists

#### Scenario: A code is shown once
- **WHEN** an administrator mints a code
- **THEN** the response contains the code, and neither the list of codes nor any later response does

#### Scenario: Not an administrator
- **WHEN** a member, or an administrator with only `jobs.read`, calls any signup operation
- **THEN** it answers 404

#### Scenario: Paging
- **WHEN** a list is read with `limit=2` and each response's `next` is passed as `after`
- **THEN** every row appears exactly once, pending requests first
