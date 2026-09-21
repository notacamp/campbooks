# frozen_string_literal: true

# Single per-user JSON fan-out channel for the /api/app SPA. Replaces the web's
# Turbo-Stream-of-HTML broadcasts with plain-JSON envelopes that land in the
# client's TanStack Query cache. One stream per user; every envelope names its
# `topic` (people, notifications, now, …) so the client routes it to the right
# cache slice. Auth rides ApplicationCable::Connection, which accepts the same
# /api/app bearer (query param) as HTTP — see api-migration/realtime.md.
#
# Surfaces publish with one line from any controller/job/service:
#   UserSyncChannel.publish(user, topic: "people", action: "upsert",
#                           id: row_id, payload: serialized_row)
class UserSyncChannel < ApplicationCable::Channel
  def subscribed
    stream_for current_user
  end

  def unsubscribed
    stop_all_streams
  end

  # Push a JSON envelope to one user's SPA.
  #   topic   — routes it client-side (people | notifications | now | …)
  #   action  — the verb (upsert | remove | refresh | append | …)
  #   id      — the affected record id (nil for a whole-topic refresh)
  #   payload — the already-serialized resource, or a partial patch
  def self.publish(user, topic:, action:, id: nil, payload: {})
    return unless user

    broadcast_to(user, {
      topic:   topic.to_s,
      action:  action.to_s,
      id:      id,
      payload: payload,
      at:      ::Time.current.iso8601
    })
  end
end
