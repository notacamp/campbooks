# frozen_string_literal: true

module Api
  module App
    module People
      # GET  /api/app/people/:id/details      — contact facts
      # PATCH /api/app/people/:id/details     — update field (name/relationship/kind/state)
      # POST /api/app/people/:id/details/analyze    — trigger AI re-analysis
      # POST /api/app/people/:id/details/attention  — set attention verdict
      # POST /api/app/people/:id/details/merge      — merge into target person
      class DetailsController < Api::App::BaseController
        before_action :set_person

        # GET /api/app/people/:id/details
        def show
          profile = ::People::Profile.for(@person, user: current_user)
          render_data(serialize_profile(profile))
        end

        # PATCH /api/app/people/:id/details
        def update
          field = params[:field].to_s

          case field
          when "name"
            @person.update!(name: params.require(:value).to_s.strip.presence || @person.name)
          when "relationship"
            @person.update!(relationship_type: params[:value].presence)
          when "kind"
            kind_val = params.require(:value).to_s
            return render_error("invalid_value", "Invalid sender kind.", status: :unprocessable_entity) unless Contact.sender_kinds.key?(kind_val)

            contact = primary_contact
            return render_not_found unless contact

            stream = kind_val == "service" ? Contacts::StreamKind.classify(contact) : nil
            contact.update!(sender_kind: kind_val, sender_kind_source: "taught", stream_kind: stream)
            ::Organizations::FromDomain.link(contact) if contact.kind_service?
          when "state"
            state_val = params.require(:value).to_s
            contact   = primary_contact
            return render_not_found unless contact

            case state_val
            when "star"
              contact.star!
            when "unstar"
              contact.unstar!
            when "allow"
              contact.allow!
            when "block"
              Contacts::Block.call(contact, user: current_user)
            when "unblock"
              Contacts::Unblock.call(contact, user: current_user)
            else
              return render_error("invalid_value", "Unsupported state '#{state_val}'.",
                                  status: :unprocessable_entity)
            end
          else
            return render_error("unsupported_field", "Unsupported field '#{field}'.",
                                status: :unprocessable_entity)
          end

          profile = ::People::Profile.for(@person, user: current_user)
          render_data(serialize_profile(profile))
        end

        # POST /api/app/people/:id/details/analyze
        def analyze
          contact = primary_contact
          ContactAnalysisJob.perform_later(contact.id, force: true) if contact
          render_data({ queued: true })
        end

        # POST /api/app/people/:id/details/attention
        def attention
          verdict = params[:verdict].to_s
          unless %w[important unimportant forget].include?(verdict)
            return render_error("invalid_value", "Invalid verdict.", status: :unprocessable_entity)
          end

          case verdict
          when "important"
            ::Attention::Teach.record(person: @person, user: current_user, label: "important", source: "rail")
          when "unimportant"
            ::Attention::Teach.record(person: @person, user: current_user, label: "unimportant", source: "rail")
          when "forget"
            ::Attention::Teach.forget(person: @person, user: current_user)
          end

          render_data({ verdict: verdict })
        end

        # POST /api/app/people/:id/details/merge
        def merge
          contact = primary_contact
          return render_not_found unless contact

          if params[:approve].to_s == "true"
            target = contact.suggested_person
            return render_error("no_suggestion", "No merge suggestion found.", status: :unprocessable_entity) unless target

            contact.update!(person: target, suggested_person_id: nil,
                            suggested_reason: nil, suggested_confidence: nil)
            render_data({ merged: true, target_id: target.id })
          else
            contact.update!(suggested_person_id: nil, suggested_reason: nil, suggested_confidence: nil)
            render_data({ dismissed: true })
          end
        end

        private

        def set_person
          @person = current_workspace.people.find(params[:id])
        end

        def primary_contact
          @person.contacts.max_by { |c| c.email_count.to_i }
        end

        def serialize_profile(profile)
          {
            person_id:    @person.id,
            name:         @person.display_name,
            emails:       (profile.emails || []).map { |addr, primary| { address: addr, primary: primary } },
            organization: profile.organization&.then { |o| { id: o.id, name: o.name } },
            relationship: profile.relationship,
            sender_kind:  profile.sender_kind,
            starred:      profile.starred?,
            tags:         (profile.tags || []).map { |t| { id: t.id, name: t.name } },
            documents:    (profile.documents || []).map { |d| { id: d.id, name: d.display_name } },
            events:       (profile.events || []).map { |e| { id: e.id, title: e.title } },
            analysis_stale: profile.analysis_stale?,
            analyzed_at:  profile.analyzed_at&.iso8601,
            duplicate_suggestion: profile.duplicate_suggestion&.then { |s|
              { person_id: s[:person_id], confidence: s[:confidence], reason: s[:reason] }
            }
          }
        rescue StandardError
          { person_id: @person.id, name: @person.display_name }
        end
      end
    end
  end
end
