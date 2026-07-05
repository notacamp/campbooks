# frozen_string_literal: true

require "test_helper"

module Contacts
  class ConsolidatorTest < ActiveSupport::TestCase
    # Names distinctive enough that no other test's people fuzzy-match them.
    NAME = "Quorvax Zendleman"

    test "merges same-named people within one workspace" do
      ws = Workspace.create!(name: "Consolidator WS")
      primary = ws.people.create!(name: NAME)
      secondary = ws.people.create!(name: NAME)
      contact = ws.contacts.create!(email: "qz@example.com", person: secondary)

      assert Consolidator.consolidate!(secondary)

      assert_not Person.exists?(secondary.id)
      assert_equal primary.id, contact.reload.person_id
    end

    test "never merges people across workspaces" do
      ws_a = Workspace.create!(name: "Consolidator WS A")
      ws_b = Workspace.create!(name: "Consolidator WS B")
      person_a = ws_a.people.create!(name: NAME)
      person_b = ws_b.people.create!(name: NAME)
      contact_b = ws_b.contacts.create!(email: "qz-b@example.com", person: person_b)

      assert_not Consolidator.consolidate!(person_b)

      assert Person.exists?(person_a.id)
      assert Person.exists?(person_b.id)
      assert_equal person_b.id, contact_b.reload.person_id
    end
  end
end
