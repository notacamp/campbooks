# frozen_string_literal: true

# Data-only self-heal: materialize the Organizations directory from already-
# analyzed contacts (Person#organization strings) so existing installs get their
# directory populated on upgrade, without anyone finding the manual "Sync from
# contacts" button. From this release analyses also materialize inline
# (Organizations::Backfill.link_analyzed_person); this covers the history.
#
# Idempotent (find-or-create on org + membership) and deliberately defensive:
# prod boots through db:prepare, so a bad workspace is logged and skipped rather
# than ever failing the migration.
class BackfillOrganizationsFromAnalyzedPeople < ActiveRecord::Migration[8.1]
  def up
    [ Workspace, Person, Organization, OrganizationMembership ].each(&:reset_column_information)

    Workspace.find_each do |workspace|
      count = Organizations::Backfill.new(workspace).call
      say "workspace #{workspace.id}: #{count} organization(s) materialized" if count.positive?
    rescue StandardError => e
      say "workspace #{workspace.id}: organizations backfill skipped (#{e.class}: #{e.message})"
    end
  rescue StandardError => e
    say "organizations backfill skipped entirely (#{e.class}: #{e.message})"
  end

  def down
    # Data-only; nothing to undo.
  end
end
