module Organizations
  class Backfill
    # Materialize a single freshly-analyzed person into the directory (org +
    # membership) the moment analysis fills Person#organization, so organizations
    # appear as profiling completes instead of waiting for a manual "Sync from
    # contacts". Same semantics as the full backfill: a person already in the
    # directory is left alone. Concurrent analyses can race on a new org name
    # (unique index on workspace+name) — one retry resolves to the winner's row.
    def self.link_analyzed_person(person)
      return unless person

      name = person.read_attribute(:organization).to_s.strip
      return if name.blank?
      return if person.organization_memberships.exists?

      retries = 0
      begin
        org = person.workspace.organizations.find_or_create_by!(name: name)
        OrganizationMembership.find_or_create_by!(person: person, organization: org) { |m| m.status = :active }
      rescue ActiveRecord::RecordNotUnique, ActiveRecord::RecordInvalid
        raise if (retries += 1) > 1
        retry
      end
    end

    def initialize(workspace) = @workspace = workspace
    def call
      created = 0
      @workspace.people.where.not(organization: [ nil, "" ]).where.not(id: already_member_ids)
        .distinct.pluck(:organization).each do |org_name|
        name = org_name.strip
        next if name.blank?
        org = @workspace.organizations.find_or_create_by!(name: name)
        link_people(org, org_name)
        created += 1
      end
      created
    end
    private
    def already_member_ids
      OrganizationMembership.joins(:organization)
        .where(organizations: { workspace_id: @workspace.id }).select(:person_id)
    end
    def link_people(org, org_name)
      @workspace.people.where(organization: org_name)
        .where.not(id: already_member_ids).find_each do |person|
        OrganizationMembership.find_or_create_by!(person: person, organization: org) { |m| m.status = :active }
      end
    end
  end
end
