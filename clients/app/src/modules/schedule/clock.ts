import { useEffect, useState } from "react";

/** The time, ticking once a minute (the line in the week, "Today", Home's Next up); `fixed` pins it. */
export function useClock(fixed: Date | undefined): Date {
  const [now, setNow] = useState(() => fixed ?? new Date());
  useEffect(() => {
    if (fixed !== undefined) {
      return;
    }
    const id = setInterval(() => {
      setNow(new Date());
    }, 60_000);
    return () => {
      clearInterval(id);
    };
  }, [fixed]);
  return fixed ?? now;
}
