# frozen_string_literal: true

module Organizations
  # Idempotent backfill: groups existing Person.organization strings into Organization
  # records and creates active OrganizationMembership rows. Only links people who
  # don't already have an active membership — safe to run multiple times.
  class Backfill
    def initialize(workspace)
      @workspace = workspace
    end

    def call
      created = 0

      @workspace.people
        .where.not(organization: [ nil, "" ])
        .where.not(id: already_member_person_ids)
        .distinct.pluck(:organization).each do |org_name|
          name = org_name.strip
          next if name.blank?

          org = @workspace.organizations.find_or_create_by!(name: name)
          link_people_to(org, name)
          created += 1
        end

      created
    end

    private

    def already_member_person_ids
      OrganizationMembership.joins(:organization)
        .where(organizations: { workspace_id: @workspace.id })
        .select(:person_id)
    end

    def link_people_to(org, org_name)
      @workspace.people
        .where(organization: org_name)
        .where.not(id: already_member_person_ids)
        .find_each do |person|
          OrganizationMembership.find_or_create_by!(
            person: person,
            organization: org
          ) { |m| m.status = :active }
        end
    end
  end
end
