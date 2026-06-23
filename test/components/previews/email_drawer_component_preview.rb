# frozen_string_literal: true

class EmailDrawerComponentPreview < ViewComponent::Preview
  def open
    render(Campbooks::EmailDrawer.new(open: true, size: :lg)) do |drawer|
      drawer.with_header do
        helpers.tag.div(class: "flex items-center justify-between w-full") do
          helpers.concat(helpers.tag.h3("Email Subject Example", class: "text-sm font-semibold text-gray-900"))
          helpers.concat(helpers.tag.button("Close", class: "text-xs text-gray-400 hover:text-gray-600"))
        end
      end
      drawer.with_body do
        helpers.tag.div(class: "p-5") do
          helpers.tag.p("This is an example drawer body. Email content would load here via Turbo Frame.", class: "text-sm text-gray-600")
        end
      end
    end
  end

  def closed
    render(Campbooks::EmailDrawer.new(open: false, size: :lg)) do |drawer|
      drawer.with_header do
        helpers.tag.h3("Closed Drawer", class: "text-sm font-semibold text-gray-900")
      end
      drawer.with_body do
        helpers.tag.p("This drawer is closed by default.", class: "text-sm text-gray-600")
      end
    end
  end

  def medium_size
    render(Campbooks::EmailDrawer.new(open: true, size: :md)) do |drawer|
      drawer.with_header do
        helpers.tag.h3("Medium Drawer", class: "text-sm font-semibold text-gray-900")
      end
      drawer.with_body do
        helpers.tag.p("This is the md size (w-96).", class: "text-sm text-gray-600")
      end
    end
  end
end
