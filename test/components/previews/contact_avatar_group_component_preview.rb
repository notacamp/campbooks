# frozen_string_literal: true

# Facepile of an email thread's participants. Shows size scale and "+N" overflow.
class ContactAvatarGroupComponentPreview < ViewComponent::Preview
  PEOPLE = [
    { email: "ana@example.com", contact_id: nil },
    { email: "bob@studio.io", contact_id: nil },
    { email: "carla@firm.co", contact_id: nil },
    { email: "diego@agency.com", contact_id: nil },
    { email: "eve@partner.org", contact_id: nil },
    { email: "frank@vendor.net", contact_id: nil }
  ].freeze

  # @param size select { choices: [sm, md, lg, xl] }
  # @param count number
  # @param max number
  def playground(size: :xl, count: 5, max: 3)
    render(Campbooks::ContactAvatarGroup.new(
      participants: PEOPLE.first(count.to_i), size: size.to_sym, max: max.to_i
    ))
  end

  def single
    render(Campbooks::ContactAvatarGroup.new(participants: PEOPLE.first(1), size: :xl))
  end

  def three
    render(Campbooks::ContactAvatarGroup.new(participants: PEOPLE.first(3), size: :xl))
  end

  def overflow
    render(Campbooks::ContactAvatarGroup.new(participants: PEOPLE.first(6), size: :xl, max: 3))
  end

  def big_overflow
    render(Campbooks::ContactAvatarGroup.new(
      participants: PEOPLE + Array.new(20) { |i| { email: "extra#{i}@x.com" } },
      size: :xl, max: 3
    ))
  end

  def size_sm
    render(Campbooks::ContactAvatarGroup.new(participants: PEOPLE, size: :sm, max: 3))
  end

  def size_md
    render(Campbooks::ContactAvatarGroup.new(participants: PEOPLE, size: :md, max: 3))
  end

  def size_lg
    render(Campbooks::ContactAvatarGroup.new(participants: PEOPLE, size: :lg, max: 3))
  end

  def size_xl
    render(Campbooks::ContactAvatarGroup.new(participants: PEOPLE, size: :xl, max: 3))
  end

  def accent
    render(Campbooks::ContactAvatarGroup.new(participants: PEOPLE.first(4), size: :xl, max: 3, variant: :accent))
  end

  def account_ring
    render(Campbooks::ContactAvatarGroup.new(
      participants: PEOPLE.first(4), size: :xl, max: 3, account_color: "#595dec"
    ))
  end
end
