## ADDED Requirements

### Requirement: AI usage for administrators
The admin API SHALL let an administrator with `system.read` (or `super`) read
a day's AI usage (`GET /admin/v1/system/ai-usage`, `system.admin_ai_usage`;
today in UTC unless a `day` is given): the configured cap (`null` when there
is none), the reserve kept for people, the day and when it resets, and for
each workspace that used AI
that day its calls, refused calls, whether it hit the cap, the calls left
today, and its calls, failed calls and tokens by role and feature.
Workspaces that hit the cap SHALL come first, then by calls. The answer
SHALL hold ids and counts only.

#### Scenario: Someone who is not a system reader
- **WHEN** a member, or an administrator without `system.read`, reads the AI usage
- **THEN** the response is 404

#### Scenario: A capped workspace
- **WHEN** one workspace reached its cap today and another did not
- **THEN** the capped one is listed first with `capped` true and its refused calls, and the other with its remaining calls

#### Scenario: Another day
- **WHEN** an administrator asks for a past day
- **THEN** the counts of that day are returned without a remaining count
