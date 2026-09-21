# frozen_string_literal: true

module Api
  module App
    module People
      # GET /api/app/people/orgs/:id
      # Org page: standing, stand note, member persons + services rows.
      class OrgsController < Api::App::BaseController
        def show
          org = current_workspace.organizations.find(params[:id])

          standing = PeopleStanding.for_user(current_user).find_by(counterpart: org)&.standing ||
                     ::People::Standing.for_organization(org, user: current_user)

          # Member persons and services: find all contacts belonging to this org,
          # then look up their standings from the materialized table.
          person_ids  = org.contacts.kind_person.pluck(:person_id).compact.uniq
          service_ids = org.contacts.kind_service.pluck(:person_id).compact.uniq

          person_rows  = PeopleStanding.for_user(current_user)
                                       .where(counterpart_type: "Person", counterpart_id: person_ids)
                                       .ranked
          service_rows = PeopleStanding.for_user(current_user)
                                       .where(counterpart_type: "Person", counterpart_id: service_ids)
                                       .ranked

          render_data(
            Api::App::OrgSerializer.new(
              org,
              standing:     standing,
              person_rows:  person_rows,
              service_rows: service_rows
            ).as_json
          )
        end
      end
    end
  end
end
