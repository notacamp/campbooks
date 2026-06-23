# frozen_string_literal: true

class StatComponentPreview < ViewComponent::Preview
  def default
    render(Campbooks::Stat.new(value: "42", label: "Documents this month"))
  end

  def neutral
    render(Campbooks::Stat.new(value: "128", label: "Total emails", variant: :neutral))
  end

  def attention
    render(Campbooks::Stat.new(value: "3", label: "Need review", variant: :attention))
  end

  def success
    render(Campbooks::Stat.new(value: "12", label: "Approved", variant: :success))
  end

  def danger
    render(Campbooks::Stat.new(value: "2", label: "Failed jobs", variant: :danger))
  end

  def info
    render(Campbooks::Stat.new(value: "5", label: "Connected accounts", variant: :info))
  end

  def with_link
    render(Campbooks::Stat.new(value: "7", label: "Unread emails", variant: :attention, href: "/email_messages"))
  end

  def with_icon
    render(Campbooks::Stat.new(value: "€1,234", label: "Revenue", variant: :success)) do |stat|
      stat.with_icon do
        helpers.tag.svg(class: "w-5 h-5 text-green-500", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24") do
          helpers.tag.path("stroke-linecap" => "round", "stroke-linejoin" => "round", "stroke-width" => "2", d: "M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V6m0 10v2")
        end
      end
    end
  end

  def all_variants
    render(Campbooks::Card.new(padding: :lg)) do
      helpers.tag.div(class: "grid grid-cols-5 gap-4") do
        helpers.concat(render(Campbooks::Stat.new(value: "42", label: "Neutral", variant: :neutral)))
        helpers.concat(render(Campbooks::Stat.new(value: "3", label: "Attention", variant: :attention)))
        helpers.concat(render(Campbooks::Stat.new(value: "12", label: "Success", variant: :success)))
        helpers.concat(render(Campbooks::Stat.new(value: "2", label: "Danger", variant: :danger)))
        helpers.concat(render(Campbooks::Stat.new(value: "5", label: "Info", variant: :info)))
      end
    end
  end
end
