# frozen_string_literal: true

require "test_helper"

module Organizations
  class BackfillTest < ActiveSupport::TestCase
    setup { @ws = Workspace.create!(name: "Org Backfill WS") }

    def build_person(organization: nil, name: "Person #{SecureRandom.hex(4)}")
      @ws.people.create!(name: name, organization: organization)
    end

    test "link_analyzed_person creates the organization and an active membership" do
      person = build_person(organization: "Acme GmbH")

      Backfill.link_analyzed_person(person)

      org = @ws.organizations.find_by(name: "Acme GmbH")
      assert org.present?
      assert org.organization_memberships.active.exists?(person_id: person.id)
    end

    test "link_analyzed_person reuses an existing organization by name" do
      org = @ws.organizations.create!(name: "Acme GmbH")
      person = build_person(organization: "Acme GmbH")

      assert_no_difference -> { @ws.organizations.count } do
        Backfill.link_analyzed_person(person)
      end
      assert org.organization_memberships.exists?(person_id: person.id)
    end

    test "link_analyzed_person is idempotent" do
      person = build_person(organization: "Acme GmbH")

      2.times { Backfill.link_analyzed_person(person) }

      assert_equal 1, @ws.organizations.count
      assert_equal 1, person.organization_memberships.count
    end

    test "link_analyzed_person leaves a person who already belongs to an organization alone" do
      person = build_person(organization: "New Corp")
      existing = @ws.organizations.create!(name: "Old Corp")
      OrganizationMembership.create!(person: person, organization: existing, status: :active)

      Backfill.link_analyzed_person(person)

      assert_not @ws.organizations.exists?(name: "New Corp")
      assert_equal 1, person.organization_memberships.count
    end

    test "link_analyzed_person is a no-op for nil person or blank organization" do
      person = build_person(organization: "   ")

      Backfill.link_analyzed_person(nil)
      Backfill.link_analyzed_person(person)

      assert_equal 0, @ws.organizations.count
    end

    test "full backfill materializes organizations for every analyzed person" do
      build_person(organization: "Acme GmbH")
      build_person(organization: "Acme GmbH")
      build_person(organization: "Globex")
      build_person(organization: nil)

      Backfill.new(@ws).call

      assert_equal %w[Acme\ GmbH Globex], @ws.organizations.order(:name).pluck(:name)
      assert_equal 3, OrganizationMembership.joins(:organization)
        .where(organizations: { workspace_id: @ws.id }).count
    end
  end
end
