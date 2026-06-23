module Campbooks
  module Calendar
    # Shared layout math for the time-grid views (Day + Week): the hour scale, and
    # the greedy overlap-column packing that positions a day's timed events into
    # side-by-side columns. Included into DayGrid and WeekTimeGrid (both subclass
    # Campbooks::Base, so `l` is available for hour labels).
    module TimeGrid
      START_HOUR = 0
      END_HOUR = 24
      HOUR_PX = 44
      MIN_EVENT_PX = 22

      def grid_height = (END_HOUR - START_HOUR) * HOUR_PX
      def hours = (START_HOUR...END_HOUR)
      def hour_top(hour) = (hour - START_HOUR) * HOUR_PX
      def hour_label(day, hour) = l(day.to_time.change(hour: hour), format: :clock)

      # [{event:, top:, height:, left: (%), width: (%)}] for the day's timed events,
      # overlaps split into equal side-by-side columns. left/width are percentages
      # of a single day column (the Week grid scales them into its 1/7 slice).
      def day_boxes(events, day)
        timed = events.select { |e| !e.all_day && e.start_at && e.end_at }
                      .sort_by { |e| [ e.start_at, e.end_at ] }
        result = []
        cluster = []
        cluster_end = nil

        flush = lambda do
          col_ends = []
          assignments = cluster.map do |e|
            col = col_ends.index { |end_at| end_at <= e.start_at }
            if col.nil?
              col_ends << e.end_at
              col = col_ends.size - 1
            else
              col_ends[col] = e.end_at
            end
            [ e, col ]
          end
          ncols = col_ends.size
          assignments.each { |e, col| result << box_for(e, day, col, ncols) }
          cluster = []
        end

        timed.each do |e|
          if cluster.empty? || e.start_at < cluster_end
            cluster << e
            cluster_end = [ cluster_end, e.end_at ].compact.max
          else
            flush.call
            cluster << e
            cluster_end = e.end_at
          end
        end
        flush.call unless cluster.empty?
        result
      end

      def box_for(event, day, col, ncols)
        day_start = day.beginning_of_day
        start_min = [ ((event.start_at - day_start) / 60).to_i, START_HOUR * 60 ].max
        end_min   = [ ((event.end_at - day_start) / 60).to_i, END_HOUR * 60 ].min
        end_min = start_min + 30 if end_min <= start_min

        top = ((start_min - START_HOUR * 60) / 60.0) * HOUR_PX
        height = [ ((end_min - start_min) / 60.0) * HOUR_PX, MIN_EVENT_PX ].max
        col_width = 100.0 / ncols
        { event: event, top: top.round, height: height.round, left: (col * col_width).round(2), width: (col_width - 1).round(2) }
      end
    end
  end
end
