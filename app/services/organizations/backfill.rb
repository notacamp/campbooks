module Organizations
  class Backfill
    def initialize(workspace) = @workspace = workspace
    def call
      created = 0
      @workspace.people.where.not(organization: [nil, ""]).where.not(id: already_member_ids)
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
